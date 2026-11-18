//! The link's ordering and lock-order guarantees: a frame never leaves before the document asked ahead of it (2216, D2), and
//! a binding told while a session sends never deadlocks (the lock order in mod.rs).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use super::fake::{FakeJobs, Rig, until};

fn first(kinds: &[String], what: &str) -> Option<usize> {
    kinds.iter().position(|k| k == what)
}

#[tokio::test]
async fn a_frame_asked_after_a_document_leaves_after_it() {
    let rig = Rig::new("order").await;
    let s = rig.session();
    s.push_doc(super::fake::minimal_doc(-3.0));
    s.frame(json!({ "type": "preview", "file": "x.wav", "start": 0, "length": 1 }));
    until(5, "the preview", || {
        first(&rig.engine.kinds(Rig::SESSION), "preview").is_some()
    })
    .await;
    let kinds = rig.engine.kinds(Rig::SESSION);
    let (doc, preview) = (first(&kinds, "project"), first(&kinds, "preview"));
    assert!(doc.is_some() && doc < preview, "{kinds:?}");
}

#[tokio::test]
async fn documents_pushed_in_a_row_are_decided_in_the_order_asked() {
    let rig = Rig::new("docs").await;
    let s = rig.session();
    for n in 0..8 {
        s.push_doc(super::fake::minimal_doc(-(n as f64)));
    }
    until(10, "the last document", || {
        rig.engine
            .of(Rig::SESSION)
            .iter()
            .any(|f| f["type"] == "project" && f["doc"]["masterVolume"] == json!(-7.0))
    })
    .await;
    let masters: Vec<Value> = rig
        .engine
        .of(Rig::SESSION)
        .iter()
        .filter(|f| f["type"] == "project")
        .map(|f| f["doc"]["masterVolume"].clone())
        .filter(|m| m.as_f64().is_some_and(|v| v > -100.0))
        .collect();
    let mut sorted = masters.clone();
    sorted.sort_by(|a, b| b.as_f64().partial_cmp(&a.as_f64()).unwrap());
    sorted.dedup();
    let mut seen = masters.clone();
    seen.dedup();
    assert_eq!(seen, sorted, "{masters:?}");
}

#[tokio::test]
async fn a_copy_that_takes_long_does_not_hold_a_stop_behind_it() {
    let rig = Rig::new("parked").await;
    let jobs = FakeJobs::start().await;
    jobs.takes(Duration::from_secs(2));
    rig.link
        .bind(&json!({ "url": rig.engine.url, "jobsUrl": jobs.url, "engineId": "e1" }));
    let p = format!("{}/ws/a.wav", rig.dir.display());
    std::fs::write(&p, "x".repeat(10)).unwrap();
    let mut doc = super::fake::minimal_doc(-3.0);
    doc["recordDir"] = json!(p);
    let s = rig.session();
    s.push_doc(doc);
    until(5, "the job", || jobs.asked().len() == 1).await;
    s.frame(json!({ "type": "flush" }));
    until(1, "the flush while the copy runs", || {
        first(&rig.engine.kinds(Rig::SESSION), "flush").is_some()
    })
    .await;
    assert!(first(&rig.engine.kinds(Rig::SESSION), "project").is_none());
}

// A job that panics is counted and the queue goes on; the next document still reaches the engine (B1).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_job_that_panics_does_not_end_the_queue() {
    let rig = Rig::new("panic").await;
    let s = rig.session();
    s.enqueue(|_| async { panic!("a job that dies") });
    let (tx, rx) = std::sync::mpsc::channel();
    let s2 = s.clone();
    // From a plain thread, as the runtime's is: the enqueue must return.
    std::thread::spawn(move || {
        s2.push_doc(super::fake::minimal_doc(-3.0));
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(Duration::from_secs(5)).is_ok(),
        "the enqueue hung"
    );
    until(5, "the document", || {
        first(&rig.engine.kinds(Rig::SESSION), "project").is_some()
    })
    .await;
    assert_eq!(super::stats::Stats::get(&rig.link.stats().panics), 1);
}

// A frame asked before a later document leaves before it, whatever that document takes to decide (D5).
#[tokio::test]
async fn a_frame_never_waits_for_a_document_asked_after_it() {
    let rig = Rig::new("strict").await;
    let s = rig.session();
    s.enqueue(|_| async { tokio::time::sleep(Duration::from_millis(400)).await });
    s.frame(json!({ "type": "preview", "file": "x.wav", "start": 0, "length": 1 }));
    s.push_doc(super::fake::minimal_doc(-3.0));
    until(5, "the document", || {
        first(&rig.engine.kinds(Rig::SESSION), "project").is_some()
    })
    .await;
    let kinds = rig.engine.kinds(Rig::SESSION);
    let (preview, doc) = (first(&kinds, "preview"), first(&kinds, "project"));
    assert!(preview.is_some() && preview < doc, "{kinds:?}");
}

// An unload under a folder is decided after the start queued before it, so nothing is served once it answers (J3 fix 1).
#[tokio::test]
async fn an_unload_under_a_folder_follows_the_start_queued_before_it() {
    let rig = Rig::new("under").await;
    let s = rig.session();
    let (ws, project) = (rig.ws.clone(), rig.project.clone());
    s.enqueue(move |s| async move { s.start(&ws, &project).await });
    let any = rig.link.unload_under(&rig.project).await;
    assert!(any, "the queued start was missed");
    assert!(s.serving().is_none());
}

// The deadlock check owns its runtime: a deadlocked worker would make a dropped runtime wait for ever.
#[test]
fn a_binding_told_while_a_session_sends_does_not_deadlock() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let rig = rt.block_on(async {
        let rig = Rig::new("locks").await;
        let s = rig.session();
        s.frame(json!({ "type": "flush" }));
        until(5, "open", || s.is_ready()).await;
        rig
    });
    let s = rig.session();
    let stop = Arc::new(AtomicBool::new(false));
    let (done, finished) = std::sync::mpsc::channel();
    let binder = {
        let (link, stop, done) = (rig.link.clone(), stop.clone(), done.clone());
        let told = json!({ "url": rig.engine.url, "engineId": "e1" });
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                link.bind(&told);
            }
            let _ = done.send(());
        })
    };
    let sender = {
        let (s, stop) = (s.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                s.drop_socket();
                s.frame(json!({ "type": "flush" }));
            }
            let _ = done.send(());
        })
    };
    std::thread::sleep(Duration::from_millis(1500));
    stop.store(true, Ordering::Relaxed);
    let both = (0..2).all(|_| finished.recv_timeout(Duration::from_secs(5)).is_ok());
    if !both {
        std::mem::forget(rig);
        rt.shutdown_background();
        panic!("bind and a session's send wait on each other");
    }
    let _ = (binder.join(), sender.join());
    drop(rig);
    rt.shutdown_timeout(Duration::from_secs(5));
}

// A switch while the dry run plays: the replaced run's stop leaves before the next project's document (2220).
#[tokio::test]
async fn a_switch_stops_the_replaced_dry_run_before_the_next_document() {
    let rig = Rig::new("switch").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(10, "the dry run's play", || {
        first(&rig.engine.kinds(Rig::SESSION), "transport play").is_some()
    })
    .await;
    let second = format!("{}/q.k7", rig.ws);
    std::fs::create_dir_all(&second).unwrap();
    let before = rig.engine.kinds(Rig::SESSION).len();
    s.start(&rig.ws, &second).await;
    until(10, "the second document", || {
        rig.engine.kinds(Rig::SESSION)[before..]
            .iter()
            .any(|k| k == "project")
    })
    .await;
    let after: Vec<String> = rig.engine.kinds(Rig::SESSION)[before..].to_vec();
    let (stop, doc) = (first(&after, "transport stop"), first(&after, "project"));
    assert!(stop.is_some() && stop < doc, "{after:?}");
}
