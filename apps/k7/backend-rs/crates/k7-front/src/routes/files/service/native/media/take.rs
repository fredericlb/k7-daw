//! take.rs — `saveTake` held natively (2450 to 2453): a take and its picture checked against the project's record directory
//! by their real paths, and the landing wait of a mirrored take. What a reader gets wrong: the files stay where the
//! engine wrote them, in the record directory; nothing outside it is ever opened.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use k7_file_service::media::names::{Take as Named, inside_dir, outside_sentence, take_name};

use super::{Counts, Counts as C};

/// The landing wait's own numbers, the TypeScript's (2452).
pub const LANDING: Duration = Duration::from_secs(20);
pub const LOOK: Duration = Duration::from_millis(250);

/// What a take may be read from, and how long a mirrored one is waited for.
pub struct Cfg {
    pub roots: Vec<PathBuf>,
    pub landing: Duration,
    pub look: Duration,
}

/// What a save asks: the project's record directory and the files named under it.
pub struct Request {
    pub record_dir: String,
    pub file: Option<String>,
    pub video: Option<String>,
}

/// The first file's name in the record directory and its size.
#[derive(Debug, PartialEq)]
pub struct Saved {
    pub path: String,
    pub bytes: u64,
}

/// `file` is inside `dir` by their real paths: links are followed on both sides, so none leads out.
fn real_inside(dir: &Path, file: &Path) -> Option<PathBuf> {
    let (dir, file) = (dir.canonicalize().ok()?, file.canonicalize().ok()?);
    file.starts_with(&dir).then_some(file)
}

/// The record directory is in one of the account's roots (2451).
fn in_roots(dir: &str, roots: &[PathBuf]) -> bool {
    let Ok(real) = Path::new(dir).canonicalize() else {
        return false;
    };
    roots
        .iter()
        .filter_map(|r| r.canonicalize().ok())
        .any(|r| real.starts_with(r))
}

/// `takeHere`: the take as this project holds it, waiting for a mirrored one to land (size held between two looks).
async fn take_here(c: &Cfg, dir: &str, file: &str) -> Result<PathBuf, String> {
    let held = |p: &Path| real_inside(Path::new(dir), p).ok_or_else(|| outside_sentence(file));
    match take_name(dir, file) {
        Named::Refused => Err(outside_sentence(file)),
        Named::Here(f) => held(Path::new(&f)),
        Named::Land(here) => {
            let here = PathBuf::from(here);
            let until = Instant::now() + c.landing;
            let mut last: i64 = -1;
            loop {
                let size = std::fs::metadata(&here).map_or(-1, |m| m.len() as i64);
                if size > 0 && size == last {
                    return held(&here);
                }
                if Instant::now() >= until {
                    return Err(outside_sentence(file));
                }
                last = size;
                tokio::time::sleep(c.look).await;
            }
        }
    }
}

/// `saveTake`: the take, the picture, or each when the browser sent both apart; the first one's name and size.
pub async fn save_take(c: &Cfg, counts: &C, r: &Request) -> Result<Saved, String> {
    if r.file.as_deref().unwrap_or("").is_empty() && r.video.as_deref().unwrap_or("").is_empty() {
        return Err("nothing was recorded".into());
    }
    let dir = r.record_dir.as_str();
    let video = match r.video.as_deref().filter(|v| !v.is_empty()) {
        Some(v) if !(in_roots(dir, &c.roots) && inside_dir(dir, v)) => {
            return Err(outside_sentence(v));
        }
        Some(v) => {
            Some(real_inside(Path::new(dir), Path::new(v)).ok_or_else(|| outside_sentence(v))?)
        }
        None => None,
    };
    let file = match r.file.as_deref().filter(|f| !f.is_empty()) {
        Some(f) if !in_roots(dir, &c.roots) => return Err(outside_sentence(f)),
        Some(f) => Some(take_here(c, dir, f).await?),
        None => None,
    };
    let first = file.iter().chain(video.iter()).next();
    let Some(first) = first else {
        return Err("nothing was recorded".into());
    };
    let bytes = std::fs::metadata(first)
        .map_err(|e| format!("cannot read the take: {e}"))?
        .len();
    let path = first
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Counts::bump(&counts.takes);
    Ok(Saved { path, bytes })
}
