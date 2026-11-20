//! revisions.rs — the change token of each workspace: the inputs, a memo of the walk's summary kept while a feed
//! vouches for it, one process-wide epoch, and one walk per root at a time (1310 to 1325).
//! What a reader gets wrong: a token is a name for a state, never a count of changes; equal inputs answer the
//! same token, and no lock is held while a root is walked.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::libraries::Home;
use crate::revision::{Inputs, Summary, outside_newest, spell, walk_sources};
use crate::tree::Limits;
use crate::watch::{Feed, Watch};

/// The longest a summary is trusted on the feed's word alone: the TypeScript's `REVISION_WALK_ANYWAY_MS`.
const WALK_ANYWAY: Duration = Duration::from_secs(10);
/// Roots watched at once; past it the least recently asked is dropped (1325).
pub const MAX_WATCHED: usize = 64;
/// A root not asked for this long is not watched any more (1325).
const IDLE: Duration = Duration::from_secs(15 * 60);
/// A missed change is logged once an hour per workspace (1321).
const LOG_EVERY: Duration = Duration::from_secs(3600);

struct Kept {
    dir: String,
    stamp: u64,
    at: Instant,
    summary: Summary,
}

/// A walk in flight; its token once it ends, `Some(None)` when it ended without one.
struct Flight {
    dir: String,
    mutations: u64,
    done: Mutex<Option<Option<String>>>,
    cv: Condvar,
}

struct Root {
    mutations: u64,
    kept: Option<Kept>,
    last: Option<(Inputs, u64)>,
    /// Walks started, and the start of the newest one settled: an older walk never puts its state back (1315).
    started: u64,
    settled: u64,
    flight: Option<Arc<Flight>>,
    asked: Instant,
    watched: bool,
    unknowable_logged: bool,
    missed_logged: Option<Instant>,
}

/// The workspaces' tokens.
pub struct Revisions {
    feed: Box<dyn Feed>,
    watch: Option<&'static Watch>,
    now: fn() -> Instant,
    boot: String,
    epoch: AtomicU64,
    roots: Mutex<HashMap<String, Root>>,
    walks: AtomicU64,
    missed: AtomicU64,
}

/// The watcher of the process as a feed.
struct Global;

impl Feed for Global {
    fn generation(&self, root: &str) -> Option<u64> {
        Watch::global().generation(root)
    }
}

fn base36(mut n: u128) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8_lossy(&out).into_owned()
}

/// Ends a walk's flight whatever way the walker leaves, so a waiter never waits for a panic.
struct Ending<'a> {
    owner: &'a Revisions,
    root: &'a str,
    flight: Arc<Flight>,
}

impl Drop for Ending<'_> {
    fn drop(&mut self) {
        let mut roots = self.owner.roots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = roots.get_mut(self.root)
            && r.flight
                .as_ref()
                .is_some_and(|f| Arc::ptr_eq(f, &self.flight))
        {
            r.flight = None;
        }
        drop(roots);
        let mut done = self.flight.done.lock().unwrap_or_else(|e| e.into_inner());
        done.get_or_insert(None);
        self.flight.cv.notify_all();
    }
}

impl Revisions {
    /// The process's tokens, over the process's watcher.
    pub fn global() -> &'static Revisions {
        static GLOBAL: OnceLock<Revisions> = OnceLock::new();
        GLOBAL.get_or_init(|| {
            let mut r = Revisions::with_feed(Box::new(Global), Instant::now);
            r.watch = Some(Watch::global());
            r
        })
    }

    /// Over a feed of the caller's and its clock; no watcher is asked to watch anything.
    pub fn with_feed(feed: Box<dyn Feed>, now: fn() -> Instant) -> Revisions {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        Revisions {
            feed,
            watch: None,
            now,
            boot: base36(ms),
            epoch: AtomicU64::new(0),
            roots: Mutex::new(HashMap::new()),
            walks: AtomicU64::new(0),
            missed: AtomicU64::new(0),
        }
    }

    /// Over a watcher of its own, asked to watch each root.
    pub fn with_watch(watch: &'static Watch, now: fn() -> Instant) -> Revisions {
        let mut r = Revisions::with_feed(Box::new(WatchFeed(watch)), now);
        r.watch = Some(watch);
        r
    }

    /// Walks taken, and ceiling walks that found a change the feed never reported (1321).
    pub fn walks(&self) -> u64 {
        self.walks.load(Ordering::Relaxed)
    }

    pub fn missed(&self) -> u64 {
        self.missed.load(Ordering::Relaxed)
    }

    /// Roots this memo holds watched.
    pub fn watched(&self) -> usize {
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots.values().filter(|r| r.watched).count()
    }

    /// The inputs `root` last answered (1332).
    pub fn last_inputs(&self, root: &str) -> Option<Inputs> {
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots.get(root)?.last.as_ref().map(|(i, _)| i.clone())
    }

    /// When the watcher last moved `root`'s generation, in milliseconds since the epoch.
    pub fn heard_ms(&self, root: &str) -> Option<u64> {
        self.watch?.last_event_ms(root)
    }

    pub fn is_watched(&self, root: &str) -> bool {
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots.get(root).is_some_and(|r| r.watched)
    }

    fn blank(now: Instant) -> Root {
        Root {
            mutations: 0,
            kept: None,
            last: None,
            started: 0,
            settled: 0,
            flight: None,
            asked: now,
            watched: false,
            unknowable_logged: false,
            missed_logged: None,
        }
    }

    /// The token of `root`'s active project `dir` (`none:` without one); `fresh` is a walk, never the memo (1305).
    pub fn token(&self, home: &Home, root: &str, dir: Option<&str>, fresh: bool) -> String {
        let now = (self.now)();
        let dropped = self.touch(root, now);
        if let Some(w) = self.watch {
            for r in &dropped {
                w.unwatch(r);
            }
            if self.feed.generation(root).is_none() {
                w.watch(root);
            }
        }
        let Some(dir) = dir else {
            return self.settle_with(root, None, |_| Inputs {
                dir: None,
                mutations: 0,
                summary: None,
                outside: 0.0,
                unknowable: false,
            });
        };
        loop {
            let stamp = self.feed.generation(root);
            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            let r = roots
                .entry(root.to_string())
                .or_insert_with(|| Revisions::blank(now));
            if !fresh {
                if let Some(k) = r.kept.as_ref().filter(|k| k.dir == dir)
                    && stamp == Some(k.stamp)
                    && now.saturating_duration_since(k.at) < WALK_ANYWAY
                {
                    let summary = k.summary;
                    drop(roots);
                    let outside = outside_newest(home);
                    return self.settle_with(root, None, |mutations| Inputs {
                        dir: Some(dir.to_string()),
                        mutations,
                        summary: Some(summary),
                        outside,
                        unknowable: false,
                    });
                }
                // A walk begun before a bump never answers for after it (1346).
                let mutations = r.mutations;
                if let Some(f) = r
                    .flight
                    .as_ref()
                    .filter(|f| f.dir == dir && f.mutations == mutations)
                    .cloned()
                {
                    drop(roots);
                    let mut done = f.done.lock().unwrap_or_else(|e| e.into_inner());
                    while done.is_none() {
                        done = f.cv.wait(done).unwrap_or_else(|e| e.into_inner());
                    }
                    if let Some(Some(t)) = done.as_ref() {
                        return t.clone();
                    }
                    continue;
                }
            }
            return self.walk(home, root, dir, stamp, now, roots);
        }
    }

    /// One walk, the mutex of the roots held only to start and to settle (1315).
    fn walk(
        &self,
        home: &Home,
        root: &str,
        dir: &str,
        stamp: Option<u64>,
        now: Instant,
        mut roots: std::sync::MutexGuard<'_, HashMap<String, Root>>,
    ) -> String {
        let r = roots.get_mut(root).expect("touched");
        r.started += 1;
        let seq = r.started;
        let mutations = r.mutations;
        // The age ran out with nothing heard: the walk is also a check of the feed (1321).
        let ceiling = stamp.is_some()
            && r.kept.as_ref().is_some_and(|k| {
                k.dir == dir
                    && Some(k.stamp) == stamp
                    && now.saturating_duration_since(k.at) >= WALK_ANYWAY
            });
        // A flight of another project or of an older count is no one's to join any more: this one takes its place.
        let joinable = r
            .flight
            .as_ref()
            .is_some_and(|f| f.dir == dir && f.mutations == mutations);
        let flight = (!joinable).then(|| {
            let f = Arc::new(Flight {
                dir: dir.to_string(),
                mutations,
                done: Mutex::new(None),
                cv: Condvar::new(),
            });
            r.flight = Some(Arc::clone(&f));
            f
        });
        drop(roots);
        let ending = flight.map(|flight| Ending {
            owner: self,
            root,
            flight,
        });
        let (summary, unknowable) = walk_sources(dir, &Limits::default());
        self.walks.fetch_add(1, Ordering::Relaxed);
        let inputs = Inputs {
            dir: Some(dir.to_string()),
            mutations,
            summary: Some(summary),
            outside: outside_newest(home),
            unknowable,
        };
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let r = roots.get_mut(root).expect("touched");
        let missed = ceiling
            && r.last.as_ref().is_some_and(|(l, _)| {
                l.dir == inputs.dir && l.mutations == mutations && l.summary != inputs.summary
            });
        // Kept only when no write landed since the walk began and it is the newest (1315).
        if stamp.is_some() && !unknowable && r.mutations == mutations && seq >= r.settled {
            r.kept = Some(Kept {
                dir: dir.to_string(),
                stamp: stamp.unwrap_or_default(),
                at: now,
                summary,
            });
        } else if seq >= r.settled {
            r.kept = None;
        }
        if unknowable && !r.unknowable_logged {
            r.unknowable_logged = true;
            eprintln!(
                "[files] native: {root} passed a walk limit, its token is walked at every ask"
            );
        }
        let log = missed
            && r.missed_logged
                .is_none_or(|t| now.saturating_duration_since(t) >= LOG_EVERY);
        if log {
            r.missed_logged = Some(now);
        }
        let epoch = self.settle(r, inputs, Some(seq));
        drop(roots);
        let token = spell(&self.boot, epoch, true);
        if missed {
            self.missed.fetch_add(1, Ordering::Relaxed);
            if log {
                eprintln!("[files] native: the watcher missed a change in {root}, watching again");
            }
            if let Some(w) = self.watch {
                w.rewatch(root);
            }
        }
        if let Some(e) = ending {
            *e.flight.done.lock().unwrap_or_else(|x| x.into_inner()) = Some(Some(token.clone()));
        }
        token
    }

    /// The epoch of `inputs`: the last one when they are those last answered, else the process's next (1310).
    fn settle(&self, r: &mut Root, inputs: Inputs, seq: Option<u64>) -> u64 {
        if let Some(s) = seq {
            if s < r.settled
                && let Some((_, e)) = &r.last
            {
                return *e;
            }
            r.settled = s;
        }
        match &r.last {
            Some((l, e)) if *l == inputs && !inputs.unknowable => *e,
            _ => {
                let e = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
                r.last = Some((inputs, e));
                e
            }
        }
    }

    /// Settles inputs that need no walk; `make` reads the mutation counter's current value.
    fn settle_with(
        &self,
        root: &str,
        seq: Option<u64>,
        make: impl FnOnce(u64) -> Inputs,
    ) -> String {
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let r = roots.get_mut(root).expect("touched");
        let mut inputs = make(r.mutations);
        inputs.mutations = r.mutations;
        let active = inputs.dir.is_some();
        let epoch = self.settle(r, inputs, seq);
        spell(&self.boot, epoch, active)
    }

    /// Marks `root` asked and returns the roots to stop watching: past the cap, or idle (1325).
    fn touch(&self, root: &str, now: Instant) -> Vec<String> {
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let r = roots
            .entry(root.to_string())
            .or_insert_with(|| Revisions::blank(now));
        r.asked = now;
        r.watched = true;
        let mut dropped: Vec<String> = Vec::new();
        for (k, v) in roots.iter() {
            if k != root && v.watched && now.saturating_duration_since(v.asked) >= IDLE {
                dropped.push(k.clone());
            }
        }
        let mut live = roots.values().filter(|v| v.watched).count() - dropped.len();
        while live > MAX_WATCHED {
            let oldest = roots
                .iter()
                .filter(|(k, v)| k.as_str() != root && v.watched && !dropped.contains(k))
                .min_by_key(|(_, v)| v.asked)
                .map(|(k, _)| k.clone());
            let Some(k) = oldest else { break };
            dropped.push(k);
            live -= 1;
        }
        for k in &dropped {
            if let Some(v) = roots.get_mut(k) {
                v.watched = false;
                v.kept = None;
            }
        }
        dropped
    }

    /// How many writes of this process were announced on `root`: two reads that differ by one saw one write between them.
    pub fn mutations(&self, root: &str) -> u64 {
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots.get(root).map_or(0, |r| r.mutations)
    }

    /// A write of this process landed: the next token is new and walks (1306).
    pub fn bump(&self, root: &str) {
        let now = (self.now)();
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let r = roots
            .entry(root.to_string())
            .or_insert_with(|| Revisions::blank(now));
        r.mutations += 1;
        r.kept = None;
    }

    /// The workspace is gone: its watch dropped and its state with it; the epoch is the process's, so it never repeats.
    pub fn forget(&self, root: &str) {
        self.roots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(root);
        if let Some(w) = self.watch {
            w.unwatch(root);
        }
    }
}

impl Revisions {
    /// Every workspace below `folder` forgotten: a removal names one and the front has not parsed which (1323).
    pub fn forget_under(&self, folder: &str) {
        let folder = crate::paths::resolve_all(&[folder]);
        let named: Vec<String> = {
            let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            roots
                .keys()
                .filter(|k| crate::paths::is_within(&folder, k))
                .cloned()
                .collect()
        };
        for root in named {
            self.forget(&root);
        }
    }
}

struct WatchFeed(&'static Watch);

impl Feed for WatchFeed {
    fn generation(&self, root: &str) -> Option<u64> {
        self.0.generation(root)
    }
}

#[path = "revisions_walked.rs"]
mod walked;

#[cfg(test)]
#[path = "revisions_tests.rs"]
mod tests;
