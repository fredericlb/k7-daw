//! ports.rs — the shares' traits bound to the account's real parts: 1k's renderer and 1o's compile (2501, 2520).
//! What a reader gets wrong: it only CALLS `jobs` and `docs`; a compile runs on the compiler's own stack, never on the blocking pool's.

use std::path::Path;
use std::sync::Arc;

use k7_file_service::libraries::Home;
use k7_file_service::shares::names::no_such_timeline;
use serde_json::{Value, json};
use song::doc::{ProjectDoc, Vsts};
use song::session::View;
use tokio::sync::oneshot;

use super::{BoxFut, Manifest, MixDoc, Mixdown, Stem, Views};
use crate::routes::files::service::native::docs::{self, Asked, Docs};
use crate::routes::files::service::native::jobs::Jobs;
use crate::routes::files::service::native::stack::{Failed, on_stack};

/// What a compile that is declined is answered with: the route hands the request back before any work was done.
pub const DECLINED: &str = "\u{0}declined:";

/// The way a stand-in asks the jobs and waits for the answer.
async fn asked(jobs: &Jobs, kind: &str, args: Value) -> Result<Value, String> {
    let (tx, rx) = oneshot::channel();
    // Dropping this function's future drops `stop`, which is how the asker gives up.
    let (stop, stopped) = oneshot::channel::<()>();
    jobs.ask_until(
        kind,
        args,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
        Box::pin(async move {
            let _ = stopped.await;
        }),
    );
    let answer = rx
        .await
        .map_err(|_| "the render was dropped".to_owned())??;
    drop(stop);
    Ok(answer.get("value").cloned().unwrap_or(Value::Null))
}

/// 1k's renderer: `render.run` through the jobs' driver, on this machine or a bound one.
pub struct JobsMixdown(pub Arc<Jobs>);

impl Mixdown for JobsMixdown {
    fn available(&self) -> bool {
        matches!(self.0.query("render.available", &json!({})), Ok(v) if v["value"] == json!(true))
    }

    fn render<'a>(
        &'a self,
        doc: &'a Value,
        out: &'a Path,
        stems: bool,
    ) -> BoxFut<'a, Result<Manifest, String>> {
        Box::pin(async move {
            if !self.available() {
                return Err(k7_file_service::shares::no_engine("mixdown"));
            }
            let args = json!({ "doc": doc, "outDir": out.to_string_lossy(), "stems": stems });
            let v = asked(&self.0, "render.run", args).await?;
            let mix = v["mix"]
                .as_str()
                .ok_or("the render named no mix")?
                .to_owned();
            let stems = v["stems"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .filter_map(|s| {
                            Some(Stem {
                                name: s["name"].as_str()?.to_owned(),
                                file: s["file"].as_str()?.to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(Manifest { mix, stems })
        })
    }
}

/// 1o's compile for the shares: the active project through the shared sessions, a share's folder through a session of its own.
pub struct RealViews {
    pub docs: Docs,
    pub home: Home,
    pub log: k7_hub::Log,
}

impl RealViews {
    fn prepared(&self, project: &str, own: bool) -> Result<docs::Prepared, String> {
        let asked = Asked::of_project(project, None);
        let mut p = docs::prepare_with(&self.home, &self.docs, asked).map_err(str::to_owned)?;
        if own {
            p.ask.vsts = Vsts::None;
        }
        Ok(p)
    }

    /// The engine view of the active project, or why it is declined.
    fn engine_text(&self, project: &str) -> Result<Arc<str>, String> {
        let p = self.prepared(project, false)?;
        docs::view_text_with(&self.docs, true, &p, View::Engine)
            .map_err(|why| format!("{DECLINED}{why}"))
    }
}

fn doc_of(text: &str) -> Result<Value, String> {
    let whole: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    whole
        .get("doc")
        .cloned()
        .ok_or_else(|| "the compile made no document".to_owned())
}

/// `auditionTimeline` over the compiled document, or the TypeScript's sentence when the project has no such timeline.
fn audition(doc: Value, id: &str) -> Result<Value, String> {
    let has = doc["timelines"]
        .as_array()
        .is_some_and(|l| l.iter().any(|t| t["id"] == json!(id)));
    if !has {
        return Err(no_such_timeline(id));
    }
    let project: ProjectDoc = serde_json::from_value(doc).map_err(|e| e.to_string())?;
    serde_json::to_value(song::clips::audition_timeline(project, id)).map_err(|e| e.to_string())
}

impl Views for RealViews {
    fn mix_doc<'a>(
        &'a self,
        dir: &'a Path,
        timeline: Option<&'a str>,
    ) -> BoxFut<'a, Result<MixDoc, String>> {
        Box::pin(async move {
            let (me, project) = (self.clone_parts(), dir.to_string_lossy().into_owned());
            let text =
                on_stack(move || me.engine_text(&project))
                    .await
                    .map_err(|f| match f {
                        Failed::Panicked => "the compile panicked".to_owned(),
                        Failed::Lost => "the compile was lost".to_owned(),
                    })??;
            if text.starts_with("{\"failure\":") {
                return Err(format!("{DECLINED}compile"));
            }
            let doc = doc_of(&text)?;
            let doc = match timeline {
                Some(id) => audition(doc, id)?,
                None => doc,
            };
            Ok(MixDoc {
                name: doc["name"].as_str().unwrap_or_default().to_owned(),
                doc,
            })
        })
    }

    fn origins(&self, dir: &Path) -> Value {
        let project = dir.to_string_lossy().into_owned();
        let Ok(p) = self.prepared(&project, true) else {
            return json!({});
        };
        let root = p
            .ask
            .roots
            .confine
            .clone()
            .unwrap_or_else(|| project.clone());
        let r = k7_file_service::module_reads::Reading {
            home: &self.home,
            code_root: &self.docs.code_root,
            root: &root,
            active: Some(&project),
        };
        k7_file_service::module_reads::node_origins(&r).unwrap_or_else(|_| json!({}))
    }
}

impl RealViews {
    fn clone_parts(&self) -> RealViews {
        RealViews {
            docs: self.docs.clone(),
            home: self.home.clone(),
            log: self.log.clone(),
        }
    }
}
