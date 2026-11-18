//! jobs.rs — the staged outputs of jobs, which `adopt` moves into a workspace: swept when a day old (3-B8).
//! What a reader gets wrong: a job folder's age is its newest file's, so an output still being written keeps it.

use std::time::{Duration, SystemTime};

use super::plan::JOB_OUT;
use crate::paths::join;

/// How long a job's staged output is kept.
pub const KEEP: Duration = Duration::from_secs(24 * 3600);

fn newest(dir: &str) -> Option<SystemTime> {
    let mut t = std::fs::metadata(dir).and_then(|m| m.modified()).ok()?;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        if let Some(m) = e.metadata().ok().and_then(|m| m.modified().ok()) {
            t = t.max(m);
        }
    }
    Some(t)
}

/// Removes every job folder under the account's home whose newest file is older than `keep`; how many went.
pub fn sweep(studio: &str, now: SystemTime, keep: Duration) -> usize {
    let root = join(&[studio, JOB_OUT]);
    let Ok(read) = std::fs::read_dir(&root) else {
        return 0;
    };
    let mut gone = 0;
    for e in read.flatten() {
        let dir = join(&[&root, &e.file_name().to_string_lossy()]);
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let old = newest(&dir).is_some_and(|t| now.duration_since(t).is_ok_and(|age| age > keep));
        if old && std::fs::remove_dir_all(&dir).is_ok() {
            gone += 1;
        }
    }
    gone
}
