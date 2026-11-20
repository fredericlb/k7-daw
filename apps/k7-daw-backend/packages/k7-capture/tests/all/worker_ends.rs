//! worker_ends.rs — how a worker ends: every handle dropped, a step that panics, and no file call on an async thread.
//! What a reader gets wrong: the async-thread oracle is `block_on`, which panics inside a runtime thread and not on the blocking pool.

use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use k7_capture::capture::Options;
use k7_capture::worker::spawn;

use crate::rig::{put, settler};
use crate::worker::{BACKSTOP, Bed};

/// True when the caller runs on a runtime's async thread, which a file call must never.
pub fn on_async_thread() -> bool {
    let Ok(rt) = tokio::runtime::Handle::try_current() else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| rt.block_on(async {}))).is_err()
}

static ID_ON_ASYNC: AtomicBool = AtomicBool::new(false);

fn watched_id() -> io::Result<String> {
    if on_async_thread() {
        ID_ON_ASYNC.store(true, Ordering::SeqCst);
    }
    k7_capture::disk::uuid_v4()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_worker_whose_handles_are_all_dropped_ends_and_unpins_its_root() {
    let b = Bed::new("e-drop", false, None).await;
    let fake = b.fake.clone();
    assert!(fake.pinned());
    drop(b);
    let t0 = Instant::now();
    while fake.pinned() {
        assert!(t0.elapsed() < BACKSTOP, "the worker outlived its handles");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_step_that_panics_stops_the_worker_unpins_its_root_and_answers_no_more() {
    let b = Bed::new("e-panic", false, None).await;
    assert!(b.h.run(|_| panic!("a step that fails")).await.is_none());
    b.until("stopped", |b| b.h.stopped()).await;
    assert!(!b.h.watching());
    assert!(!b.fake.pinned());
    assert!(b.h.run(|_| ()).await.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pin_the_commit_and_the_unpin_never_run_on_an_async_thread() {
    let dir = crate::support::Scratch::new("e-async");
    let root = dir.at("ws");
    std::fs::create_dir_all(&root).unwrap();
    let (fake, rec) = (
        crate::worker::Fake::new(),
        std::sync::Arc::new(crate::worker::Rec::default()),
    );
    fake.watch_threads();
    let mut o = Options::new(&root, "e-async", settler());
    o.new_id = watched_id;
    let d = crate::worker::deps(&fake, &rec, false, None);
    let h = spawn(o, d, Default::default()).await.unwrap();
    put(&root, "a.md", "a");
    fake.report(&["a.md"]);
    let t0 = Instant::now();
    while rec.batches.lock().unwrap().is_empty() {
        assert!(t0.elapsed() < BACKSTOP, "no batch");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    h.stop().await;
    assert!(
        !ID_ON_ASYNC.load(Ordering::SeqCst),
        "a commit ran on an async thread"
    );
    assert!(!fake.on_async(), "a pin or an unpin ran on an async thread");
}
