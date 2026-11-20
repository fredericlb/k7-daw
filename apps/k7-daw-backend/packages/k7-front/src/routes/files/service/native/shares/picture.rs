//! picture.rs — the picture's job: the browser, the settings, the mixdown, the media and the film written whole into the person's folder.
//! What a reader gets wrong: the scene is the text the writer made, spliced into the job unparsed; it is read only for three numbers and its media.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_file_service::shares::picture::{
    SYSTEM_CHROMIUM, VideoSettings, find_chromium, job_text, resolve_video_settings,
};
use serde_json::Value;

use super::disk::{Folder, real_inside};
use super::{Account, BoxFut, Ports, Shares};
use crate::routes::local::export::unique;

/// The most media a scene may name.
pub const MEDIA_LIMIT: usize = 10_000;
/// What a second render says while one runs.
pub const BUSY: &str = "a picture render is already running";

/// The browser: the real one is `k7_picture`, a test's writes a fixed film.
pub trait Browser: Send + Sync {
    fn render(&self, job: k7_picture::Job) -> BoxFut<'_, Result<k7_picture::Rendered, String>>;
}

pub struct Chromium;

impl Browser for Chromium {
    fn render(&self, job: k7_picture::Job) -> BoxFut<'_, Result<k7_picture::Rendered, String>> {
        Box::pin(k7_picture::render(job))
    }
}

/// What `picture.render` is asked: the project, the scene's text, the sound and the options.
pub struct Request<'a> {
    pub project: &'a Path,
    /// The folder the film lands in: the person's choice, never the project's.
    pub into: &'a Path,
    pub scene: &'a str,
    pub sound: &'a str,
    pub opts: &'a VideoSettings,
    pub script: PathBuf,
    pub debug: bool,
    /// `safePath`: a workspace-relative path to the file it names, or `None`.
    pub resolve: &'a (dyn Fn(&str) -> Option<PathBuf> + Sync),
}

/// Each `media/…` string of the scene with the file `map` names for its decoded path, or none to leave it out (2552).
pub fn media_by(scene: &Value, map: &dyn Fn(&str) -> Option<PathBuf>) -> HashMap<String, PathBuf> {
    let mut found = HashMap::new();
    let mut todo = vec![scene];
    while let Some(v) = todo.pop() {
        match v {
            Value::String(s) if s.starts_with("media/") && found.len() < MEDIA_LIMIT => {
                let rel = k7_picture::percent_decode(&s["media/".len()..]);
                if let Some(real) = map(&rel) {
                    found.insert(s.clone(), real);
                }
            }
            Value::Array(a) => todo.extend(a),
            Value::Object(m) => todo.extend(m.values()),
            _ => {}
        }
    }
    found
}

/// Each `media/…` string of the scene with the file it names, kept only when its real path is in the workspace (2552).
pub fn media_of(
    scene: &Value,
    root: &Path,
    resolve: &dyn Fn(&str) -> Option<PathBuf>,
) -> HashMap<String, PathBuf> {
    media_by(scene, &|rel| {
        resolve(rel).and_then(|abs| real_inside(root, &abs))
    })
}

fn number(scene: &Value, at: &[&str]) -> f64 {
    at.iter()
        .try_fold(scene, |v, k| v.get(k))
        .and_then(Value::as_f64)
        .unwrap_or(f64::NAN)
}

/// The folder the film goes to: it must exist, since the person chose it.
fn target_dir(into: &Path) -> Result<PathBuf, String> {
    if into.is_dir() {
        Ok(into.to_path_buf())
    } else {
        Err(format!("{} is not a folder", into.display()))
    }
}

/// `render_picture` with the media already mapped by the caller, whose gate is not the workspace's.
pub async fn render_with(
    shares: &Arc<Shares>,
    acct: &Account,
    ports: &Ports,
    req: Request<'_>,
    media: Option<HashMap<String, PathBuf>>,
) -> Result<String, String> {
    let Some(_slot) = shares.begin_picture() else {
        return Err(BUSY.into());
    };
    let env = |k: &str| acct.var(k);
    let exists = |p: &Path| p.exists();
    let list = |p: &Path| {
        let names = std::fs::read_dir(p).ok()?;
        Some(
            names
                .filter_map(|e| Some(e.ok()?.file_name().to_string_lossy().into_owned()))
                .collect(),
        )
    };
    if k7_mediatek::programs::NONE {
        return Err(format!(
            "a picture is not rendered here: it needs a browser started as a program, and {}",
            k7_mediatek::programs::WHY
        ));
    }
    let chromium = find_chromium(&env, &exists, &list, &SYSTEM_CHROMIUM)?;
    if !req.script.is_file() {
        return Err(format!(
            "the render page is not built ({})",
            req.script.display()
        ));
    }
    let scene: Value =
        serde_json::from_str(req.scene).map_err(|e| format!("the scene is not readable: {e}"))?;
    let vp = scene.get("viewport").and_then(Value::as_array);
    let at = |i: usize| {
        vp.and_then(|a| a.get(i))
            .and_then(Value::as_f64)
            .unwrap_or(f64::NAN)
    };
    let resolved = resolve_video_settings(
        req.opts,
        [at(0), at(1)],
        number(&scene, &["fps"]),
        number(&scene, &["durationSec"]),
    );
    let exports = target_dir(req.into)?;
    // The sound is the mixdown's, only when the project's sound is its mix.
    let mix_folder = if req.opts.audio && req.sound == "mix" {
        let doc = ports.views.mix_doc(req.project, None).await?;
        let folder = Folder::make(&acct.tmp, "tbmix-").map_err(|e| e.to_string())?;
        let manifest = ports.mixdown.render(&doc.doc, folder.path(), false).await?;
        Some((folder.path().join(&manifest.mix), folder))
    } else {
        None
    };
    let name = req
        .project
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let name = name.strip_suffix(".k7").unwrap_or(&name);
    let name = if name.is_empty() { "picture" } else { name };
    let job = k7_picture::Job {
        chromium,
        script: req.script,
        job: job_text(req.scene, req.opts, &resolved, mix_folder.is_some()),
        mix: mix_folder.as_ref().map(|(p, _)| p.clone()),
        media: media.unwrap_or_else(|| media_of(&scene, &acct.root, req.resolve)),
        out: exports.join(name),
        debug: req.debug,
        env: acct.env_pairs(),
        log: Arc::new(|_| {}),
    };
    let done = ports.browser.render(job).await?;
    shares.counts.declines.fetch_add(
        done.blocked.len() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );
    let ext = if done.mime.contains("mp4") {
        "mp4"
    } else {
        "webm"
    };
    // The film appears whole by a rename, under a name no file of the folder has.
    let part = done.into_part();
    let film = unique(&exports, &format!("{name}.{ext}"), Path::exists);
    if let Err(e) = std::fs::rename(&part, &film) {
        let _ = std::fs::remove_file(&part);
        return Err(e.to_string());
    }
    shares
        .counts
        .pictures
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(film.to_string_lossy().into_owned())
}
