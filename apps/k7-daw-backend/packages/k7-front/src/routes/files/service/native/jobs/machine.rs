//! machine.rs — a render asked of an engine machine over its jobs path, staged under the render cache and copied back (2320, 2320b).
//! What a reader gets wrong: the machine writes only under a root it may, so the files land in `staging` first and are moved after.

use std::path::{Path, PathBuf};
use std::time::Duration;

use k7_file_service::jobs::manifest::{Moved, move_into};
use serde_json::{Value, json};

use super::render::RenderOpts;
use crate::routes::engine_reach::job::{JOB_IDLE, JOB_TOTAL, of_type, request_job};
use crate::routes::files::service::native::engine::jobs::WsChannel;

/// A machine's render is let go of after this, as `MACHINE_RENDER_TIMEOUT_MS` is (2321).
pub const MACHINE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The folder (or the one file) a machine's render is made in, removed whatever happens to the request.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0).or_else(|_| std::fs::remove_file(&self.0));
    }
}

fn random_name() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the system's random source");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Renders `doc` on the machine at `url` and moves the files into `out_dir`; `renders` is the account's render cache.
pub async fn render_on(
    url: &str,
    renders: &Path,
    doc: &str,
    out_dir: &str,
    opts: &RenderOpts,
    total: Duration,
) -> Result<Moved, String> {
    // A folder on the MACHINE's disk means nothing here, and a guess would render silence.
    if opts.base.as_deref().is_some_and(|b| !b.is_empty()) {
        return Err("a render with a base folder needs an engine binary on this machine".into());
    }
    let jobs = renders.join("jobs");
    std::fs::create_dir_all(&jobs).map_err(|e| e.to_string())?;
    let staging = Staging(jobs.join(random_name()));
    let staged = staging.0.display().to_string();
    let doc: Value = serde_json::from_str(doc).map_err(|e| e.to_string())?;
    let body = json!({ "type": "render", "doc": doc, "outDir": staged, "stems": opts.stems });
    let mut chan = WsChannel::open(url).await?;
    let done = request_job(
        &mut chan,
        body.as_object().cloned().unwrap_or_default(),
        of_type("rendered"),
        |_| {},
        total,
        JOB_IDLE,
    )
    .await;
    crate::routes::engine_reach::job::JobChannel::close(&mut chan);
    let reply = Value::Object(done.map_err(|e| e.0)?);
    let out = out_dir.to_owned();
    // The copies are file work: off the runtime, with the staging kept alive until they end.
    tokio::task::spawn_blocking(move || {
        let moved = move_into(&reply, &staged, &out);
        drop(staging);
        moved
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Renders `doc` on the machine at `url` to exactly `out_wav`: asked for a name beside it, renamed once whole (2313).
pub async fn render_to_on(url: &str, doc: &str, out_wav: &Path) -> Result<(), String> {
    super::render::sweep_parts(out_wav);
    let part = PathBuf::from(format!("{}.part-{}", out_wav.display(), random_name()));
    let _gone = Staging(part.clone());
    let doc: Value = serde_json::from_str(doc).map_err(|e| e.to_string())?;
    let body = json!({ "type": "render", "doc": doc, "out": part.display().to_string() });
    let mut chan = WsChannel::open(url).await?;
    let done = request_job(
        &mut chan,
        body.as_object().cloned().unwrap_or_default(),
        of_type("rendered"),
        |_| {},
        JOB_TOTAL,
        JOB_IDLE,
    )
    .await;
    crate::routes::engine_reach::job::JobChannel::close(&mut chan);
    done.map_err(|e| e.0)?;
    tokio::fs::rename(&part, out_wav)
        .await
        .map_err(|e| e.to_string())
}
