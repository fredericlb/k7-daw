//! filetime.rs — a file's size and modification time in the units the tunnel compares them in: milliseconds
//! since the Unix epoch, as Node's `stat().mtimeMs` gives them. A download is stamped with the SERVER's mtime,
//! which is what lets the next comparison skip it.

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Info {
    pub is_file: bool,
    pub is_dir: bool,
    pub size: u64,
    pub mtime_ms: f64,
}

/// What is at `p`, or None when nothing is.
pub fn stat(p: &Path) -> Option<Info> {
    let m = std::fs::metadata(p).ok()?;
    let mtime_ms = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    Some(Info {
        is_file: m.is_file(),
        is_dir: m.is_dir(),
        size: m.len(),
        mtime_ms,
    })
}

/// Stamp `p` with a modification time in epoch milliseconds.
pub fn set_mtime_ms(p: &Path, ms: f64) {
    let at = UNIX_EPOCH + Duration::from_secs_f64(ms.max(0.0) / 1000.0);
    if let Ok(f) = std::fs::File::options().write(true).open(p) {
        let _ = f.set_modified(at);
    }
}

/// Two mtimes the same to within the rounding a filesystem applies.
pub fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 2.0
}
