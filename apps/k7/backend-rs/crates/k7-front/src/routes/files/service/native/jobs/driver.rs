//! driver.rs — the real `Driver` of an account's jobs: renders on this machine or a bound one, over the link (sub-step 1k, B1).
//! What a reader gets wrong: availability and `bound` answer from memory on the runtime's thread, never a probe (2351).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};

use crate::routes::files::service::native::callbacks::Reply;
use k7_file_service::jobs::manifest::Moved;
use k7_file_service::jobs::separations::NO_ENGINE;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::machine::{MACHINE_TIMEOUT, render_on, render_to_on};
use super::render::{LocalRenderer, RenderOpts};
use super::separate;
use super::{Driver, Gone, Jobs, RenderFuture};
use crate::routes::files::service::machine::engine_binary;
use crate::routes::files::service::native::engine::EngineLink;

/// What the account lends its renders: the environment a child sees (2322) and the code root the engine binary is looked for from.
#[derive(Clone, Debug, Default)]
pub struct DriverEnv {
    pub code_root: PathBuf,
    pub env: Vec<(String, String)>,
}

impl DriverEnv {
    pub fn new(code_root: PathBuf, env: Vec<(String, String)>) -> DriverEnv {
        DriverEnv { code_root, env }
    }

    fn var(&self, name: &str) -> Option<&str> {
        let found = self.env.iter().find(|(k, _)| k == name);
        found.map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
    }

    /// The account's render cache, `<studio home>/.renders`: the one root every machine may write under.
    pub fn renders_home(&self) -> PathBuf {
        let home = self
            .var("STUDIO_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let user = self.var("HOME").or_else(|| self.var("USERPROFILE"));
                Path::new(user.unwrap_or("/")).join(".k7")
            });
        home.join(".renders")
    }
}

/// The driver the front runs: this machine's engine when it has one, else the bound machine.
pub fn real_driver(link: &EngineLink, env: DriverEnv) -> Arc<dyn Driver> {
    let bin = engine_binary(env.var("STUDIO_ENGINE"), &env.code_root);
    Arc::new(RealDriver {
        rt: link.0.rt.clone(),
        link: link.clone(),
        env,
        bin,
        jobs: OnceLock::new(),
    })
}

struct RealDriver {
    rt: Handle,
    link: EngineLink,
    env: DriverEnv,
    /// Found once, when the driver is made: a probe per question would run on the runtime's thread.
    bin: Option<PathBuf>,
    /// The jobs whose registry a cut's request writes into, told by `Jobs::new`.
    jobs: OnceLock<Weak<Jobs>>,
}

impl RealDriver {
    /// The jobs paths a render may go to: the bound machine's.
    fn machines(&self) -> Vec<String> {
        self.link.jobs_url().into_iter().collect()
    }
}

/// `{mix, stems: [{name, file}]}`, the manifest the TypeScript's renderer answers.
fn manifest(moved: &Moved) -> Value {
    let stems: Vec<Value> = moved
        .stems
        .iter()
        .map(|(name, file)| json!({ "name": name, "file": file }))
        .collect();
    json!({ "mix": moved.mix, "stems": stems })
}

impl Driver for RealDriver {
    fn renderer_available(&self) -> bool {
        self.bin.is_some() || !self.machines().is_empty()
    }

    fn bound(&self) -> bool {
        self.link.jobs_url().is_some()
    }

    fn render_run_until(&self, args: Value, reply: Reply, gone: Gone) {
        let text = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_owned);
        let (Some(out_dir), Some(doc)) = (text("outDir"), args.get("doc")) else {
            return reply(Err("outDir and doc are needed".into()));
        };
        let doc = doc.to_string();
        let opts = RenderOpts {
            stems: args.get("stems").and_then(Value::as_bool).unwrap_or(false),
            base: text("base"),
        };
        let (bin, env) = (self.bin.clone(), self.env.clone());
        let machines = self.machines();
        self.rt.spawn(async move {
            let render = async {
                match bin {
                    Some(bin) => {
                        let local = LocalRenderer::new(bin, env.env.clone());
                        local.render(&doc, Path::new(&out_dir), &opts).await
                    }
                    None => on_machines(&machines, &env, &doc, &out_dir, &opts).await,
                }
            };
            // The asker gave up: the render's future is dropped here, which kills the child (kill_on_drop).
            let done = tokio::select! {
                biased;
                done = render => done,
                () = gone => return,
            };
            reply(done.map(|m| json!({ "value": manifest(&m) })));
        });
    }

    fn render_all(&self, dir: String, reply: Reply) {
        let freezing = self
            .jobs
            .get()
            .and_then(Weak::upgrade)
            .and_then(|j| j.freezing.get().cloned());
        match freezing {
            Some(f) => f.render_all(&dir, reply),
            None => reply(Ok(json!({ "value": [] }))),
        }
    }

    fn render_freeze(&self, doc: String, out_wav: PathBuf) -> RenderFuture {
        let (link, env, bin) = (self.link.clone(), self.env.clone(), self.bin.clone());
        Box::pin(async move { freeze_render(&link, &env, bin, &doc, &out_wav).await })
    }

    fn separate(&self, id: &str, generation: u64, args: Value) {
        let Some(jobs) = self.jobs.get().cloned() else {
            return;
        };
        // The binding dropped after `start` took its record: fail it, or it stays on "sending" for good.
        let Some(url) = self.link.jobs_url() else {
            if let Some(j) = jobs.upgrade() {
                j.separation_failed(id, generation, true, NO_ENGINE);
            }
            return;
        };
        self.rt
            .spawn(separate::run(jobs, url, id.to_owned(), generation, args));
    }

    fn attach(&self, jobs: Weak<Jobs>) {
        let _ = self.jobs.set(jobs);
    }
}

/// Each machine in turn, the next tried when one fails; the last failure is the answer.
async fn on_machines(
    machines: &[String],
    env: &DriverEnv,
    doc: &str,
    out_dir: &str,
    opts: &RenderOpts,
) -> Result<Moved, String> {
    let mut failed = None;
    for url in machines {
        let renders = env.renders_home();
        match render_on(url, &renders, doc, out_dir, opts, MACHINE_TIMEOUT).await {
            Ok(m) => return Ok(m),
            Err(e) => failed = Some(e),
        }
    }
    Err(failed.unwrap_or_else(|| "no engine machine to render on".into()))
}

/// A freeze's render: the bound machine when there is one, else this machine's engine (the TypeScript's order for a freeze).
async fn freeze_render(
    link: &EngineLink,
    env: &DriverEnv,
    bin: Option<PathBuf>,
    doc: &str,
    out_wav: &Path,
) -> Result<(), String> {
    if let Some(url) = link.jobs_url() {
        return render_to_on(&url, doc, out_wav).await;
    }
    let Some(bin) = bin else {
        return Err("no k7-runtime binary on this machine".into());
    };
    LocalRenderer::new(bin, env.env.clone())
        .render_to_wav(doc, out_wav)
        .await
}
