//! account — the workspaces one account captures: each one's worker, its log and its history; and what the account does
//! with them as a whole. No workspace joins a relay room (docs/files/replica.md).
//! What a reader gets wrong: the account owns no `Workspace`; the host (the front's bundle) tells it what to attach.

mod attach;
mod folders;
mod history;
mod history_ask;
mod join;
mod land;
mod out;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::disk::LiveLog;
use crate::worker::Handle;
pub use attach::{BoxFuture, Bundle, Host};
pub use history_ask::Ask;

/// The one spelling a root is keyed by: a handler's `C:\x` and a slashed `C:/x` are the same workspace (docs/file-capture.md).
pub fn root_key(root: &str) -> String {
    k7_file_service::paths::resolve_all(&[root])
}

/// The room the account's own library was captured as; `_` starts no workspace name, so none can take it.
pub const ACCOUNT_LIBRARY_ROOM: &str = "_account";

/// One captured workspace: its worker and where its log lives.
pub struct Attached {
    pub root: String,
    /// Its folder's name here.
    pub name: String,
    pub log_dir: String,
    pub handle: Handle,
    pub log: LiveLog,
}

/// The account's captures by root.
pub struct Account {
    attached: Mutex<HashMap<String, Arc<Attached>>>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    host: OnceLock<Arc<Host>>,
    /// One attach at a time: two for one root would be two workers over one folder.
    attaching: tokio::sync::Mutex<()>,
    /// Where each root's batches go, kept to start and stop a root's history record.
    outs: Mutex<HashMap<String, Arc<out::AcctOut>>>,
    /// The workspaces' own histories, kept on this device.
    history: OnceLock<Arc<crate::history::Histories>>,
    /// Why a workspace's history room could not start, by root: its capture goes on without one.
    history_errors: Mutex<HashMap<String, String>>,
    /// The rooms' cadence: entries per rolled segment and per snapshot; a test makes them small.
    cadence: (f64, f64),
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Account {
    pub fn new(log: Arc<dyn Fn(&str) + Send + Sync>) -> Account {
        Account {
            attached: Mutex::default(),
            log,
            host: OnceLock::new(),
            attaching: tokio::sync::Mutex::new(()),
            outs: Mutex::default(),
            history: OnceLock::new(),
            history_errors: Mutex::default(),
            cadence: (crate::history::ROLL_EVERY, crate::history::SNAPSHOT_EVERY),
        }
    }

    /// The device's rooms roll a segment every `roll` entries and snapshot every `snapshot`; before `bind`.
    pub fn with_cadence(mut self, roll: f64, snapshot: f64) -> Account {
        self.cadence = (roll, snapshot);
        self
    }

    /// Who the account asks for what it does not hold itself; once, before the first attach.
    pub fn bind(&self, host: Host) {
        let _ = self.host.set(Arc::new(host));
    }

    pub fn insert(&self, a: Attached) -> Arc<Attached> {
        let a = Arc::new(a);
        lock(&self.attached).insert(root_key(&a.root), a.clone());
        a
    }

    /// The captured workspace at `root`, whichever way the caller spells it (`C:\x`, `C:/x`, a trailing slash).
    pub fn attached_at(&self, root: &str) -> Option<Arc<Attached>> {
        lock(&self.attached).get(&root_key(root)).cloned()
    }

    fn say(&self, line: &str) {
        (self.log)(line);
    }

    /// Stop capturing `a` for good: its capture and its history; a workspace of that name later starts afresh.
    pub async fn forget(&self, a: &Attached) {
        a.handle.stop().await;
        self.histories().release(&a.log_dir);
        lock(&self.attached).remove(&root_key(&a.root));
        let dir = a.log_dir.clone();
        let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(dir)).await;
    }

    /// The workspace at `root` is deleted here: its capture, its history and its log go; false when nothing captures it.
    pub async fn end_workspace(&self, root: &str) -> bool {
        let Some(a) = self.attached_at(root) else {
            return false;
        };
        self.forget(&a).await;
        true
    }

    /// Stop capturing the workspace at `root`, which is gone or no longer served; its history stays.
    pub async fn detach(&self, root: &str) {
        let Some(a) = lock(&self.attached).remove(&root_key(root)) else {
            return;
        };
        a.handle.stop().await;
        self.histories().release(&a.log_dir);
    }

    /// Every capture stopped, its ledger saved: the process holding them ends.
    pub async fn stop_all(&self) {
        let all: Vec<Arc<Attached>> = lock(&self.attached).values().cloned().collect();
        for a in all {
            a.handle.stop().await;
        }
    }
}
