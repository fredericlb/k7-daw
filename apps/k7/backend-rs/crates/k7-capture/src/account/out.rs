//! out.rs — where a captured workspace's batches go: its log and its history room.
//! What a reader gets wrong: the log is this machine's record only, and a room that cannot write never stops the capture.

use std::sync::{Arc, Mutex};

use k7_replica::blob::BlobId;
use serde_json::Value;

use super::attach::Bundle;
use super::lock;
use crate::disk::LiveLog;
use crate::history::Histories;
use crate::worker::Out;

/// Where a capture's batches go: its log, then its history.
pub(super) struct AcctOut {
    pub log: LiveLog,
    pub bundle: Arc<dyn Bundle>,
    pub say: Arc<dyn Fn(&str) + Send + Sync>,
    /// Where the batches are also ordered into the workspace's own history, when no relay orders them.
    pub history: Mutex<Option<Recorder>>,
}

/// The device's room a batch is recorded in: the rooms and the workspace's log folder that keys its room.
#[derive(Clone)]
pub(super) struct Recorder {
    pub rooms: Arc<Histories>,
    pub log_dir: String,
}

impl AcctOut {
    /// The batch in the workspace's own history: a room that cannot write is said, and never keeps an edit from the log.
    fn record(&self, root: &str, batch: &Value) {
        let Some(r) = lock(&self.history).clone() else {
            return;
        };
        if let Err(e) = r.rooms.record(&r.log_dir, batch) {
            (self.say)(&format!("   ✗ history: {root}: {e}"));
        }
    }
}

impl Out for AcctOut {
    fn batch(&self, root: &str, batch: Value) {
        // The log is this machine's record only: one it cannot write must not keep the batch from the history.
        if let Err(e) = self.log.append(&batch) {
            (self.say)(&format!("   ✗ live log: {root}: {e}"));
        }
        self.record(root, &batch);
    }

    // No relay wants a blob: the capture's missing list is for nobody here.
    fn missing(&self, _: &str, _: Vec<BlobId>) {}

    fn note(&self, _: &str, line: &str) {
        (self.say)(line);
    }

    fn feed(&self, root: &str, paths: Vec<String>, generation: u64) {
        self.bundle.feed(root, paths, generation);
    }
}
