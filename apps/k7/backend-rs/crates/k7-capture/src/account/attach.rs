//! attach.rs — what the account does to start a workspace's capture: the host's contract and attach; attaching them all
//! at start is `join.rs`. A workspace is local and never joins a relay.
//! What a reader gets wrong: the host's asks (`let_go_under`, the lock) are awaited here, never in a worker's step.

use std::any::Any;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use k7_file_service::paths::{basename, dirname, join};
use k7_file_service::workspaces::EXAMPLES;
use k7_replica::ops::K7x;

use super::out::{AcctOut, Recorder};
use super::{ACCOUNT_LIBRARY_ROOM, Account, Attached, lock};
use crate::capture::{Baseline, Options};
use crate::disk::{LiveLog, log_dir_of, replica_id};
use crate::source::Source;
use crate::worker::{Deps, Timing, Window, spawn};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The folder beside the workspaces that holds each one's open-project marker.
const MARKERS_DIR: &str = ".active-projects";

/// What the capture asks of the host that holds the workspaces: its side of a rename or a drop, the account's lock.
pub trait Bundle: Send + Sync + 'static {
    /// The paths a workspace's watcher heard, gathered per root (2125), with the root's generation.
    fn feed(&self, root: &str, paths: Vec<String>, generation: u64);
    /// The rewrite of old namespaces, awaited once before the attach (2150).
    fn migrate_namespaces(&self) -> BoxFuture<Result<(), String>>;
    /// Module watches and the served project under `root` let go, before a rename or a drop (2124).
    fn let_go_under(&self, root: &str) -> BoxFuture<Result<(), String>>;
    /// The account's write lock, held while a folder is renamed or removed (2153).
    fn lock(&self) -> BoxFuture<Box<dyn Any + Send>>;
    /// The set of workspace folders changed: what the listings kept is stale.
    fn folders_changed(&self);
}

/// What the account is given once: where things live and who answers for the rest.
pub struct Host {
    /// The studio home, whose `.sync/` folder holds every log.
    pub home: String,
    /// The folder holding the workspaces.
    pub workspaces: String,
    /// The account's own library: local to this device.
    pub library: String,
    pub source: Arc<dyn Source>,
    pub k7x: Arc<dyn K7x>,
    pub bundle: Arc<dyn Bundle>,
    pub timing: Timing,
}

impl Host {
    pub fn root_of(&self, name: &str) -> String {
        join(&[&self.workspaces, name])
    }

    pub fn marker_of(&self, root: &str) -> String {
        join(&[&dirname(root), MARKERS_DIR, &basename(root)])
    }
}

/// What a start reads off the disk before anything is built, all of it "not now" when it cannot be read (2102).
struct Prep {
    log_dir: String,
    replica: String,
}

fn prepare(home: &str, root: &str) -> io::Result<Prep> {
    let log_dir = log_dir_of(home, root);
    let replica = replica_id(home)?;
    Ok(Prep { log_dir, replica })
}

impl Account {
    pub(super) fn host(&self) -> Result<&Arc<Host>, String> {
        self.host
            .get()
            .ok_or_else(|| "the account has no host".to_string())
    }

    /// Start capturing the workspace at `root` (named `name` in its account) once; None for Examples; an unreadable head
    /// or id is an error, said by the caller, and nothing is written over it (2102).
    pub async fn attach(
        self: &Arc<Self>,
        root: &str,
        name: &str,
    ) -> Result<Option<Arc<Attached>>, String> {
        // Examples is each build's own, stocked on every device: capturing it would make every build a change.
        if name == EXAMPLES {
            return Ok(None);
        }
        let host = self.host()?.clone();
        let _one_at_a_time = self.attaching.lock().await;
        if let Some(had) = self.attached_at(root) {
            return Ok(Some(had));
        }
        let (home, r) = (host.home.clone(), root.to_string());
        let prep = tokio::task::spawn_blocking(move || prepare(&home, &r))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| format!("{root}: {e}")))?;
        self.start(&host, root, name, prep).await.map(Some)
    }

    async fn start(
        self: &Arc<Self>,
        host: &Arc<Host>,
        root: &str,
        name: &str,
        prep: Prep,
    ) -> Result<Arc<Attached>, String> {
        let mut o = Options::new(root, &prep.replica, host.k7x.clone());
        o.blob_dir = Some(format!("{}/blobs", prep.log_dir));
        // A workspace keeps its own history on this device, and diffs a restart against it.
        let mut records = name != ACCOUNT_LIBRARY_ROOM;
        let rooms = self.histories();
        let baseline: Option<Arc<dyn Baseline>> = if records {
            let b = self.baseline_for(&rooms, root, &prep.log_dir, &o).await;
            records = b.is_some();
            b
        } else {
            None
        };
        let out = Arc::new(AcctOut {
            log: LiveLog::new(&prep.log_dir),
            bundle: host.bundle.clone(),
            say: self.log.clone(),
            history: Mutex::new(records.then(|| Recorder {
                rooms: rooms.clone(),
                log_dir: prep.log_dir.clone(),
            })),
        });
        lock(&self.outs).insert(super::root_key(root), out.clone());
        let deps = Deps {
            source: host.source.clone(),
            out: out.clone(),
            timing: host.timing,
            baseline,
            hook: None,
        };
        let handle = spawn(o, deps, Arc::new(Window::default()))
            .await
            .map_err(|e| format!("{root}: {e}"))?;
        let a = self.insert(Attached {
            root: root.to_string(),
            name: name.to_string(),
            log_dir: prep.log_dir.clone(),
            handle,
            log: LiveLog::new(&prep.log_dir),
        });
        Ok(a)
    }
}
