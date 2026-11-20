//! watch.rs — the process's one `notify` watcher over many workspace roots, and a generation per root that moves
//! when an event may have changed a project source (1322 to 1326). The roots are keyed by their spelling as given.
//! What a reader gets wrong: `watch` never holds the roots' lock across `notify`'s call, whose event loop would
//! wait for that lock in its handler and never answer (a deadlock on inotify).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind};

use crate::filter::{project_app, skip};
use crate::paths::{extname, is_within, slashed};
use crate::revision::is_source;
use crate::watcher::{self, Boxed};

/// A root that failed is watched again no sooner than this (1324).
const RETRY: Duration = Duration::from_secs(60);

/// What a memo asks of a watcher: the root's generation, or `None` when it is not watched or the watcher is down.
pub trait Feed: Send + Sync {
    fn generation(&self, root: &str) -> Option<u64>;
}

/// Where a pinned root's event paths go: below the root, `/`-spelt, and "" when something must be looked at whole (2126).
pub type Sink = Arc<dyn Fn(Vec<String>) + Send + Sync>;

struct Root {
    /// A pinned root is watched until `unpin`: no eviction of the revision's reaches it.
    sink: Option<Sink>,
    spellings: Vec<String>,
    up: bool,
    down_since: Option<Instant>,
    stamp: AtomicU64,
    /// When the generation last moved, in milliseconds since the epoch: the measure of staleness (1332).
    at_ms: AtomicU64,
}

type Heard = Arc<dyn Fn(&str) + Send + Sync>;

/// Something that hears every path below a folder, before any filter (2228).
struct Listener {
    id: u64,
    spellings: Vec<String>,
    heard: Heard,
}

struct Shared {
    listeners: Mutex<Vec<Listener>>,
    roots: Mutex<HashMap<String, Root>>,
    /// One clock for every root's generations: a root watched again never repeats a value.
    clock: AtomicU64,
}

impl Shared {
    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Moves a root's generation and notes when.
    fn move_root(&self, r: &Root) {
        r.at_ms.store(now_ms(), Ordering::Relaxed);
        r.stamp.store(self.tick(), Ordering::Relaxed);
    }

    /// Every root moved: FSEvents restarts its one stream on each watch or unwatch, losing what fell between (1345).
    fn move_all(&self, roots: &HashMap<String, Root>) {
        for r in roots.values() {
            self.move_root(r);
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The process's one watcher.
pub struct Watch {
    shared: Arc<Shared>,
    watcher: Mutex<Option<Boxed>>,
    /// What each root is watched by, so an unwatch lets go of all of it (`watcher::plan`).
    planned: Mutex<HashMap<String, Vec<PathBuf>>>,
    quiet: Mutex<Vec<String>>,
}

fn trimmed(p: &str) -> String {
    let s = slashed(p);
    let t = s.trim_end_matches('/');
    if t.is_empty() {
        "/".into()
    } else {
        t.to_string()
    }
}

/// The root as spelled and as the file system names it (FSEvents says `/private/var`; Windows `\\?\` is stripped).
fn spellings(root: &str) -> Vec<String> {
    let mut v = vec![trimmed(root)];
    if let Ok(c) = std::fs::canonicalize(root) {
        let c = c.to_string_lossy();
        let s = trimmed(c.strip_prefix(r"\\?\").unwrap_or(&c));
        if !v.contains(&s) {
            v.push(s);
        }
    }
    v
}

/// Whether an event at `rel` (below its root, `/`-spelt, "" for the root) may have changed a source; `is_dir` is `None` when gone (1324).
pub fn relevant(kind: &EventKind, rel: &str, is_dir: impl Fn() -> Option<bool>) -> bool {
    // inotify reports the walk's own `readdir` as an open.
    if matches!(kind, EventKind::Access(_)) {
        return false;
    }
    if rel.is_empty() {
        return true;
    }
    if rel.split('/').any(skip) {
        return false;
    }
    let dir = std::cell::OnceCell::new();
    let dir = || *dir.get_or_init(&is_dir);
    // Windows reports a folder as modified when an entry is added in it; the entry's own event says which.
    if matches!(kind, EventKind::Modify(k) if !matches!(k, ModifyKind::Name(_)))
        && dir() == Some(true)
    {
        return false;
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let ext = extname(name);
    if ext.is_empty() || is_source(name) || project_app(name).is_some() {
        return true;
    }
    // A file of another extension is media or text; a folder of any name may hold sources.
    if matches!(
        kind,
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder)
    ) {
        return true;
    }
    // A rename or a removal that names no kind leaves nothing to stat: it may have been a folder of sources (1344).
    let untyped = matches!(
        kind,
        EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Remove(RemoveKind::Any | RemoveKind::Other)
    );
    match dir() {
        Some(d) => d,
        None => untyped,
    }
}

impl Root {
    fn new(root: &str) -> Root {
        Root {
            sink: None,
            spellings: spellings(root),
            up: false,
            down_since: None,
            stamp: AtomicU64::new(0),
            at_ms: AtomicU64::new(0),
        }
    }

    /// The paths of `ev` below this root, whatever their extension (a capture wants media too); "" for a rescan.
    fn delivered(&self, ev: &Event, rescan: bool) -> Vec<String> {
        if rescan {
            return vec![String::new()];
        }
        if matches!(ev.kind, EventKind::Access(_)) {
            return vec![];
        }
        let mut out: Vec<String> = vec![];
        for p in &ev.paths {
            let p = slashed(&p.to_string_lossy()).into_owned();
            let below = self.spellings.iter().find(|s| is_within(s, &p));
            let rel = below.map_or("", |s| {
                p.get(s.len()..).unwrap_or("").trim_start_matches('/')
            });
            if below.is_some() && !rel.is_empty() && !out.iter().any(|o| o == rel) {
                out.push(rel.to_string());
            }
        }
        out
    }
}

/// Hands the paths to the sinks, with the roots' lock released: a sink may take its own locks or wake a task.
fn deliver(out: Vec<(Sink, Vec<String>)>) {
    for (sink, rels) in out {
        sink(rels);
    }
}

/// A listener's registration: dropping it lets go.
pub struct Listening {
    shared: std::sync::Weak<Shared>,
    id: u64,
}

impl Drop for Listening {
    fn drop(&mut self) {
        if let Some(s) = self.shared.upgrade() {
            s.listeners
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|l| l.id != self.id);
        }
    }
}

impl Shared {
    /// Tell the listeners each path below their folder, outside every lock their callbacks could wait on.
    fn tell(&self, ev: &Event) {
        if matches!(ev.kind, EventKind::Access(_)) {
            return;
        }
        let mut calls: Vec<(Heard, String)> = vec![];
        {
            let ls = self.listeners.lock().unwrap_or_else(|e| e.into_inner());
            for l in ls.iter() {
                for p in &ev.paths {
                    let p = slashed(&p.to_string_lossy()).into_owned();
                    for s in &l.spellings {
                        if is_within(s, &p) {
                            let rel = p.get(s.len()..).unwrap_or("").trim_start_matches('/');
                            calls.push((Arc::clone(&l.heard), rel.to_string()));
                            break;
                        }
                    }
                }
            }
        }
        for (f, rel) in calls {
            f(&rel);
        }
    }

    fn heard(&self, ev: notify::Result<Event>) {
        if let Ok(ev) = &ev {
            self.tell(ev);
        }
        let mut out: Vec<(Sink, Vec<String>)> = vec![];
        self.heard_into(ev, &mut out);
        deliver(out);
    }

    fn heard_into(&self, ev: notify::Result<Event>, out: &mut Vec<(Sink, Vec<String>)>) {
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let ev = match ev {
            Ok(ev) => ev,
            Err(e) => {
                // An error names the roots it touches, or else none: then every root is down.
                for r in roots.values_mut() {
                    let named = e.paths.iter().any(|p| {
                        let p = slashed(&p.to_string_lossy()).into_owned();
                        r.spellings.iter().any(|s| is_within(s, &p))
                    });
                    if named || e.paths.is_empty() {
                        r.up = false;
                        r.down_since = Some(Instant::now());
                        self.move_root(r);
                        if let Some(sink) = &r.sink {
                            out.push((sink.clone(), vec![String::new()]));
                        }
                    }
                }
                return;
            }
        };
        let rescan = ev.need_rescan() || ev.paths.is_empty();
        for r in roots.values() {
            if let Some(sink) = &r.sink {
                let rels = r.delivered(&ev, rescan);
                if !rels.is_empty() {
                    out.push((sink.clone(), rels));
                }
            }
            let moved = rescan
                || ev.paths.iter().any(|p| {
                    let p = slashed(&p.to_string_lossy()).into_owned();
                    r.spellings.iter().any(|s| {
                        is_within(s, &p)
                            && relevant(
                                &ev.kind,
                                p.get(s.len()..).unwrap_or("").trim_start_matches('/'),
                                || std::fs::metadata(&p).ok().map(|m| m.is_dir()),
                            )
                    })
                });
            if moved {
                self.move_root(r);
            }
        }
    }
}

impl Default for Watch {
    fn default() -> Self {
        Watch::new()
    }
}

impl Watch {
    /// A watcher of its own, for tests; the process's is `global`.
    pub fn new() -> Watch {
        Watch {
            shared: Arc::new(Shared {
                listeners: Mutex::new(Vec::new()),
                roots: Mutex::new(HashMap::new()),
                clock: AtomicU64::new(0),
            }),
            watcher: Mutex::new(None),
            planned: Mutex::new(HashMap::new()),
            quiet: Mutex::new(Vec::new()),
        }
    }

    pub fn global() -> &'static Watch {
        static GLOBAL: OnceLock<Watch> = OnceLock::new();
        GLOBAL.get_or_init(Watch::new)
    }

    /// Hears every path below `dir` as `/`-spelt, `dir` itself being watched through a root above it: no watcher of its own
    /// (2228), so a folder Windows would hold open is not held by this. Access events are never told.
    pub fn listen(&self, dir: &str, heard: impl Fn(&str) + Send + Sync + 'static) -> Listening {
        static IDS: AtomicU64 = AtomicU64::new(1);
        let id = IDS.fetch_add(1, Ordering::Relaxed);
        self.shared
            .listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Listener {
                id,
                spellings: spellings(dir),
                heard: Arc::new(heard),
            });
        Listening {
            shared: Arc::downgrade(&self.shared),
            id,
        }
    }

    /// Roots under `folder` are never watched; only test folders are named (docs/service.md).
    pub fn silence(&self, folder: &str) {
        let mut q = self.quiet.lock().unwrap_or_else(|e| e.into_inner());
        q.push(trimmed(folder));
    }

    fn silenced(&self, root: &str) -> bool {
        let q = self.quiet.lock().unwrap_or_else(|e| e.into_inner());
        let root = trimmed(root);
        q.iter().any(|f| is_within(f, &root))
    }

    /// Watches `root` recursively; a root already up is left alone, one that failed waits a minute (1324).
    pub fn watch(&self, root: &str) {
        if self.silenced(root) {
            return;
        }
        {
            let mut roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(r) = roots.get(root)
                && (r.up || r.down_since.is_some_and(|t| t.elapsed() < RETRY))
            {
                return;
            }
            // Registered before the call, so an event arriving at once finds its root.
            roots
                .entry(root.to_string())
                .or_insert_with(|| Root::new(root));
        }
        let ok = self.add(root);
        let mut roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = roots.get_mut(root) {
            r.spellings = spellings(root);
            r.up = ok;
            r.down_since = (!ok).then(Instant::now);
        }
        self.shared.move_all(&roots);
    }

    fn add(&self, root: &str) -> bool {
        let mut w = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        if w.is_none() {
            let shared = Arc::clone(&self.shared);
            *w = watcher::make(move |ev| shared.heard(ev)).ok();
        }
        let Some(w) = w.as_mut() else { return false };
        let mut paths: Vec<PathBuf> = Vec::new();
        for (path, mode) in watcher::plan(Path::new(root), watcher::scarce()) {
            if w.watch(&path, mode).is_err() {
                for done in paths.drain(..) {
                    let _ = w.unwatch(&done);
                }
                return false;
            }
            paths.push(path);
        }
        self.planned
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(root.to_string(), paths);
        true
    }

    /// Lets go of everything `root` is watched by.
    fn unwatch_all(&self, root: &str) {
        let paths = self
            .planned
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(root)
            .unwrap_or_else(|| vec![PathBuf::from(root)]);
        let mut w = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(w) = w.as_mut() {
            for path in paths {
                let _ = w.unwatch(&path);
            }
        }
    }

    /// Watches `root` until `unpin`, its event paths going to `sink` besides moving its generation; true when it is up (2126).
    pub fn pin(&self, root: &str, sink: Sink) -> bool {
        {
            let mut roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
            let r = roots
                .entry(root.to_string())
                .or_insert_with(|| Root::new(root));
            r.sink = Some(sink);
        }
        self.watch(root);
        self.generation(root).is_some()
    }

    /// Drops a root's watch pinned with `sink`, before its folder is renamed or removed; a newer pin of the same root stays.
    pub fn unpin(&self, root: &str, sink: &Sink) {
        self.drop_root(root, Some(sink));
    }

    /// Drops the root's watch; Windows holds the folder open while it is watched (1323). A pinned root stays.
    pub fn unwatch(&self, root: &str) {
        self.drop_root(root, None);
    }

    fn drop_root(&self, root: &str, pinned: Option<&Sink>) {
        let was = {
            let mut roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
            let mine = roots.get(root).is_none_or(|r| match (&r.sink, pinned) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            });
            if !mine {
                return;
            }
            roots.remove(root).is_some_and(|r| r.up)
        };
        if was {
            self.unwatch_all(root);
            let roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
            self.shared.move_all(&roots);
        }
    }

    /// Drops and adds notify's watch of `root` again, keeping its sink and its place: the recovery from a missed change.
    pub fn rewatch(&self, root: &str) {
        {
            let mut roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(r) = roots.get_mut(root) {
                r.up = false;
                r.down_since = None;
            }
        }
        self.unwatch_all(root);
        self.watch(root);
    }

    /// When `root`'s generation last moved, in milliseconds since the epoch; `None` where it is not watched (1332).
    pub fn last_event_ms(&self, root: &str) -> Option<u64> {
        let roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots.get(root).map(|r| r.at_ms.load(Ordering::Relaxed))
    }

    /// How many roots are up.
    pub fn watched(&self) -> usize {
        let roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots.values().filter(|r| r.up).count()
    }
}

impl Feed for Watch {
    fn generation(&self, root: &str) -> Option<u64> {
        let roots = self.shared.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots
            .get(root)
            .filter(|r| r.up)
            .map(|r| r.stamp.load(Ordering::Relaxed))
    }
}
