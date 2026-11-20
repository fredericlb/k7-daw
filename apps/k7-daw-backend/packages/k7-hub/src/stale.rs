//! stale.rs — purging what the accounts' caches hold that nobody used for a month:
//! automatic renders (`.renders`) and undo journals (`.history`).
//! What a reader gets wrong: "used" is the later of atime and mtime, since a compile marks a render by its atime.

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

/// What nobody used for this long is deleted.
pub const CACHE_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
/// How often the hub sweeps.
pub const CACHE_SWEEP_EVERY: Duration = Duration::from_secs(3600);

fn list(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn ms(t: std::io::Result<std::time::SystemTime>) -> u64 {
    t.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

fn last_used(path: &Path) -> Option<u64> {
    let m = std::fs::metadata(path).ok()?;
    Some(ms(m.accessed()).max(ms(m.modified())))
}

fn remove(path: &Path) -> bool {
    let gone = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    gone.is_ok() || !path.exists()
}

/// Delete every account's render unused for `ttl`; how many went.
pub fn purge_stale_renders(data_root: &Path, now: u64, ttl: Duration) -> usize {
    let ttl = ttl.as_millis() as u64;
    let mut removed = 0;
    for account in list(data_root) {
        let cache = data_root.join(account).join(".renders");
        for name in list(&cache) {
            let at = cache.join(name);
            if last_used(&at).is_some_and(|t| now.saturating_sub(t) >= ttl) && remove(&at) {
                removed += 1;
            }
        }
    }
    removed
}

/// Delete every project journal unused for `ttl`, and any older blob of a live one; how many went.
pub fn purge_stale_history(data_root: &Path, now: u64, ttl: Duration) -> usize {
    let ttl = ttl.as_millis() as u64;
    let stale = |t: u64| now.saturating_sub(t) >= ttl;
    let mut removed = 0;
    for account in list(data_root) {
        let home = data_root.join(account).join(".history");
        for project in list(&home) {
            let dir = home.join(project);
            // A journal is used by every edit, undo and redo, which rewrite it.
            let at = last_used(&dir.join("journal.json")).or_else(|| last_used(&dir));
            if at.is_some_and(stale) {
                if remove(&dir) {
                    removed += 1;
                }
                continue;
            }
            for blob in list(&dir.join("blobs")) {
                let file = dir.join("blobs").join(blob);
                if last_used(&file).is_some_and(stale) && remove(&file) {
                    removed += 1;
                }
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purges_what_is_older_than_the_rule_only() {
        let root = std::env::temp_dir().join(format!("k7-hub-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let renders = root.join("a-1/.renders");
        std::fs::create_dir_all(renders.join("old")).unwrap();
        let journal = root.join("a-1/.history/p");
        std::fs::create_dir_all(journal.join("blobs")).unwrap();
        std::fs::write(journal.join("journal.json"), "{}").unwrap();
        std::fs::write(journal.join("blobs/b"), "x").unwrap();
        let now = crate::account::now_ms();
        assert_eq!(purge_stale_renders(&root, now, CACHE_TTL), 0);
        assert_eq!(purge_stale_history(&root, now, CACHE_TTL), 0);
        let later = now + CACHE_TTL.as_millis() as u64 + 1000;
        assert_eq!(purge_stale_renders(&root, later, CACHE_TTL), 1);
        assert_eq!(purge_stale_history(&root, later, CACHE_TTL), 1);
        assert!(!journal.exists() && !renders.join("old").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
