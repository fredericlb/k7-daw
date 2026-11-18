//! history — a workspace's own history on its device (`docs/files/history.md`, "History on the device"): the capture's
//! batches ordered in a `k7_sync` room kept inside the workspace's log folder, so the log, the segments and the restore are
//! the relay's own code. What a reader gets wrong: the room is keyed by the log folder, which is what a rename moves.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde_json::{Map, Value, json};

use k7_sync::relay::{LiveRoom, Options, Relay};

use crate::capture::Baseline;

/// The room's name in its folder: one room per log folder.
const ROOM: &str = "history";

/// The device's rooms: one `k7_sync` relay with no socket, each workspace's room under its own log folder.
pub struct Histories {
    relay: Relay,
    started: OnceLock<()>,
    say: Arc<dyn Fn(&str) + Send + Sync>,
}

/// The working log rolls into a segment after this many entries, so the folder is sent a small log and only new segments.
pub const ROLL_EVERY: f64 = 20.0;
/// A snapshot of every file whole is kept after this many entries (the server's own cadence).
pub const SNAPSHOT_EVERY: f64 = 1000.0;

impl Histories {
    pub fn new(say: Arc<dyn Fn(&str) + Send + Sync>) -> Histories {
        Histories::with(say, ROLL_EVERY, SNAPSHOT_EVERY)
    }

    /// Rooms rolling every `roll` entries and snapshotting every `snapshot`; a test makes both small.
    pub fn with(say: Arc<dyn Fn(&str) + Send + Sync>, roll: f64, snapshot: f64) -> Histories {
        let mut o = Options::new(Arc::new(|slug: &str| PathBuf::from(slug)));
        o.snapshot_every = roll;
        o.history_every = snapshot;
        let said = say.clone();
        o.log = Arc::new(move |m| said(&format!("history | {m}")));
        Histories {
            relay: Relay::new(o, crate::native()),
            started: OnceLock::new(),
            say,
        }
    }

    /// A line for the host's log.
    pub fn say(&self, line: &str) {
        (self.say)(&format!("history | {line}"));
    }

    /// The idle timer (the automatic checkpoint), once, on the current runtime.
    pub fn start(&self) {
        self.started.get_or_init(|| self.relay.start());
    }

    pub fn relay(&self) -> &Relay {
        &self.relay
    }

    /// The room of the workspace whose capture log is `log_dir`, loaded on first use.
    pub fn room(&self, log_dir: &str) -> Result<LiveRoom, String> {
        self.relay.room(log_dir, ROOM)
    }

    /// The room's own folder.
    pub fn dir(&self, log_dir: &str) -> PathBuf {
        k7_sync::path::room_dir(Path::new(log_dir), ROOM)
    }

    /// The room saved and let go of, before its folder moves or goes.
    pub fn release(&self, log_dir: &str) {
        self.relay.release(log_dir, ROOM);
    }

    /// Whether the room has any entry or file yet; a room that cannot be loaded is an error, never "empty".
    pub fn is_empty(&self, log_dir: &str) -> Result<bool, String> {
        let r = self.room(log_dir)?;
        let l = r.lock();
        Ok(l.room().head == 0.0 && l.room().files().is_empty())
    }

    /// One captured batch ordered in the room.
    pub fn record(&self, log_dir: &str, batch: &Value) -> Result<(), String> {
        let Some(map) = batch.as_object().cloned() else {
            return Err("a batch that is not an object".into());
        };
        self.room(log_dir)?.submit(map).map(|_| ())
    }

    /// What the capture diffs a restart against: the room's files, so what moved while k7 was down is one entry.
    pub fn baseline(self: &Arc<Self>, log_dir: &str) -> Arc<dyn Baseline> {
        Arc::new(RoomBaseline {
            rooms: self.clone(),
            log_dir: log_dir.to_string(),
        })
    }

    /// One entry that makes the room's files `files`: a text for each that is new or differs, a removal for each missing.
    pub fn reconcile(
        &self,
        log_dir: &str,
        files: &[(String, String)],
        label: &str,
    ) -> Result<(), String> {
        let live = self.room(log_dir)?;
        live.submit_with(|l| {
            let had = l.room().files();
            let mut ops: Vec<Value> = Vec::new();
            for (f, t) in files {
                let was = had.get(f);
                if was == Some(t) {
                    continue;
                }
                let was = was.map_or(Value::Null, |w| k7_js::number::text_hash(w).into());
                ops.push(json!({ "kind": "text", "file": f, "text": t, "was": was }));
            }
            let now: std::collections::HashSet<&str> =
                files.iter().map(|(f, _)| f.as_str()).collect();
            for (f, w) in had {
                if !now.contains(f.as_str()) {
                    let was = k7_js::number::text_hash(w);
                    ops.push(json!({ "kind": "remove", "file": f, "was": was }));
                }
            }
            if ops.is_empty() {
                return Ok(None);
            }
            let mut b = Map::new();
            b.insert(
                "id".into(),
                format!("history-{}", crate::disk::uuid_v4().unwrap_or_default()).into(),
            );
            b.insert("replica".into(), "history".into());
            b.insert("local".into(), 0.into());
            b.insert("ts".into(), k7_sync::relay::now_ms().into());
            b.insert("label".into(), label.into());
            b.insert("ops".into(), ops.into());
            Ok(Some(b))
        })
        .map(|_| ())
        .map_err(|(_, e)| e.to_string())
    }
}

struct RoomBaseline {
    rooms: Arc<Histories>,
    log_dir: String,
}

impl Baseline for RoomBaseline {
    fn files(&self) -> std::io::Result<Vec<String>> {
        let room = self
            .rooms
            .room(&self.log_dir)
            .map_err(std::io::Error::other)?;
        Ok(room.lock().room().files().keys().cloned().collect())
    }

    fn get(&self, file: &str) -> std::io::Result<Option<String>> {
        let room = self
            .rooms
            .room(&self.log_dir)
            .map_err(std::io::Error::other)?;
        Ok(room.lock().room().files().get(file).cloned())
    }
}
