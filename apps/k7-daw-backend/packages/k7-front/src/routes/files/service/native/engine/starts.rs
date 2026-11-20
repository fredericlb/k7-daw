//! The engine started at the open, and the project pushed after a write: the `startEngine` and `push` ops.
//! What a reader gets wrong: the project is the marker's, read from the disk, so no memory is asked.

use serde_json::Value;

use super::Host;
use super::routes::Engine;
use crate::routes::files::service::native::entries::blocking;
use k7_file_service::space::Space;

/// How long a push waits for the start it queued, so the revision a verb answers is read after the project's own work.
const START_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

impl Engine {
    /// `startEngineForActive`: the active project's module folders watched and the session told to start it, if there is one.
    pub async fn start_active(&self, root: &str) {
        self.started(root, false).await;
    }

    /// The same, answering once the session has run the start when `wait` is set, as far as `START_WAIT` allows.
    async fn started(&self, root: &str, wait: bool) {
        let home = self.host.home_now();
        let r = root.to_string();
        let found = blocking(move || Space::open(&home, &r).active().ok().flatten()).await;
        let Some(project) = found.flatten() else {
            return;
        };
        let root = root.to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.link.session(Some(&root)).enqueue(move |s| async move {
            s.start(&root, &project).await;
            let _ = tx.send(());
        });
        if wait {
            let _ = tokio::time::timeout(START_WAIT, rx).await;
        }
    }

    /// A write landed inside the open project: its compiled document dropped, then the project started again.
    pub async fn push_active(&self, root: &str) {
        self.host.invalidate(root);
        self.started(root, true).await;
    }

    /// The `startEngine` op: the engine pointed at the active project unless this is a tree open, and the workspace's revision.
    pub async fn start_engine(&self, root: &str, start: bool) -> Value {
        if start {
            self.start_active(root).await;
        }
        let (host, r) = (self.host.clone(), root.to_string());
        let rev = blocking(move || host.revision(&r)).await;
        Value::String(rev.unwrap_or_default())
    }

    /// The load the link holds for the workspace's active project, without waiting for a queued start.
    pub async fn load_now(&self, root: &str) -> Value {
        let (home, r) = (self.host.home_now(), root.to_string());
        let active = blocking(move || Space::open(&home, &r).active().ok().flatten())
            .await
            .flatten();
        self.link
            .session(Some(root))
            .engine_load_of(active.as_deref())
            .map_or(Value::Null, |p| Value::String(p.as_str().to_string()))
    }
}
