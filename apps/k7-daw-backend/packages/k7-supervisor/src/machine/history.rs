//! history.rs — the undo audio servers asked this machine to keep (`apps/k7-daw-backend/docs/files/history.md`):
//! a month after its last use it goes. Only `history-*` roots are the purge's: a workspace copy is a cache the
//! tunnel manages, and a symbolic link is never followed or counted.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How long this machine keeps an undo's audio after it was last kept or sent back.
pub const KEPT_HISTORY_TTL: Duration = Duration::from_secs(24 * 3600 * 30);

fn dirs(of: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(of)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect()
}

fn files_below(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => files_below(&e.path(), out),
            Ok(t) if t.is_file() => out.push(e.path()),
            _ => {}
        }
    }
}

/// Delete every file under `<remote_root>/<server>/history-*` unused for `ttl`; how many went.
pub fn purge_kept_history(remote_root: &Path, ttl: Duration) -> usize {
    // Listed first, removed after: a walk over a folder being emptied is not one to trust.
    let now = SystemTime::now();
    let mut all = Vec::new();
    for server in dirs(remote_root) {
        for root in dirs(&server) {
            if root
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("history-"))
            {
                files_below(&root, &mut all);
            }
        }
    }
    all.into_iter()
        .filter(|f| {
            std::fs::metadata(f)
                .and_then(|m| m.modified())
                .is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age >= ttl))
        })
        .filter(|f| std::fs::remove_file(f).is_ok())
        .count()
}
