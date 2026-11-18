//! prepare.rs — `prepare`: a clip's optimized copy and proxy made in the background, each field landed as its copy is finished (3f, M7).
//! What a reader gets wrong: the answer is `{started}` and the work outlives the page; a copy is made as `.part` and renamed, then the clip's field lands.

use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::response::Response;
use k7_file_service::edits::prepare::{Copy, clip_copy, land_clip_copy};
use k7_file_service::journal::Journals;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::verb_host::Error;
use k7_mediatek::video_copy::{CopyKind, copy_args, copy_path_of};
use serde_json::{Value, json};
use tokio::process::Command;

use super::media::no_ffmpeg;
use super::{Checked, failure, open_project};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::entries::{blocking, told};
use crate::routes::files::service::native::locks;
use crate::routes::files::service::native::runtime::Runtime;

/// The tags being prepared, by account: a second ask for the same source says so and starts nothing.
fn running() -> &'static Mutex<HashSet<(String, String)>> {
    static RUNNING: std::sync::OnceLock<Mutex<HashSet<(String, String)>>> =
        std::sync::OnceLock::new();
    RUNNING.get_or_init(Mutex::default)
}

/// Whether the account has a prepare running, for the tests (unix-only, as they are).
#[cfg(all(test, unix))]
pub(super) fn busy(account: &str) -> bool {
    running()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .any(|(a, _)| a == account)
}

/// How often a running copy renews the registry's lease, which a ten minute silence would let go.
const BEAT: Duration = Duration::from_secs(60);

fn bad(status: u16, message: impl Into<String>) -> Box<Response> {
    Box::new(failure(&Error::new(status, message)))
}

fn forbidden(shown: &str) -> Box<Response> {
    Box::new(failure(&k7_file_service::verb_host::gate::forbidden(shown)))
}

/// What a clip is asked to have made.
struct Ask {
    clip: String,
    source: String,
    optimized: Option<CopyKind>,
    proxy: bool,
}

fn parse(req: &Checked) -> Result<Ask, Box<Response>> {
    let text = |k: &str| match req.body.get(k) {
        Some(Value::String(s)) if !s.is_empty() && s.len() <= 4096 && !s.contains('\0') => {
            Ok(s.clone())
        }
        _ => Err(bad(400, format!("{k} must be a string"))),
    };
    let clip = text("clip")?;
    if clip == ".." || clip.contains(['/', '\\']) {
        return Err(bad(400, format!("'{clip}' is not a clip name")));
    }
    let optimized = match req.body.get("optimized") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s == "remux" => Some(CopyKind::Remux),
        Some(Value::String(s)) if s == "transcode" => Some(CopyKind::Transcode),
        Some(_) => return Err(bad(400, "optimized must be remux, transcode or null")),
    };
    let proxy = match req.body.get("proxy") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad(400, "proxy must be a boolean")),
    };
    if optimized.is_none() && !proxy {
        return Err(bad(400, "there is no copy to make"));
    }
    Ok(Ask {
        clip,
        source: text("source")?,
        optimized,
        proxy,
    })
}

/// The virtual path `rel` names from the project's, dots resolved; `None` when it climbs out of the root.
fn virtual_of(project: &str, rel: &str) -> Option<String> {
    let whole = if rel.starts_with('/') {
        rel.to_string()
    } else {
        format!("{project}/{rel}")
    };
    let mut out: Vec<&str> = Vec::new();
    for seg in whole.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s),
        }
    }
    Some(format!("/{}", out.join("/")))
}

/// One copy to make: the kind, the clip's field and the real file.
struct Step {
    kind: CopyKind,
    field: Copy,
    rel: String,
    dest: String,
}

/// `POST /verb-host/prepare {at, clip, source, optimized, proxy}`: `{started}`, false while that source is being prepared.
pub async fn prepare(req: &Checked, runtime: Option<Arc<Runtime>>) -> Response {
    let ask = match parse(req) {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let at = req.body.get("at").and_then(Value::as_str);
    let active = match open_project(&req.ctx, at).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let shown = req.virt.virt(&active);
    let Some(source_virtual) = virtual_of(&shown, &ask.source) else {
        return *forbidden(&ask.source);
    };
    let source = match req.read(&source_virtual) {
        Ok(s) if Path::new(&s).is_file() => s,
        Ok(_) => return *bad(404, format!("no such file: {}", ask.source)),
        Err(r) => return *r,
    };
    // The gate spells a path without `\\?\`, so the project is spelled the same way before they are compared.
    let project: std::path::PathBuf = std::fs::canonicalize(&active)
        .ok()
        .and_then(|p| k7_file_service::confine::spelled(&p))
        .unwrap_or_else(|| active.clone())
        .into();
    // A copy is written inside the project, beside the name the clip file gives its source.
    let step = |kind: CopyKind, field: Copy, name: &str| -> Result<Step, Box<Response>> {
        let rel = copy_path_of(&ask.source, name);
        if rel == ask.source {
            return Err(bad(
                400,
                format!("'{}' has no extension to hang a copy on", ask.source),
            ));
        }
        let dest = virtual_of(&shown, &rel)
            .ok_or_else(|| forbidden(&rel))
            .and_then(|v| req.read(&v))?;
        if !Path::new(&dest).starts_with(&project) {
            return Err(forbidden(&rel));
        }
        Ok(Step {
            kind,
            field,
            rel,
            dest,
        })
    };
    let optimized = match ask
        .optimized
        .map(|k| step(k, Copy::Optimized, "optimized"))
        .transpose()
    {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let proxy = match ask
        .proxy
        .then(|| step(CopyKind::Proxy, Copy::Proxy, "proxy"))
        .transpose()
    {
        Ok(s) => s,
        Err(r) => return *r,
    };
    // The proxy is made from whatever DECODES: the optimized copy when there is one, which is also the faster read.
    let held = {
        let (p, c) = (active.clone(), ask.clip.clone());
        blocking(move || clip_copy(&p, &c, Copy::Optimized))
            .await
            .flatten()
    };
    let held = held
        .and_then(|rel| virtual_of(&shown, &rel))
        .and_then(|v| req.read(&v).ok())
        .filter(|real| Path::new(real).is_file());
    let tag = format!("picture prepare {source}");
    let key = (req.ctx.account.clone(), tag.clone());
    if !running()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key.clone())
    {
        return json_answer(200, &json!({ "started": false }));
    }
    if runtime.as_ref().is_some_and(|r| !r.bg_start(&tag)) {
        running()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&key);
        return json_answer(200, &json!({ "started": false }));
    }
    let job = Job {
        ctx: req.ctx.clone(),
        active,
        clip: ask.clip,
        source,
        held,
        steps: [optimized, proxy].into_iter().flatten().collect(),
    };
    let ffmpeg = job.ffmpeg();
    tokio::spawn(async move {
        let mut work = Box::pin(job.run(&ffmpeg));
        let result = loop {
            tokio::select! {
                r = &mut work => break r,
                () = tokio::time::sleep(BEAT) => {
                    if let Some(r) = &runtime {
                        r.bg_progress(&tag, None, None);
                    }
                }
            }
        };
        match (&runtime, result) {
            (Some(r), Ok(line)) => r.bg_end(&tag, Some(&line), None),
            (Some(r), Err(e)) => r.bg_end(&tag, None, Some(&e)),
            (None, Ok(line)) => (job.ctx.log)(&format!("   ✓ {line}")),
            (None, Err(e)) => (job.ctx.log)(&format!("   ✗ {tag} failed: {e}")),
        }
        running()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&key);
    });
    json_answer(200, &json!({ "started": true }))
}

/// What the background task holds: the copies in order, and what to make the proxy from.
struct Job {
    ctx: crate::routes::files::service::native::Ctx,
    active: String,
    clip: String,
    source: String,
    held: Option<String>,
    steps: Vec<Step>,
}

impl Job {
    fn ffmpeg(&self) -> String {
        let bin = self.ctx.docs.as_ref().and_then(|d| d.env.get("FFMPEG_BIN"));
        bin.filter(|b| !b.is_empty())
            .cloned()
            .unwrap_or_else(|| "ffmpeg".into())
    }

    async fn run(&self, ffmpeg: &str) -> Result<String, String> {
        let mut decodable = self.held.clone().unwrap_or_else(|| self.source.clone());
        let mut made = Vec::new();
        for s in &self.steps {
            let from = if s.kind == CopyKind::Proxy {
                &decodable
            } else {
                &self.source
            };
            self.copy(ffmpeg, s, from).await?;
            self.land(s).await?;
            if s.kind != CopyKind::Proxy {
                decodable.clone_from(&s.dest);
            }
            made.push(s.field_name());
        }
        Ok(format!("{}: prepared ({})", self.clip, made.join(" + ")))
    }

    /// One copy as `.part`, renamed over its destination only when ffmpeg finished.
    async fn copy(&self, ffmpeg: &str, s: &Step, from: &str) -> Result<(), String> {
        let part = format!("{}.part", s.dest);
        let env = self
            .ctx
            .docs
            .as_ref()
            .map(|d| d.env.clone())
            .unwrap_or_default();
        if k7_mediatek::programs::NONE {
            return Err(no_ffmpeg().to_string());
        }
        let mut cmd = Command::new(ffmpeg);
        cmd.args(copy_args(s.kind, from, &part)).envs(&env);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let out = match cmd.spawn() {
            Ok(child) => child.wait_with_output().await.map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!("ffmpeg is not installed — {}", no_ffmpeg()));
            }
            Err(e) => Err(e.to_string()),
        };
        let made = match out {
            Ok(o) if o.status.success() => {
                std::fs::rename(&part, &s.dest).map_err(|e| e.to_string())
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                let tail: String = err
                    .trim()
                    .chars()
                    .rev()
                    .take(500)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                Err(format!(
                    "ffmpeg failed: {}",
                    if tail.is_empty() {
                        format!("exit {}", o.status)
                    } else {
                        tail
                    }
                ))
            }
            Err(e) => Err(e),
        };
        if made.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        made
    }

    /// The clip's field, set once its copy exists: under the journal's gate and the write lock, as a filed take is.
    async fn land(&self, s: &Step) -> Result<(), String> {
        let ctx = &self.ctx;
        let gate = k7_file_service::paths::resolve_all(&[&ctx.root]);
        let _gate = Journals::global().exclusive(&gate).await;
        let _lock = locks::write(&ctx.home.studio).await;
        let (project, clip, field, rel) = (
            self.active.clone(),
            self.clip.clone(),
            s.field,
            s.rel.clone(),
        );
        let landed =
            tokio::task::spawn_blocking(move || land_clip_copy(&project, &clip, field, &rel))
                .await
                .map_err(|_| "the landing was lost".to_string())??;
        if !landed.touched.is_empty() {
            Revisions::global().bump(Space::open(&ctx.home, &ctx.root).root());
        }
        if ctx.docs.is_some() {
            told(&ctx.root, "after").await;
        }
        Ok(())
    }
}

impl Step {
    fn field_name(&self) -> &'static str {
        match self.field {
            Copy::Optimized => "optimized",
            Copy::Proxy => "proxy",
        }
    }
}
