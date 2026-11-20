//! worker.rs — the per-workspace worker: its timers, the window's epoch, the entries held back, the feed and the pinned
//! root. What a reader gets wrong: a race is made by a hook between a scan's walk and its commit, never by a sleep; a
//! sleep only waits out what must NOT happen, and every wait for what must happen is a condition with a backstop.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use k7_capture::capture::{Baseline, Options};
use k7_capture::feed::Feed;
use k7_capture::source::Source;
use k7_capture::worker::{Deps, Handle, Hook, Out, Timing, Window, spawn};
use k7_file_service::watch::Sink;
use k7_replica::blob::BlobId;
use serde_json::Value;

use crate::rig::{files, get, put, settler};
use crate::support::Scratch;

pub const BACKSTOP: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

#[derive(Default)]
pub struct Rec {
    pub batches: Mutex<Vec<Value>>,
    pub feeds: Mutex<Vec<(Vec<String>, u64)>>,
    pub notes: Mutex<Vec<String>>,
}

impl Out for Rec {
    fn batch(&self, _: &str, batch: Value) {
        self.batches.lock().unwrap().push(batch);
    }

    fn missing(&self, _: &str, _: Vec<BlobId>) {}

    fn note(&self, _: &str, line: &str) {
        self.notes.lock().unwrap().push(line.to_string());
    }

    fn feed(&self, _: &str, paths: Vec<String>, generation: u64) {
        self.feeds.lock().unwrap().push((paths, generation));
    }
}

/// A watcher the test drives: it reports what it is told, and goes up and down on request.
#[derive(Default)]
pub struct Fake {
    sink: Mutex<Option<Sink>>,
    pub up: AtomicBool,
    pub pins: AtomicUsize,
    pub unpins: AtomicUsize,
    watch_threads: AtomicBool,
    async_calls: AtomicBool,
}

impl Fake {
    pub fn new() -> Arc<Fake> {
        let f = Arc::new(Fake::default());
        f.up.store(true, Ordering::SeqCst);
        f
    }

    pub fn report(&self, rels: &[&str]) {
        let sink = self.sink.lock().unwrap().clone().expect("pinned");
        sink(rels.iter().map(|r| r.to_string()).collect());
    }

    /// From now on a pin or an unpin made on an async thread is remembered.
    pub fn watch_threads(&self) {
        self.watch_threads.store(true, Ordering::SeqCst);
    }

    pub fn on_async(&self) -> bool {
        self.async_calls.load(Ordering::SeqCst)
    }

    fn seen(&self) {
        if self.watch_threads.load(Ordering::SeqCst) && crate::worker_ends::on_async_thread() {
            self.async_calls.store(true, Ordering::SeqCst);
        }
    }

    pub fn pinned(&self) -> bool {
        self.sink.lock().unwrap().is_some()
    }
}

impl Source for Fake {
    fn pin(&self, _: &str, sink: Sink) -> bool {
        self.seen();
        self.pins.fetch_add(1, Ordering::SeqCst);
        *self.sink.lock().unwrap() = Some(sink);
        self.up.load(Ordering::SeqCst)
    }

    fn unpin(&self, _: &str, sink: &Sink) {
        self.seen();
        self.unpins.fetch_add(1, Ordering::SeqCst);
        let mut mine = self.sink.lock().unwrap();
        if mine.as_ref().is_some_and(|s| Arc::ptr_eq(s, sink)) {
            *mine = None;
        }
    }

    fn up(&self, _: &str) -> bool {
        self.up.load(Ordering::SeqCst)
    }
}

pub struct Bed {
    _dir: Scratch,
    pub root: String,
    pub fake: Arc<Fake>,
    pub rec: Arc<Rec>,
    pub h: Handle,
}

/// Timers short enough for a test; `sweeps` off leaves only what the watcher reports and the release asks for.
pub fn timing(sweeps: bool) -> Timing {
    let long = Duration::from_secs(3600);
    Timing {
        quiet: Duration::from_millis(20),
        feed: Duration::from_millis(20),
        first_sweep: if sweeps {
            Duration::from_millis(60)
        } else {
            long
        },
        sweep: if sweeps {
            Duration::from_millis(60)
        } else {
            long
        },
    }
}

pub fn deps(fake: &Arc<Fake>, rec: &Arc<Rec>, sweeps: bool, hook: Option<Hook>) -> Deps {
    Deps {
        source: fake.clone(),
        out: rec.clone(),
        timing: timing(sweeps),
        baseline: None,
        hook,
    }
}

impl Bed {
    pub async fn new(name: &str, sweeps: bool, hook: Option<Hook>) -> Bed {
        Bed::timed(name, timing(sweeps), hook).await
    }

    pub async fn timed(name: &str, timing: Timing, hook: Option<Hook>) -> Bed {
        let dir = Scratch::new(name);
        let root = dir.at("ws");
        std::fs::create_dir_all(&root).unwrap();
        let (fake, rec) = (Fake::new(), Arc::new(Rec::default()));
        let mut o = Options::new(&root, name, settler());
        o.blob_dir = None;
        let mut d = deps(&fake, &rec, false, hook);
        d.timing = timing;
        let h = spawn(o, d, Arc::new(Window::default())).await.unwrap();
        Bed {
            _dir: dir,
            root,
            fake,
            rec,
            h,
        }
    }

    pub fn batches(&self) -> Vec<Value> {
        self.rec.batches.lock().unwrap().clone()
    }

    /// Waits for what must happen.
    pub async fn until(&self, what: &str, cond: impl Fn(&Bed) -> bool) {
        let t0 = Instant::now();
        while !cond(self) {
            assert!(t0.elapsed() < BACKSTOP, "never happened: {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// The worker has handled everything sent so far, and several quiet periods have passed.
    pub async fn quiescent(&self) {
        self.h.run(|_| ()).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reported_path_is_one_batch_after_the_quiet_period_and_the_feed_hears_it() {
    let b = Bed::new("w-one", false, None).await;
    put(&b.root, "a.md", "a");
    b.fake.report(&["a.md"]);
    b.until("a batch", |b| !b.batches().is_empty()).await;
    assert_eq!(files(&b.batches().first().cloned()), ["a.md"]);
    b.until("a feed post", |b| !b.rec.feeds.lock().unwrap().is_empty())
        .await;
    let feeds = b.rec.feeds.lock().unwrap().clone();
    assert_eq!(feeds[0].0, ["a.md"]);
    assert!(feeds[0].1 > 0 && b.h.generation() >= feeds[0].1);
    assert!(b.h.watching());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_reports_is_one_batch_and_one_feed_post() {
    // Gathering windows far longer than a descheduled test thread, so the burst is one by count, not by luck.
    let slow = Timing {
        quiet: Duration::from_millis(600),
        feed: Duration::from_millis(600),
        ..timing(false)
    };
    let b = Bed::timed("w-burst", slow, None).await;
    for f in ["x.md", "y.md", "z.md"] {
        put(&b.root, f, f);
        b.fake.report(&[f]);
    }
    b.until("the batch", |b| !b.batches().is_empty()).await;
    b.quiescent().await;
    assert_eq!(b.batches().len(), 1);
    assert_eq!(
        files(&b.batches().first().cloned()),
        ["x.md", "y.md", "z.md"]
    );
    // The reports are one post; the batch's own files are news again (a sweep may find what a watcher missed).
    let feeds = b.rec.feeds.lock().unwrap().clone();
    assert!(feeds.len() <= 2, "{feeds:?}");
    assert_eq!(feeds[0].0, ["x.md", "y.md", "z.md"]);
}

/// A hook that runs `act` in the first scan only, between its walk and its commit.
fn once(act: impl Fn() + Send + Sync + 'static) -> Hook {
    let done = AtomicBool::new(false);
    Arc::new(move || {
        if !done.swap(true, Ordering::SeqCst) {
            act();
        }
    })
}

/// A bed whose first scan runs `act(handle, root)` after its walk.
async fn bed_with_hook(name: &str, act: impl Fn(&Handle, &str) + Send + Sync + 'static) -> Bed {
    let cell: Arc<OnceLock<(Handle, String)>> = Arc::new(OnceLock::new());
    let seen = cell.clone();
    let hook = once(move || {
        if let Some((h, root)) = seen.get() {
            act(h, root);
        }
    });
    let b = Bed::new(name, false, Some(hook)).await;
    let _ = cell.set((b.h.clone(), b.root.clone()));
    b
}

#[tokio::test(flavor = "multi_thread")]
async fn a_window_held_during_a_scan_drops_it_and_the_release_sweeps_again() {
    let b = bed_with_hook("w-hold", |h, root| {
        h.hold();
        put(root, "b.md", "half a window");
    })
    .await;
    put(&b.root, "a.md", "a");
    b.fake.report(&["a.md"]);
    b.quiescent().await;
    assert!(b.batches().is_empty(), "half a window must not be emitted");
    b.h.release("mix");
    b.until("the whole window", |b| !b.batches().is_empty())
        .await;
    b.quiescent().await;
    let all = b.batches();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["label"], "mix");
    assert_eq!(files(&all.first().cloned()), ["a.md", "b.md"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_window_opened_and_closed_during_a_scan_still_drops_it() {
    let b = bed_with_hook("w-epoch", |h, root| {
        h.hold();
        put(root, "b.md", "the verb's second file");
        h.release("quick");
    })
    .await;
    put(&b.root, "a.md", "a");
    b.fake.report(&["a.md"]);
    b.until("a batch", |b| !b.batches().is_empty()).await;
    b.quiescent().await;
    let all = b.batches();
    assert_eq!(
        all.len(),
        1,
        "one verb is one batch, not the half the scan saw"
    );
    assert_eq!(all[0]["label"], "quick");
    assert_eq!(files(&all.first().cloned()), ["a.md", "b.md"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_emitted_while_a_window_is_open_and_two_labels_join() {
    let b = Bed::new("w-quiet", false, None).await;
    b.h.hold();
    b.h.hold();
    put(&b.root, "a.md", "a");
    b.fake.report(&["a.md"]);
    b.quiescent().await;
    assert!(b.batches().is_empty());
    b.h.release("a");
    b.quiescent().await;
    assert!(b.batches().is_empty(), "one window is still open");
    b.h.release("b");
    b.until("the batch", |b| !b.batches().is_empty()).await;
    assert_eq!(b.batches()[0]["label"], "a + b");
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_wait_for_every_window_and_apply_in_order() {
    let b = Bed::new("w-entries", false, None).await;
    b.h.hold();
    let applied = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));
    // One task asks both, so the order they are asked in is the order of the code.
    let (h, log) = (b.h.clone(), applied.clone());
    let both = tokio::spawn(async move {
        let entry = |n: u32| {
            let (win, log) = (h.window().clone(), log.clone());
            h.run_when_free(
                |_| (),
                move |_, ()| {
                    log.lock().unwrap().push((n, win.count()));
                    Some(())
                },
            )
        };
        tokio::join!(entry(1), entry(2))
    });
    b.quiescent().await;
    assert!(
        applied.lock().unwrap().is_empty(),
        "an entry applied inside a window"
    );
    b.h.release("");
    let (one, two) = both.await.unwrap();
    assert!(one.is_some() && two.is_some());
    assert_eq!(*applied.lock().unwrap(), [(1, 0), (2, 0)]);
}

#[test]
fn a_release_without_a_hold_clamps_at_zero() {
    let w = Window::default();
    w.release_for_test("");
    assert_eq!(w.count(), 0);
    w.hold();
    assert_eq!((w.count(), w.epoch()), (1, 2));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sweep_finds_what_the_watcher_never_reported() {
    let b = Bed::new("w-sweep", true, None).await;
    put(&b.root, "lost.md", "nobody told");
    b.until("the sweep's batch", |b| !b.batches().is_empty())
        .await;
    assert_eq!(files(&b.batches().first().cloned()), ["lost.md"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_watcher_that_goes_down_is_seen_and_pinned_again() {
    let b = Bed::new("w-down", true, None).await;
    let pins = b.fake.pins.load(Ordering::SeqCst);
    b.fake.up.store(false, Ordering::SeqCst);
    b.until("watching to read false", |b| !b.h.watching()).await;
    b.until("a new pin", |b| b.fake.pins.load(Ordering::SeqCst) > pins)
        .await;
    b.fake.up.store(true, Ordering::SeqCst);
    b.until("watching again", |b| b.h.watching()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_remote_entry_is_written_without_an_echo_and_reaches_the_feed() {
    let b = Bed::new("w-remote", true, None).await;
    b.h.run_when_free(
        |_| (),
        |c, ()| {
            c.write_known("room.md", Some("from the room")).unwrap();
            Some(())
        },
    )
    .await
    .unwrap();
    assert_eq!(get(&b.root, "room.md"), "from the room");
    b.until("the feed", |b| {
        b.rec
            .feeds
            .lock()
            .unwrap()
            .iter()
            .any(|f| f.0 == ["room.md"])
    })
    .await;
    b.quiescent().await;
    assert!(
        b.batches().is_empty(),
        "a write for another replica is not an edit"
    );
}

struct Broken;

impl Baseline for Broken {
    fn files(&self) -> std::io::Result<Vec<String>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "locked",
        ))
    }

    fn get(&self, _: &str) -> std::io::Result<Option<String>> {
        Ok(None)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_start_that_fails_leaves_nothing_pinned_nor_emitted() {
    let dir = Scratch::new("w-fail");
    let root = dir.at("ws");
    std::fs::create_dir_all(&root).unwrap();
    put(&root, "a.md", "a");
    let (fake, rec) = (Fake::new(), Arc::new(Rec::default()));
    let mut d = deps(&fake, &rec, true, None);
    d.baseline = Some(Arc::new(Broken));
    let started = spawn(
        Options::new(&root, "w-fail", settler()),
        d,
        Arc::new(Window::default()),
    )
    .await;
    assert!(started.is_err());
    assert!(!fake.pinned());
    assert!(rec.batches.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_worker_unpins_publishes_it_and_answers_no_more() {
    let b = Bed::new("w-stop", true, None).await;
    b.h.stop().await;
    assert!(b.h.stopped() && !b.h.watching());
    assert!(!b.fake.pinned());
    assert!(b.h.run(|_| ()).await.is_none());
    assert!(b.h.run_when_free(|_| (), |_, ()| Some(())).await.is_none());
}

#[test]
fn the_feed_gathers_in_order_without_repeating_and_starts_afresh() {
    let mut f = Feed::new(Duration::from_millis(100));
    let t = Instant::now();
    assert!(f.due().is_none());
    f.push(vec!["b".into(), "a".into()], t);
    f.push(vec!["a".into(), "c".into()], t + Duration::from_millis(50));
    assert_eq!(f.due(), Some(t + Duration::from_millis(100)));
    assert_eq!(f.take(), ["b", "a", "c"]);
    assert!(f.due().is_none() && f.take().is_empty());
}
