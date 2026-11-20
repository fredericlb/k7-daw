//! worker_watch.rs — a worker over the process's real watcher: a change on disk reaches it, and a stop unpins the root.
//! What a reader gets wrong: the watcher is leaked so it outlives the test's runtime; each test uses its own.

use std::sync::Arc;
use std::time::{Duration, Instant};

use k7_capture::capture::Options;
use k7_capture::source::Source;
use k7_capture::worker::{Deps, Window, spawn};
use k7_file_service::watch::{Sink, Watch};

use crate::rig::{files, put, settler};
use crate::support::Scratch;
use crate::worker::{BACKSTOP, Rec, timing};

#[tokio::test(flavor = "multi_thread")]
async fn the_process_watcher_feeds_a_worker_what_changed_on_disk() {
    let dir = Scratch::new("w-real");
    let root = dir.at("ws");
    std::fs::create_dir_all(&root).unwrap();
    let watch: &'static Watch = Box::leak(Box::new(Watch::new()));
    let rec = Arc::new(Rec::default());
    let d = Deps {
        source: Arc::new(WatchRef(watch)),
        out: rec.clone(),
        timing: timing(false),
        baseline: None,
        hook: None,
    };
    let h = spawn(
        Options::new(&root, "w-real", settler()),
        d,
        Arc::new(Window::default()),
    )
    .await
    .unwrap();
    assert!(h.watching());
    put(
        &root,
        "song/take.wav",
        "not a source, still news to a capture",
    );
    let t0 = Instant::now();
    while rec.batches.lock().unwrap().is_empty() {
        assert!(t0.elapsed() < BACKSTOP, "the watcher never told the worker");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        files(&rec.batches.lock().unwrap().first().cloned()),
        ["song/take.wav"]
    );
    h.stop().await;
    assert_eq!(watch.watched(), 0, "a stopped worker's root is unpinned");
}

struct WatchRef(&'static Watch);

impl Source for WatchRef {
    fn pin(&self, root: &str, sink: Sink) -> bool {
        self.0.pin(root, sink)
    }

    fn unpin(&self, root: &str, sink: &Sink) {
        self.0.unpin(root, sink);
    }

    fn up(&self, root: &str) -> bool {
        Source::up(self.0, root)
    }
}
