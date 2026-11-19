//! jobs_separations.rs — `separation_id` against the TypeScript's, and the registry's join, keep and failure rules (2330).
//! A reader gets wrong that a record outlives its sweep: only `start` sweeps, so a poll can still read a job kept past five minutes.

use k7_file_service::jobs::separations::{KEEP, Registry, separation_id};
use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

fn record(v: Value) -> Map<String, Value> {
    let Value::Object(m) = v else {
        panic!("an object")
    };
    m
}

#[test]
fn work_in_a_different_order_of_outputs_is_other_work_and_the_same_work_joins() {
    let a = separation_id(
        "me",
        "/m/a.wav",
        "/m.onnx",
        &["vocals".into(), "drums".into()],
    );
    let b = separation_id(
        "me",
        "/m/a.wav",
        "/m.onnx",
        &["drums".into(), "vocals".into()],
    );
    assert_ne!(a, b);
    let t0 = Instant::now();
    let mut r = Registry::new();
    let first = r.start(t0, &a, "/m/a.wav", true);
    assert!(first.send);
    assert_eq!(first.job["phase"], "sending");
    let again = r.start(t0, &a, "/m/a.wav", true);
    assert!(!again.send, "a running job is joined, not asked twice");
    assert_eq!(again.generation, first.generation);
    // A thousand starts of one work are one job.
    for _ in 0..1000 {
        r.start(t0, &a, "/m/a.wav", true);
    }
    assert_eq!(r.len(), 1);
}

#[test]
fn the_machines_record_is_taken_with_this_sides_id_and_a_finished_job_is_kept_five_minutes() {
    let t0 = Instant::now();
    let mut r = Registry::new();
    let s = r.start(t0, "me1", "/m/a.wav", true);
    r.progress(t0, "me1", s.generation, record(json!({ "id": "theirs", "source": "/m/a.wav", "done": true, "failed": false, "error": null, "phase": "done", "files": ["/o/a.wav"] })));
    let job = r.job("me1").unwrap();
    assert_eq!(job["id"], "me1");
    assert_eq!(job["files"], json!(["/o/a.wav"]));
    // Done work is not joined: the same work again is a new job, whose answer comes with a request to send.
    let again = r.start(t0 + KEEP, "me1", "/m/a.wav", true);
    assert!(again.send);
    assert_ne!(again.generation, s.generation);
    // The old request's late answers say nothing in the new record.
    r.fail(t0 + KEEP, "me1", s.generation, true, "late");
    assert_eq!(r.job("me1").unwrap()["failed"], false);
    r.progress(
        t0 + KEEP,
        "me1",
        s.generation,
        record(json!({ "done": true, "phase": "done", "files": ["/late"] })),
    );
    assert_eq!(
        r.job("me1").unwrap()["done"],
        false,
        "a late record of the old request is not the new job's"
    );
    // Only `start` sweeps: a poll past five minutes still reads a job, and the next start drops it.
    let mut r = Registry::new();
    let s = r.start(t0, "x", "/x", true);
    r.fail(t0, "x", s.generation, true, "boom");
    let late = t0 + KEEP + Duration::from_secs(1);
    assert!(r.job("x").is_some());
    r.start(late, "y", "/y", true);
    assert!(r.job("x").is_none());
    // At exactly five minutes a finished job is still kept.
    let mut r = Registry::new();
    let s = r.start(t0, "x", "/x", true);
    r.fail(t0, "x", s.generation, true, "boom");
    r.start(t0 + KEEP, "y", "/y", true);
    assert!(r.job("x").is_some());
}
