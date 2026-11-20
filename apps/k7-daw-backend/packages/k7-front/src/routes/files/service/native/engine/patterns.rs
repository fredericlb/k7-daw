//! The pattern watch (2228): a saved pattern the compile inlines is a recompile, 50 ms after the last event, off 1c's one
//! watcher of the workspace. What a reader gets wrong: the file is checked against `pattern_files` after the settle, not per event.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_file_service::filter::skip;
use k7_file_service::paths::resolve;
use k7_file_service::watch::Watch;

use super::held;
use super::queue::{Served, Session};

/// How long a burst of events on one save is let settle into one recompile.
const PATTERN_SETTLE: Duration = Duration::from_millis(50);

/// Whether a path under the project (`/`-spelt) is a pattern the watch carries.
pub fn carries_pattern(rel: &str) -> bool {
    rel.ends_with(".strudel") && !rel.split('/').any(skip)
}

impl Session {
    /// The pattern watch on the project (2228): a saved pattern the compile inlines is a recompile, 50 ms after the last event.
    pub fn watch_patterns(self: &Arc<Self>, root: &str, dir: &str) {
        held(&self.st).patterns = None;
        let Some(inner) = self.inner() else { return };
        let me = Arc::downgrade(self);
        let pending: Arc<Mutex<Vec<String>>> = Arc::default();
        let armed = Arc::new(AtomicBool::new(false));
        let (root, dir_owned) = (root.to_string(), dir.to_string());
        let rt = inner.rt.clone();
        let guard = Watch::global().listen(dir, move |rel| {
            if !carries_pattern(rel) {
                return;
            }
            held(&pending).push(rel.to_string());
            if armed.swap(true, Ordering::SeqCst) {
                return;
            }
            let (me, pending, armed) = (me.clone(), pending.clone(), armed.clone());
            let (root, dir) = (root.clone(), dir_owned.clone());
            rt.spawn(async move {
                tokio::time::sleep(PATTERN_SETTLE).await;
                armed.store(false, Ordering::SeqCst);
                let rels = std::mem::take(&mut *held(&pending));
                let Some(me) = me.upgrade() else { return };
                let Some(inner) = me.inner() else { return };
                let i = inner.clone();
                let d = dir.clone();
                let hit = tokio::task::spawn_blocking(move || {
                    let files = i.host.pattern_files(&d);
                    rels.iter().any(|r| files.contains(&resolve(&d, r)))
                })
                .await
                .unwrap_or(false);
                if hit {
                    let i = inner.clone();
                    let r = root.clone();
                    let _ = tokio::task::spawn_blocking(move || i.host.invalidate(&r)).await;
                    me.push_project(&Served { root, project: dir }).await;
                }
            });
        });
        held(&self.st).patterns = Some(Box::new(guard));
    }
}
