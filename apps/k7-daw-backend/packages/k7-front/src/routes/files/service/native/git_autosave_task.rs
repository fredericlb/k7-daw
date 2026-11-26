//! git_autosave_task.rs — when a connected workspace auto-saves: 20 s after its last change, at once when the page
//! leaves it (home, background, close), and for every workspace when the front stops; a push that failed is retried.
//! What a reader gets wrong: `Idle` holds no clock (the caller passes seconds) and "a change" is the capture's
//! generation moving, so a snapshot is only TRIED; the crate refuses an unchanged tree (`docs/files/git.md`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::git::Ctx;
use super::git_autosave::round;
use super::git_fetch::now;

/// Seconds with no change before a snapshot.
pub const IDLE: u64 = 20;
/// The first wait after a failed push; it doubles up to `RETRY_MAX`.
pub const RETRY: u64 = 30;
pub const RETRY_MAX: u64 = 30 * 60;
/// How often the task looks at the workspace.
pub const LOOK: Duration = Duration::from_secs(2);

/// A workspace's auto-save clock.
#[derive(Default)]
pub struct Idle {
    mark: Option<u64>,
    changed_at: u64,
    dirty: bool,
    urgent: bool,
    unsent: bool,
    failures: u32,
    next_try: u64,
}

impl Idle {
    /// The change counter read at `now`; the first reading counts as a change, so a restart misses nothing.
    pub fn seen(&mut self, mark: u64, now: u64) {
        if self.mark != Some(mark) {
            self.mark = Some(mark);
            self.changed_at = now;
            self.dirty = true;
        }
    }

    /// A snapshot is due: something changed and nothing more for `IDLE` seconds.
    pub fn due(&self, now: u64) -> bool {
        self.dirty && (self.urgent || now.saturating_sub(self.changed_at) >= IDLE)
    }

    /// The page left the workspace: the next look snapshots without waiting.
    pub fn now_please(&mut self) {
        self.dirty = true;
        self.urgent = true;
    }

    /// A push is owed and its wait is over.
    pub fn retry_due(&self, now: u64) -> bool {
        self.unsent && now >= self.next_try
    }

    /// A round ended at `now`; `unsent` when the remote still lacks the branch.
    pub fn done(&mut self, now: u64, unsent: bool) {
        self.dirty = false;
        self.urgent = false;
        self.unsent = unsent;
        if unsent {
            self.failures = self.failures.saturating_add(1);
            let wait = RETRY.saturating_mul(1 << self.failures.saturating_sub(1).min(10));
            self.next_try = now + wait.min(RETRY_MAX);
        } else {
            self.failures = 0;
        }
    }
}

struct Entry {
    ctx: Ctx,
    idle: Arc<Mutex<Idle>>,
    wake: Arc<tokio::sync::Notify>,
}

fn running() -> &'static Mutex<HashMap<PathBuf, Arc<Entry>>> {
    static R: OnceLock<Mutex<HashMap<PathBuf, Arc<Entry>>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn entry_of(root: &Path) -> Option<Arc<Entry>> {
    running()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(root)
        .cloned()
}

/// What moves when the workspace changes: the capture's generation, else the status as a fingerprint.
fn mark(ctx: &Ctx, root: &Path) -> u64 {
    let at = root.display().to_string();
    if let Some(a) = ctx.account.as_ref().and_then(|a| a.attached_at(&at)) {
        return a.handle.generation();
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Ok(entries) = super::git::open(root).and_then(|r| r.status()) {
        for e in entries {
            let t = std::fs::metadata(root.join(&e.path))
                .and_then(|m| m.modified())
                .ok();
            (e.path, e.size, t).hash(&mut h);
        }
    }
    h.finish()
}

/// Starts the workspace's auto-save task unless one runs; it ends when the workspace or its remote is gone.
pub fn ensure(ctx: &Ctx, root: &Path) {
    let entry = Arc::new(Entry {
        ctx: ctx.clone(),
        idle: Arc::default(),
        wake: Arc::default(),
    });
    {
        let mut all = running().lock().unwrap_or_else(|p| p.into_inner());
        if all.contains_key(root) {
            return;
        }
        all.insert(root.to_path_buf(), entry.clone());
    }
    let root = root.to_path_buf();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = tokio::time::sleep(LOOK) => {}
                () = entry.wake.notified() => {}
            }
            let connected = super::git::open(&root).is_ok_and(|r| r.origin_config().is_some());
            if !root.is_dir() || !connected {
                break;
            }
            turn(&entry, &root).await;
        }
        running()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&root);
    });
}

async fn turn(entry: &Entry, root: &Path) {
    let (ctx, root2) = (entry.ctx.clone(), root.to_path_buf());
    let m = tokio::task::spawn_blocking(move || mark(&ctx, &root2))
        .await
        .unwrap_or(0);
    let t = now();
    let (snap, retry) = {
        let mut i = entry.idle.lock().unwrap_or_else(|p| p.into_inner());
        i.seen(m, t);
        (i.due(t), i.retry_due(t))
    };
    if !(snap || retry) {
        return;
    }
    let done = round(&entry.ctx, root, snap, false).await;
    let (unsent, landed) = match &done {
        Ok(r) if r.skipped => return,
        Ok(r) => (r.unsent, r.landed > 0),
        Err(e) => {
            eprintln!("k7-front: the auto-save of {} stopped: {e}", root.display());
            (true, false)
        }
    };
    // The round's own landing moved the generation: read it again so it is not taken for an edit.
    let m = if landed {
        let (ctx, root2) = (entry.ctx.clone(), root.to_path_buf());
        tokio::task::spawn_blocking(move || mark(&ctx, &root2))
            .await
            .ok()
    } else {
        None
    };
    let mut i = entry.idle.lock().unwrap_or_else(|p| p.into_inner());
    i.done(now(), unsent);
    if m.is_some() {
        i.mark = m;
    }
}

/// The page left the workspace: snapshot and push at once.
pub fn flush_now(ctx: &Ctx, root: &Path) {
    ensure(ctx, root);
    if let Some(e) = entry_of(root) {
        e.idle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .now_please();
        e.wake.notify_one();
    }
}

/// A Save's push went through: the reset branch follows it without waiting for an edit.
pub fn push_now(ctx: &Ctx, root: &Path) {
    let (ctx, root) = (ctx.clone(), root.to_path_buf());
    let flight = in_flight();
    flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    tokio::spawn(async move {
        let _done = Flight(flight);
        // The route that called holds the remote until it answers: a skipped round tries again shortly.
        for _ in 0..25 {
            match round(&ctx, &root, false, false).await {
                Ok(r) if r.skipped => tokio::time::sleep(Duration::from_millis(200)).await,
                Ok(_) => return,
                Err(e) => {
                    eprintln!(
                        "k7-front: the auto-save branch of {} was not sent: {e}",
                        root.display()
                    );
                    return;
                }
            }
        }
    });
}

fn in_flight() -> Arc<std::sync::atomic::AtomicUsize> {
    static N: OnceLock<Arc<std::sync::atomic::AtomicUsize>> = OnceLock::new();
    N.get_or_init(Default::default).clone()
}

/// Counts a `push_now` round down when it ends, however it ends.
struct Flight(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for Flight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Test seam: waits until no round a route started is running, so a test's own lock is not taken from under it.
#[cfg(test)]
pub async fn quiet() {
    while in_flight().load(std::sync::atomic::Ordering::SeqCst) > 0 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The fetch task just fetched: take in what the other devices auto-saved.
pub async fn after_fetch(ctx: &Ctx, root: &Path) {
    if let Err(e) = round(ctx, root, false, true).await {
        eprintln!(
            "k7-front: the auto-saves of {} were not taken in: {e}",
            root.display()
        );
    }
}

/// The front stops: every workspace auto-saves once, each round bounded by `within`.
pub async fn flush_all(within: Duration) {
    let all: Vec<(PathBuf, Arc<Entry>)> = running()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .map(|(r, e)| (r.clone(), e.clone()))
        .collect();
    for (root, e) in all {
        let _ = tokio::time::timeout(within, round(&e.ctx, &root, true, false)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_waits_for_twenty_quiet_seconds_after_the_last_change() {
        let mut i = Idle::default();
        i.seen(1, 100);
        assert!(!i.due(119));
        assert!(i.due(120));
        i.seen(2, 110);
        assert!(!i.due(120), "a new change restarts the wait");
        assert!(i.due(130));
        i.done(130, false);
        i.seen(2, 200);
        assert!(!i.due(1000), "nothing changed since the round");
    }

    #[test]
    fn leaving_the_workspace_snapshots_without_waiting() {
        let mut i = Idle::default();
        i.seen(1, 0);
        i.done(0, false);
        i.now_please();
        assert!(i.due(1));
    }

    #[test]
    fn a_failed_push_is_retried_later_and_later() {
        let mut i = Idle::default();
        i.done(0, true);
        assert!(!i.retry_due(RETRY - 1));
        assert!(i.retry_due(RETRY));
        i.done(RETRY, true);
        assert!(!i.retry_due(RETRY + RETRY));
        assert!(i.retry_due(RETRY + 2 * RETRY));
        for n in 0..20 {
            i.done(n, true);
        }
        assert!(
            i.retry_due(19 + RETRY_MAX),
            "never more than RETRY_MAX apart"
        );
        i.done(0, false);
        assert!(
            !i.retry_due(u64::MAX),
            "a push that went through owes nothing"
        );
    }
}
