//! worker_window.rs — an entry's write phase against a verb's window, and a release the worker has not read yet.
//! What a reader gets wrong: a race is a hook or a sleeping write closure, never a bet on timing; each test fails without its fix.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use k7_capture::capture::Options;
use k7_capture::worker::{Window, spawn};

use crate::rig::{MemBaseline, files, put, settler};
use crate::support::Scratch;
use crate::worker::{BACKSTOP, Bed, Fake, Rec, deps};

#[tokio::test(flavor = "multi_thread")]
async fn windows_opened_in_a_race_never_overlap_an_entry_s_write_phase() {
    let b = Bed::new("w-race", false, None).await;
    let writing = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicUsize::new(0));
    let mut waits = vec![];
    for _ in 0..8 {
        let (h, writing, done) = (b.h.clone(), writing.clone(), done.clone());
        waits.push(tokio::spawn(async move {
            h.run_when_free(
                |_| (),
                move |_, ()| {
                    writing.store(true, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(2));
                    writing.store(false, Ordering::SeqCst);
                    done.fetch_add(1, Ordering::SeqCst);
                    Some(())
                },
            )
            .await
        }));
    }
    let racers: Vec<_> = (0..3)
        .map(|_| {
            let (h, writing, done) = (b.h.clone(), writing.clone(), done.clone());
            std::thread::spawn(move || {
                let t0 = Instant::now();
                while done.load(Ordering::SeqCst) < 8 && t0.elapsed() < BACKSTOP {
                    h.hold();
                    // A hold that returned while an entry still wrote would let a verb's file be overwritten.
                    assert!(!writing.load(Ordering::SeqCst), "a hold returned mid-write");
                    h.release("");
                }
            })
        })
        .collect();
    for r in racers {
        r.join().unwrap();
    }
    for w in waits {
        w.await.unwrap().unwrap();
    }
    assert_eq!(done.load(Ordering::SeqCst), 8, "each entry applied once");
    assert_eq!(b.h.window().count(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hold_waits_for_the_write_phase_it_finds_running() {
    let b = Bed::new("w-hold-wait", false, None).await;
    let (started, finished) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    let (s, f) = (started.clone(), finished.clone());
    let entry = tokio::spawn({
        let h = b.h.clone();
        async move {
            h.run_when_free(
                |_| (),
                move |_, ()| {
                    s.store(true, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(150));
                    f.store(true, Ordering::SeqCst);
                    Some(())
                },
            )
            .await
        }
    });
    b.until("the write phase", |_| started.load(Ordering::SeqCst))
        .await;
    let (h, f) = (b.h.clone(), finished.clone());
    let saw = std::thread::spawn(move || {
        h.hold();
        f.load(Ordering::SeqCst)
    })
    .join()
    .unwrap();
    assert!(
        saw,
        "the verb's hold returned before the entry finished writing"
    );
    entry.await.unwrap().unwrap();
    b.h.release("");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_entry_whose_window_opened_after_its_compute_runs_again_once_free() {
    let b = Bed::new("w-requeue", false, None).await;
    let (computes, writes) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (h, c, w) = (b.h.clone(), computes.clone(), writes.clone());
    let entry = tokio::spawn(async move {
        let win = h.window().clone();
        h.run_when_free(
            move |_| {
                // The verb's hold arrives after the first compute and before its write.
                if c.fetch_add(1, Ordering::SeqCst) == 0 {
                    win.hold();
                }
            },
            move |_, ()| {
                w.fetch_add(1, Ordering::SeqCst);
                Some(())
            },
        )
        .await
    });
    b.until("the first compute", |_| {
        computes.load(Ordering::SeqCst) == 1
    })
    .await;
    b.quiescent().await;
    assert_eq!(
        writes.load(Ordering::SeqCst),
        0,
        "written inside the window"
    );
    b.h.release("");
    entry.await.unwrap().unwrap();
    assert_eq!(
        (
            computes.load(Ordering::SeqCst),
            writes.load(Ordering::SeqCst)
        ),
        (2, 1)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_window_closed_behind_the_workers_back_still_makes_the_next_scan_sweep_whole() {
    let b = Bed::new("w-closed", false, None).await;
    let win = b.h.window().clone();
    let root = b.root.clone();
    // Only the window moves: the worker never reads a release, as when a flush wins the race against it.
    b.h.run(move |_| {
        win.hold();
        put(&root, "b.md", "the verb's file");
        win.release_for_test("verb");
    })
    .await
    .unwrap();
    put(&b.root, "a.md", "a");
    b.fake.report(&["a.md"]);
    b.until("a batch", |b| !b.batches().is_empty()).await;
    b.quiescent().await;
    let all = b.batches();
    assert_eq!(all.len(), 1, "one verb is one batch");
    assert_eq!(all[0]["label"], "verb");
    assert_eq!(files(&all.first().cloned()), ["a.md", "b.md"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_window_held_before_the_spawn_holds_the_baseline_diff_back_into_one_labelled_batch() {
    let dir = Scratch::new("w-first");
    let root = dir.at("ws");
    put(&root, "a.md", "changed while down");
    let (fake, rec) = (Fake::new(), Arc::new(Rec::default()));
    let mut d = deps(&fake, &rec, false, None);
    d.baseline = MemBaseline::of(&[("a.md", "as the room had it")]).arc();
    let win = Arc::new(Window::default());
    win.hold();
    let h = spawn(Options::new(&root, "w-first", settler()), d, win.clone())
        .await
        .unwrap();
    put(&root, "b.md", "the verb's file");
    h.run(|_| ()).await.unwrap();
    assert!(
        rec.batches.lock().unwrap().is_empty(),
        "the baseline was diffed inside a window"
    );
    h.release("verb");
    let t0 = Instant::now();
    while rec.batches.lock().unwrap().is_empty() {
        assert!(t0.elapsed() < BACKSTOP, "no batch after the release");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    let all = rec.batches.lock().unwrap().clone();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["label"], "verb");
    assert_eq!(files(&all.first().cloned()), ["a.md", "b.md"]);
    h.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_verb_that_wrote_and_released_during_a_compute_is_read_by_the_next_one() {
    let b = Bed::new("w-epoch", false, None).await;
    let (win, root) = (b.h.window().clone(), b.root.clone());
    let computes = Arc::new(AtomicUsize::new(0));
    let c = computes.clone();
    let seen =
        b.h.run_when_free(
            move |_| {
                // The verb opens, writes and closes inside the compute: the count is back at zero by the write phase.
                if c.fetch_add(1, Ordering::SeqCst) == 0 {
                    win.hold();
                    put(&root, "f.md", "the verb's text");
                    win.release_for_test("verb");
                }
                std::fs::read_to_string(format!("{root}/f.md")).unwrap_or_default()
            },
            |_, seen| Some(seen),
        )
        .await
        .unwrap();
    assert_eq!(
        computes.load(Ordering::SeqCst),
        2,
        "the write trusted a compute that a verb overtook"
    );
    assert_eq!(seen, "the verb's text");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_that_answers_none_is_computed_again_and_never_counts_as_applied() {
    let b = Bed::new("w-again", false, None).await;
    let (computes, writes) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (c, w) = (computes.clone(), writes.clone());
    let got =
        b.h.run_when_free(
            move |_| c.fetch_add(1, Ordering::SeqCst),
            move |_, n| (w.fetch_add(1, Ordering::SeqCst) >= 3).then_some(n),
        )
        .await;
    assert_eq!(got, Some(3), "the answer is the compute that was written");
    assert_eq!(
        (
            computes.load(Ordering::SeqCst),
            writes.load(Ordering::SeqCst)
        ),
        (4, 4)
    );
}

static FAIL_ONCE: AtomicBool = AtomicBool::new(true);

fn id_fails_once() -> std::io::Result<String> {
    if FAIL_ONCE.swap(false, Ordering::SeqCst) {
        return Err(std::io::Error::other("no id this time"));
    }
    k7_capture::disk::uuid_v4()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_baseline_scan_that_fails_is_noted_and_the_first_sweep_sends_the_edit() {
    let dir = Scratch::new("w-base-fail");
    let root = dir.at("ws");
    put(&root, "a.md", "changed while down");
    let (fake, rec) = (Fake::new(), Arc::new(Rec::default()));
    let mut d = deps(&fake, &rec, true, None);
    d.baseline = MemBaseline::of(&[("a.md", "as the room had it")]).arc();
    let mut o = Options::new(&root, "w-base-fail", settler());
    o.new_id = id_fails_once;
    let h = spawn(o, d, Arc::new(Window::default())).await.unwrap();
    let t0 = Instant::now();
    while rec.batches.lock().unwrap().is_empty() {
        assert!(
            t0.elapsed() < BACKSTOP,
            "the edit was lost with the failed scan"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let notes = rec.notes.lock().unwrap().clone();
    assert!(
        notes.iter().any(|n| n.contains("no id this time")),
        "the failure was not said: {notes:?}"
    );
    assert_eq!(
        files(&rec.batches.lock().unwrap().first().cloned()),
        ["a.md"]
    );
    h.stop().await;
}
