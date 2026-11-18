//! relay — the device's history rooms (`docs/files/history.md`): one Room per log folder, loaded on first use, saved
//! every `snapshot_every` entries, given an automatic checkpoint once idle, dropped once unused, saved on stop.
//! What a reader gets wrong: an entry is appended to `entries.jsonl` BEFORE the room's memory takes it.

mod rooms;

pub use rooms::load as load_room;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::fsx;
use crate::log::{self, ENTRIES, LogError};
use crate::room::{Room, files_json};
use crate::settle::Js;

/// A saved state is rewritten after this many entries; the entries between live in an append-only log.
const SNAPSHOT_EVERY: f64 = 200.0;
/// A full copy of the files is kept for history after this many entries.
const HISTORY_EVERY: f64 = 1000.0;
/// A room edited and then left alone this long gets an automatic checkpoint.
const CHECKPOINT_IDLE: Duration = Duration::from_secs(600);
/// A room nobody is joined to and nobody touched this long is saved and dropped from memory.
const EVICT_IDLE: Duration = Duration::from_secs(600);

/// Where an account's rooms are kept, and the relay's cadences.
#[derive(Clone)]
pub struct Options {
    /// The account folder a room is saved under.
    pub account_dir: Arc<dyn Fn(&str) -> PathBuf + Send + Sync>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
    pub snapshot_every: f64,
    pub history_every: f64,
    pub checkpoint_idle: Duration,
    pub evict_idle: Duration,
}

impl Options {
    pub fn new(account_dir: Arc<dyn Fn(&str) -> PathBuf + Send + Sync>) -> Self {
        Options {
            account_dir,
            log: Arc::new(|m| eprintln!("sync | {m}")),
            snapshot_every: SNAPSHOT_EVERY,
            history_every: HISTORY_EVERY,
            checkpoint_idle: CHECKPOINT_IDLE,
            evict_idle: EVICT_IDLE,
        }
    }
}

/// One loaded room and who is in it.
pub struct Live {
    room: Room,
    dir: PathBuf,
    since_snapshot: f64,
    /// When the room was last edited, or None once that edit has its automatic checkpoint.
    edited_at: Option<Instant>,
    /// When anything last reached the room.
    used_at: Instant,
}

impl Live {
    pub fn room(&self) -> &Room {
        &self.room
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

pub(crate) struct Inner {
    opts: Options,
    js: Arc<dyn Js>,
    rooms: Mutex<HashMap<String, Arc<Mutex<Live>>>>,
    /// One gate per room being loaded: a load runs outside `rooms`, so a big room holds up only its own callers.
    loading: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    stopped: AtomicBool,
}

/// The relay: its rooms and its idle timer.
#[derive(Clone)]
pub struct Relay {
    inner: Arc<Inner>,
}

/// One live room as the history routes reach it: `submit` orders a batch exactly as a replica's is.
pub struct LiveRoom {
    inner: Arc<Inner>,
    live: Arc<Mutex<Live>>,
}

impl LiveRoom {
    pub fn lock(&self) -> MutexGuard<'_, Live> {
        lock(&self.live)
    }

    /// Order `batch` in this room, on disk and relayed, as a replica's batch is.
    pub fn submit(&self, batch: Map<String, Value>) -> Result<Value, String> {
        let mut l = self.lock();
        self.inner.order(&mut l, batch)
    }

    /// Build a batch from the room as it is and order it, the room held throughout; None orders nothing.
    pub fn submit_with(
        &self,
        build: impl FnOnce(&Live) -> Result<Option<Map<String, Value>>, (u16, Value)>,
    ) -> Result<Option<Value>, (u16, Value)> {
        let mut l = self.lock();
        let Some(batch) = build(&l)? else {
            return Ok(None);
        };
        let entry = self.inner.order(&mut l, batch);
        entry
            .map(Some)
            .map_err(|e| (500, serde_json::json!({ "error": e })))
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Milliseconds since the epoch, as `Date.now()`.
pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// The room's state and log saved; `since_snapshot` is cleared only once both are written.
fn snapshot(l: &mut Live, history_every: f64) -> Result<(), LogError> {
    fsx::write_atomic(&l.dir.join("room.json"), &l.room.state_text())?;
    let files = || files_json(l.room.files());
    log::roll_log(&l.dir, l.room.head, &files, history_every)?;
    l.since_snapshot = 0.0;
    Ok(())
}

impl Inner {
    fn say(&self, m: &str) {
        (self.opts.log)(m)
    }

    /// Save a room, said rather than thrown: its entries are on disk already, and a later save retries.
    fn save(&self, l: &mut Live) {
        if let Err(e) = snapshot(l, self.opts.history_every) {
            self.say(&format!("cannot save {}: {}", l.dir.display(), e.message()));
        }
    }

    /// Order a batch: on disk first.
    fn order(&self, l: &mut Live, batch: Map<String, Value>) -> Result<Value, String> {
        let entries = l.dir.join(ENTRIES);
        let (entry, fresh) = l
            .room
            .submit(self.js.as_ref(), batch, |e| {
                fsx::append(&entries, &log::entry_line(e))
            })
            .map_err(|r| r.0)?;
        if !fresh {
            return Ok(entry);
        }
        l.since_snapshot += 1.0;
        if l.since_snapshot >= self.opts.snapshot_every {
            self.save(l);
        }
        l.edited_at = Some(Instant::now());
        Ok(entry)
    }

    /// One pass of the idle timer: automatic checkpoints, then the rooms nobody uses saved and dropped.
    fn idle_pass(&self) {
        let now = Instant::now();
        let all: Vec<(String, Arc<Mutex<Live>>)> = lock(&self.rooms)
            .iter()
            .map(|(k, l)| (k.clone(), l.clone()))
            .collect();
        let idle = |l: &Live| {
            l.edited_at.is_none() && now.duration_since(l.used_at) >= self.opts.evict_idle
        };
        let mut gone = Vec::new();
        // Each room is taken alone: one busy ordering a batch holds up no other room's lookup.
        for (key, l) in all {
            let mut g = lock(&l);
            // A room whose files cannot be read or written is said and kept; the timer goes on.
            if let Err(e) = self.idle_room(&mut g, now) {
                self.say(&format!("idle {}: {}", g.dir.display(), e.message()));
                continue;
            }
            if idle(&g) {
                if g.since_snapshot > 0.0
                    && let Err(e) = snapshot(&mut g, self.opts.history_every)
                {
                    self.say(&format!("idle {}: {}", g.dir.display(), e.message()));
                    continue;
                }
                drop(g);
                gone.push((key, l));
            }
        }
        let mut rooms = lock(&self.rooms);
        for (key, l) in gone {
            // Dropped only if still the same room, still idle and saved: a caller may have joined it meanwhile.
            let same = rooms.get(&key).is_some_and(|r| Arc::ptr_eq(r, &l));
            if same
                && l.try_lock()
                    .is_ok_and(|g| idle(&g) && g.since_snapshot == 0.0)
            {
                rooms.remove(&key);
            }
        }
    }

    fn idle_room(&self, l: &mut Live, now: Instant) -> Result<(), LogError> {
        let Some(at) = l.edited_at else { return Ok(()) };
        if now.duration_since(at) < self.opts.checkpoint_idle {
            return Ok(());
        }
        l.edited_at = None;
        let last = log::read_checkpoints(&l.dir)?
            .last()
            .and_then(|c| c.get("seq"))
            .and_then(Value::as_f64);
        if last != Some(l.room.head) {
            log::add_checkpoint(
                &l.dir,
                log::checkpoint(l.room.head, "auto", now_ms(), "auto"),
            )?;
        }
        Ok(())
    }
}

impl Relay {
    /// A relay over `opts`, each op settled through `js`; `start` runs its timers.
    pub fn new(opts: Options, js: Arc<dyn Js>) -> Relay {
        Relay {
            inner: Arc::new(Inner {
                opts,
                js,
                rooms: Mutex::new(HashMap::new()),
                loading: Mutex::new(HashMap::new()),
                stopped: AtomicBool::new(false),
            }),
        }
    }

    /// The idle timer, on the current runtime, until `stop`.
    pub fn start(&self) {
        let inner = self.inner.clone();
        let o = &inner.opts;
        let tick = o.checkpoint_idle.min(o.evict_idle) / 4;
        let tick = tick.clamp(Duration::from_millis(10), Duration::from_secs(60));
        tokio::spawn(async move {
            let mut every = tokio::time::interval(tick);
            every.tick().await;
            loop {
                every.tick().await;
                if inner.stopped.load(Ordering::Acquire) {
                    return;
                }
                let i = inner.clone();
                let _ = tokio::task::spawn_blocking(move || i.idle_pass()).await;
            }
        });
    }

    /// The live room the history routes order a restore in, loaded on demand.
    pub fn room(&self, slug: &str, workspace: &str) -> Result<LiveRoom, String> {
        let live = self.inner.live(slug, workspace)?;
        Ok(LiveRoom {
            inner: self.inner.clone(),
            live,
        })
    }

    /// A room saved and dropped from memory, so its folder can move or go: the next use loads it again from where it is.
    pub fn release(&self, slug: &str, workspace: &str) {
        let key = format!("{slug}/{workspace}");
        let Some(l) = lock(&self.inner.rooms).remove(&key) else {
            return;
        };
        let mut l = lock(&l);
        if l.since_snapshot > 0.0 {
            self.inner.save(&mut l);
        }
    }

    /// The account folder a room of `slug` is kept under.
    pub fn account_dir(&self, slug: &str) -> PathBuf {
        (self.inner.opts.account_dir)(slug)
    }

    /// What settles and replays ops.
    pub fn js(&self) -> &dyn Js {
        self.inner.js.as_ref()
    }

    /// How many rooms are in memory: for tests and diagnostics.
    pub fn loaded(&self) -> usize {
        lock(&self.inner.rooms).len()
    }

    /// Save every room.
    pub fn stop(&self) {
        self.inner.stopped.store(true, Ordering::Release);
        let rooms: Vec<_> = lock(&self.inner.rooms).values().cloned().collect();
        for l in rooms {
            let mut l = lock(&l);
            if l.since_snapshot > 0.0 {
                self.inner.save(&mut l);
            }
        }
    }
}
