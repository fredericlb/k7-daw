//! modwatch.rs — the module watch's generations, natively (2640 to 2642): an account's joins of a project's module roots,
//! the roots inside a captured workspace heard through the capture's feed and the others by one `notify` watcher per real
//! root in the process, shared by every join and closed when the last leaves.
//! What a reader gets wrong: saying "moved" is always safe and "still" never is; no lock is held while a watcher is made or closed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::watcher::{self, Boxed};

/// A ceiling, not a cache interval: it makes a MISSED event a delay (`moduleWatch.md`).
pub const WALK_ANYWAY: Duration = Duration::from_secs(10);

type Subs = Arc<Mutex<Vec<(u64, Arc<AtomicU64>)>>>;

/// One real root's watcher, and who hears it.
struct Shared {
    subs: Subs,
    _watcher: Boxed,
}

fn registry() -> &'static Mutex<HashMap<PathBuf, Shared>> {
    static R: OnceLock<Mutex<HashMap<PathBuf, Shared>>> = OnceLock::new();
    R.get_or_init(Mutex::default)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// How many real roots the process watches now; what the tests count.
pub fn watchers() -> usize {
    lock(registry()).len()
}

struct Join {
    id: u64,
    feed: Option<String>,
    /// The roots inside the feed's workspace, relative to it and `/`-separated; "" is the workspace itself.
    heard: Vec<String>,
    /// The real roots watched for this join.
    real: Vec<PathBuf>,
    /// Bumped by every event on every root; only its changes matter.
    gens: Arc<AtomicU64>,
    seen: Mutex<(u64, Option<Instant>)>,
}

/// An account's joins, by project folder.
pub struct Watch {
    watching: bool,
    joins: Mutex<HashMap<String, Arc<Join>>>,
}

/// Does a reported path touch `root`: the root itself, under it, or a folder holding it that moved whole.
fn touches(root: &str, rel: &str) -> bool {
    root.is_empty()
        || rel == root
        || rel.strip_prefix(root).is_some_and(|r| r.starts_with('/'))
        || root.strip_prefix(rel).is_some_and(|r| r.starts_with('/'))
}

/// `abs` under `root` as a `/`-separated path, "" for the root itself, or None when it lies outside.
fn inside(root: &Path, abs: &Path) -> Option<String> {
    let rel = abs.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

impl Watch {
    pub fn new(watching: bool) -> Watch {
        Watch {
            watching,
            joins: Mutex::default(),
        }
    }

    /// How many project folders this account has joined.
    pub fn joined(&self) -> usize {
        lock(&self.joins).len()
    }

    /// Join `dir`'s `roots`; idempotent for one feed, and a different feed starts over (the capture was replaced).
    pub fn join(&self, dir: &str, roots: &[String], feed: Option<&str>) {
        let had = lock(&self.joins).get(dir).cloned();
        if let Some(had) = &had {
            if had.feed.as_deref() == feed {
                return;
            }
            self.release_dir(dir);
        }
        let gens = Arc::new(AtomicU64::new(0));
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (mut heard, mut real) = (Vec::new(), Vec::new());
        for root in roots {
            if let Some(rel) = feed.and_then(|f| inside(Path::new(f), Path::new(root))) {
                heard.push(rel);
            } else if self.watching
                && let Ok(path) = std::fs::canonicalize(root)
                && subscribe(&path, id, &gens)
            {
                real.push(path);
            }
        }
        let join = Arc::new(Join {
            id,
            feed: feed.map(String::from),
            heard,
            real,
            gens,
            seen: Mutex::new((0, None)),
        });
        lock(&self.joins).insert(dir.to_string(), join);
    }

    /// Whether `dir`'s roots are KNOWN not to have moved since the last walk; false is always safe.
    pub fn still(&self, dir: &str) -> bool {
        self.still_at(dir, Instant::now())
    }

    pub fn still_at(&self, dir: &str, now: Instant) -> bool {
        let Some(join) = lock(&self.joins).get(dir).cloned() else {
            return false;
        };
        let gens = join.gens.load(Ordering::Acquire);
        let mut seen = lock(&join.seen);
        let late = seen
            .1
            .is_none_or(|at| now.saturating_duration_since(at) > WALK_ANYWAY);
        if gens != seen.0 || late {
            *seen = (gens, Some(now));
            return false;
        }
        true
    }

    /// This process wrote something: every join counts as moved.
    pub fn moved(&self) {
        for j in lock(&self.joins).values() {
            j.gens.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// The capture's feed reported `paths` under workspace `ws`: the joins heard through it that they touch move.
    pub fn feed(&self, ws: &str, paths: &[String]) {
        for j in lock(&self.joins).values() {
            if j.feed.as_deref() == Some(ws)
                && paths.iter().any(|p| j.heard.iter().any(|r| touches(r, p)))
            {
                j.gens.fetch_add(1, Ordering::AcqRel);
            }
        }
    }

    /// Let go of this account's joins under `under` by path component, or of all; how many were let go.
    pub fn release(&self, under: Option<&str>) -> usize {
        let gone: Vec<Arc<Join>> = {
            let mut joins = lock(&self.joins);
            let dirs: Vec<String> = joins
                .keys()
                .filter(|d| under.is_none_or(|u| Path::new(d.as_str()).starts_with(u)))
                .cloned()
                .collect();
            dirs.iter().filter_map(|d| joins.remove(d)).collect()
        };
        let n = gone.len();
        for j in gone {
            leave(&j);
        }
        n
    }

    fn release_dir(&self, dir: &str) {
        let gone = lock(&self.joins).remove(dir);
        if let Some(j) = gone {
            leave(&j);
        }
    }
}

/// Hear real root `path` for join `id`, making its watcher when no join holds one; false when it cannot be watched.
fn subscribe(path: &Path, id: u64, gens: &Arc<AtomicU64>) -> bool {
    // A published version of a library never changes, and a scarce host does not pay to watch what cannot move.
    if watcher::scarce() && watcher::under_frozen(path) {
        return true;
    }
    if let Some(s) = lock(registry()).get(path) {
        lock(&s.subs).push((id, gens.clone()));
        return true;
    }
    // Made outside the registry's lock: a watcher's start may take a while.
    let subs: Subs = Arc::new(Mutex::new(vec![(id, gens.clone())]));
    let heard = subs.clone();
    let made = watcher::make(move |r: notify::Result<notify::Event>| {
        // An error is itself a change, as the TypeScript's `error` handler counts it.
        if r.as_ref()
            .is_ok_and(|e| matches!(e.kind, notify::EventKind::Access(_)))
        {
            return;
        }
        for (_, g) in lock(&heard).iter() {
            g.fetch_add(1, Ordering::AcqRel);
        }
    });
    let Ok(mut watcher) = made else {
        return false;
    };
    for (at, mode) in watcher::plan(path, watcher::scarce()) {
        if watcher.watch(&at, mode).is_err() {
            return false;
        }
    }
    let lost = {
        let mut map = lock(registry());
        match map.get(path) {
            // Another join made it meanwhile: ours is let go once the lock is.
            Some(s) => {
                lock(&s.subs).push((id, gens.clone()));
                Some(watcher)
            }
            None => {
                map.insert(
                    path.to_path_buf(),
                    Shared {
                        subs,
                        _watcher: watcher,
                    },
                );
                None
            }
        }
    };
    drop(lost);
    true
}

/// A join gone: its roots forgotten, and a watcher nobody hears any more closed outside the registry's lock.
fn leave(j: &Join) {
    let mut closing = Vec::new();
    {
        let mut map = lock(registry());
        for path in &j.real {
            let Some(s) = map.get(path) else { continue };
            let empty = {
                let mut subs = lock(&s.subs);
                subs.retain(|(id, _)| *id != j.id);
                subs.is_empty()
            };
            if empty && let Some(s) = map.remove(path) {
                closing.push(s);
            }
        }
    }
    drop(closing);
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.release(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_matches_by_path_component_and_a_root_is_watched_once() {
        let tmp = std::env::temp_dir().join(format!("k7-modwatch-{}", std::process::id()));
        let (b, bc) = (tmp.join("b"), tmp.join("bc"));
        std::fs::create_dir_all(b.join("nodes")).unwrap();
        std::fs::create_dir_all(bc.join("nodes")).unwrap();
        let w = Watch::new(true);
        let before = watchers();
        let one = |d: &Path| vec![d.join("nodes").display().to_string()];
        w.join(&b.display().to_string(), &one(&b), None);
        w.join(&bc.display().to_string(), &one(&bc), None);
        assert_eq!(watchers(), before + 2);
        assert_eq!(w.release(Some(&b.display().to_string())), 1);
        assert_eq!((w.joined(), watchers()), (1, before + 1));
        let other = Watch::new(true);
        other.join("x", &one(&bc), None);
        assert_eq!(watchers(), before + 1, "one watcher for two accounts");
        assert_eq!(w.release(None), 1);
        assert_eq!(watchers(), before + 1, "the other account still hears it");
        other.release(None);
        assert_eq!(watchers(), before);
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn a_root_in_a_captured_workspace_moves_with_its_feed_and_still_needs_a_walk_first() {
        let w = Watch::new(true);
        let ws = "/ws";
        w.join("/ws/p", &["/ws/p/nodes".to_string()], Some(ws));
        assert!(!w.still("/ws/p"), "the first ask walks");
        assert!(w.still("/ws/p"));
        w.feed(ws, &["p/other/x".to_string()]);
        assert!(w.still("/ws/p"), "a path beside the roots moves nothing");
        w.feed(ws, &["p/nodes/a.k7x".to_string()]);
        assert!(!w.still("/ws/p"));
        assert!(w.still("/ws/p"));
        let later = Instant::now() + WALK_ANYWAY + Duration::from_secs(1);
        assert!(!w.still_at("/ws/p", later), "the ceiling walks anyway");
        w.moved();
        assert!(!w.still("/ws/p"));
    }
}
