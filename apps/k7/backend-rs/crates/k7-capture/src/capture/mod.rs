//! capture — every change to a workspace's files, whoever made it, as batches of ops (`liveCapture.ts`, rule for rule).
//! Synchronous over a `Clock`: the timers, the watcher and the window's epoch are the worker's (C3), which calls the
//! `on_*` handlers below. What a reader gets wrong: a file written for ANOTHER replica reaches the ledger after it landed.

mod blobs;
mod deferred;
mod diff;
mod learn;
mod walk;
mod write;

use std::io;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use k7_file_service::filter;
use k7_file_service::paths::{dirname, join};
use k7_replica::blob::BlobId;
use k7_replica::ops::K7x;
use serde_json::Value;

use crate::cache::BlobCache;
use crate::confirmed::ConfirmedDir;
use crate::disk::confined_path;
use crate::index::BlobIndex;
use crate::ledger::Known;
pub use blobs::Fetched;
use deferred::Deferred;
pub use diff::{DIRTY_SWEEP, Step};
pub use walk::{files_under, through_link};

/// The time the capture reads, so a test moves it by hand.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> f64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> f64 {
        let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        since.map_or(0.0, |d| d.as_millis() as f64)
    }
}

/// The text each file had as the room last confirmed it: what a restart diffs the workspace against.
pub trait Baseline: Send + Sync {
    fn files(&self) -> io::Result<Vec<String>>;
    fn get(&self, file: &str) -> io::Result<Option<String>>;
}

impl Baseline for ConfirmedDir {
    fn files(&self) -> io::Result<Vec<String>> {
        k7_replica::Confirmed::files(self)
    }

    fn get(&self, file: &str) -> io::Result<Option<String>> {
        k7_replica::Confirmed::get(self, file)
    }
}

pub struct Options {
    pub root: String,
    pub replica: String,
    /// Hidden from capture, by NAME of any path segment: the workspace's own `skip`.
    pub skip: fn(&str) -> bool,
    /// Never read as text: known by its blob ref.
    pub is_binary: fn(&str) -> bool,
    /// The local blob cache; without one a binary still has a ref, but no bytes are kept to send or write.
    pub blob_dir: Option<String>,
    /// Where the binaries' hashes are kept across restarts; beside the blob cache's folder by default.
    pub index_file: Option<String>,
    /// A binary modified more recently than this is still being written: it waits rather than travel half-written.
    pub binary_settle_ms: f64,
    /// How long after its last change, write or start a workspace counts as active, and keeps the fast net.
    pub active_ms: f64,
    /// The idle re-scan's spacing; by default 5 minutes with a watcher up, 60 s without.
    pub idle_reconcile_ms: Option<f64>,
    pub clock: Arc<dyn Clock>,
    /// Makes a batch's id; a test makes it fail.
    pub new_id: fn() -> io::Result<String>,
    pub k7x: Arc<dyn K7x>,
}

impl Options {
    pub fn new(root: &str, replica: &str, k7x: Arc<dyn K7x>) -> Self {
        Options {
            root: root.to_string(),
            replica: replica.to_string(),
            skip: filter::skip,
            is_binary: filter::is_binary,
            blob_dir: None,
            index_file: None,
            binary_settle_ms: 1000.0,
            active_ms: 2.0 * 60_000.0,
            idle_reconcile_ms: None,
            clock: Arc::new(SystemClock),
            new_id: crate::disk::uuid_v4,
            k7x,
        }
    }
}

/// The quiet period closes a batch after this; the worker arms it (`quiet_armed`).
pub const QUIET_MS: u64 = 100;
/// The re-scan of an active workspace, of an idle one, of an idle one whose watcher is up.
pub const RECONCILE_MS: u64 = 5000;
const IDLE_RECONCILE_MS: f64 = 60_000.0;
const IDLE_WATCHED_MS: f64 = 5.0 * 60_000.0;

pub struct LiveCapture {
    pub(super) o: Options,
    pub(super) ledger: IndexMap<String, Known>,
    pub(super) dirty: IndexSet<String>,
    pub(super) last_full: f64,
    pub(super) last_active: f64,
    pub(super) window_closed: bool,
    pub(super) labels: Vec<String>,
    pub(super) local: u64,
    pub(super) cache: Option<BlobCache>,
    pub(super) index: Option<BlobIndex>,
    pub(super) young: IndexSet<String>,
    pub(super) young_due: Option<f64>,
    pub(super) quiet_armed: bool,
    pub(super) generation: u64,
    pub(super) fed: Vec<String>,
    pub(super) prepared: bool,
    pub(super) halted: bool,
    pub(super) watching: bool,
    pub(super) baseline: Option<Arc<dyn Baseline>>,
    pub(super) deferred: Deferred,
    pub(super) emitted: Vec<Value>,
    pub(super) missing: Vec<BlobId>,
    pub(super) reads: Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>>,
    pub(super) work: u64,
    /// Files written without a flush (`write_known_batched`), until `take_unsynced` hands them to the durability pass.
    pub(super) unsynced: Vec<std::path::PathBuf>,
}

impl LiveCapture {
    pub fn new(o: Options) -> Self {
        let index_file = o.index_file.clone().or_else(|| {
            o.blob_dir
                .as_ref()
                .map(|d| join(&[&dirname(d), "blob-index.json"]))
        });
        let now = o.clock.now_ms();
        LiveCapture {
            cache: o.blob_dir.as_deref().map(BlobCache::new),
            index: index_file.as_deref().map(BlobIndex::open),
            deferred: Deferred::new(&o.root),
            ledger: IndexMap::new(),
            dirty: IndexSet::new(),
            last_full: 0.0,
            last_active: now,
            window_closed: false,
            labels: vec![],
            local: 0,
            young: IndexSet::new(),
            young_due: None,
            quiet_armed: false,
            generation: 0,
            fed: vec![],
            prepared: false,
            halted: false,
            watching: false,
            baseline: None,
            emitted: vec![],
            missing: vec![],
            reads: None,
            work: 0,
            unsynced: Vec::new(),
            o,
        }
    }

    pub fn root(&self) -> &str {
        &self.o.root
    }

    /// Bumped by every reported path: only its changes matter, never its value.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Stopped for good: a deleted workspace's end must stay its last batch.
    pub fn stopped(&self) -> bool {
        self.halted
    }

    /// Whether the watcher is up: a generation that stands still means something only then.
    pub fn watching(&self) -> bool {
        self.watching && !self.halted
    }

    /// The worker tells the capture whether its watcher is up, which sets the idle re-scan's spacing.
    pub fn set_watching(&mut self, up: bool) {
        self.watching = up;
    }

    /// The workspace file's absolute path; refuses a path that would leave the workspace (`livePath.ts`).
    pub fn path_of(&self, rel: &str) -> io::Result<String> {
        if rel.is_empty() {
            return Ok(self.o.root.clone());
        }
        confined_path(&self.o.root, rel).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }

    /// `path_of` for a write: refused when a folder on the way is a link, which would land the file outside the workspace.
    pub(super) fn write_path_of(&self, rel: &str) -> io::Result<String> {
        let abs = self.path_of(rel)?;
        if !rel.is_empty() && through_link(&self.o.root, rel)? {
            let why = format!("refused write {rel}: a folder on its way is a link");
            return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
        }
        Ok(abs)
    }

    pub(super) fn now(&self) -> f64 {
        self.o.clock.now_ms()
    }

    /// Every path reported since the last call, in order: the feed's input (2125).
    pub fn take_paths(&mut self) -> Vec<String> {
        std::mem::take(&mut self.fed)
    }

    /// The batches emitted since the last call, as the TypeScript's `onBatch` hears them.
    pub fn take_batches(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.emitted)
    }

    /// The blobs a written ref asked for since the last call, as `onMissingBlob` hears them.
    pub fn take_missing(&mut self) -> Vec<BlobId> {
        std::mem::take(&mut self.missing)
    }

    /// What was said since the last call, for the worker to log.
    pub fn take_notes(&mut self) -> Vec<String> {
        self.deferred.take_notes()
    }

    /// True while a window closed since the last full sweep: files it wrote are not captured yet.
    pub fn window_closed(&self) -> bool {
        self.window_closed
    }

    /// Whether the quiet timer should be (re)armed for `QUIET_MS`.
    pub fn quiet_armed(&self) -> bool {
        self.quiet_armed
    }

    /// When the young binaries are to be looked at again, if any wait.
    pub fn young_due(&self) -> Option<f64> {
        self.young_due
    }

    /// Ledger entries visited (counted where the loops visit them) and files changed so far: a test counts work, never time.
    #[doc(hidden)]
    pub fn work(&self) -> u64 {
        self.work
    }

    /// Count the workspace files read from now on; `take_reads` answers them.
    pub fn track_reads(&mut self) {
        self.reads = Some(Default::default());
    }

    /// The list `track_reads` fills, for a test that counts it from a closure that cannot reach the capture.
    #[doc(hidden)]
    pub fn reads_probe(&self) -> Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>> {
        self.reads.clone()
    }

    pub fn take_reads(&mut self) -> Vec<String> {
        self.reads
            .as_ref()
            .map(|r| std::mem::take(&mut *r.lock().unwrap_or_else(|e| e.into_inner())))
            .unwrap_or_default()
    }

    /// Stopped: the index is saved and the worker closes the watcher and the timers.
    pub fn stop(&mut self) {
        self.halted = true;
        let now = self.now();
        if let Some(i) = self.index.as_mut() {
            let _ = i.save(&self.ledger, now);
        }
        self.quiet_armed = false;
        self.young_due = None;
    }

    /// A mutation window closed (the worker counts them, 2121); its label names the batch the next quiet period emits.
    pub fn release(&mut self, label: &str) {
        if !label.is_empty() {
            self.labels.push(label.to_string());
        }
        self.window_closed = true;
        self.schedule();
    }

    /// One path moved: a file, a folder (created, renamed or removed whole), or something gone.
    pub fn touch(&mut self, rel: &str) {
        if rel.split('/').any(self.o.skip) {
            return;
        }
        self.last_active = self.now();
        self.dirty.insert(rel.to_string());
        self.publish(rel);
        self.schedule();
    }

    pub(super) fn schedule(&mut self) {
        self.quiet_armed = true;
    }

    /// Tell the feed `rel` moved.
    pub(super) fn publish(&mut self, rel: &str) {
        self.generation += 1;
        self.fed.push(rel.to_string());
    }

    /// The quiet timer fired: whether a flush is due, and whether it sweeps whole.
    pub fn quiet_plan(&mut self) -> Option<bool> {
        self.quiet_armed = false;
        Some(self.window_closed)
    }

    /// The re-scan timer fired: whether a sweep is due (an idle workspace sweeps seldom).
    pub fn sweep_due(&mut self, force: bool) -> bool {
        let now = self.now();
        let idle = now - self.last_active >= self.o.active_ms;
        let spacing = self.o.idle_reconcile_ms.unwrap_or(if self.watching {
            IDLE_WATCHED_MS
        } else {
            IDLE_RECONCILE_MS
        });
        force || !idle || now - self.last_full >= spacing
    }

    /// The quiet timer fired, no window open: emit what the watcher reported.
    pub fn on_quiet(&mut self) -> Option<Value> {
        let full = self.quiet_plan()?;
        self.flush_from_timer(full)
    }

    /// The re-scan timer fired, no window open.
    pub fn on_sweep(&mut self, force: bool) -> Option<Value> {
        if self.sweep_due(force) {
            self.flush_from_timer(true)
        } else {
            None
        }
    }

    /// Milliseconds until the young binaries are to be looked at again, if any wait.
    pub fn until_young(&self) -> Option<f64> {
        self.young_due.map(|due| (due - self.now()).max(0.0))
    }

    /// Labels of the windows closed since the last batch, which name the next one.
    pub fn add_labels(&mut self, labels: Vec<String>) {
        self.labels.extend(labels);
    }

    /// The young binaries' timer fired: each is reported again, now that it had time to settle.
    pub fn on_young(&mut self) {
        self.young_due = None;
        let files: Vec<String> = self.young.drain(..).collect();
        for rel in files {
            self.touch(&rel);
        }
    }

    /// A scan that failed: what it could not do is said, and the next flush sweeps everything.
    pub fn step_failed(&mut self, e: &io::Error) {
        self.window_closed = true;
        self.deferred
            .say(format!("   ✗ live capture: {}: {e}", self.o.root));
    }

    /// A flush no caller awaits: what it cannot do is said, and the next one sweeps everything.
    fn flush_from_timer(&mut self, full: bool) -> Option<Value> {
        match self.flush_now(full) {
            Ok(batch) => batch,
            Err(e) => {
                self.step_failed(&e);
                None
            }
        }
    }
}
