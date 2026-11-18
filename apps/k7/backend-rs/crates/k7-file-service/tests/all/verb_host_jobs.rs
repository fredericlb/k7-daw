//! The job table (3f): a job's life, the heartbeat that stops it, forgetting, and whose id it is. The clock is turned by hand.

use std::time::Duration;

use k7_file_service::verb_host::jobs::{HEARTBEAT, KEEP, MAX_RUNNING, Table};
use serde_json::json;

const T0: Duration = Duration::from_secs(1_000);

fn at(s: u64) -> Duration {
    T0 + Duration::from_secs(s)
}

#[test]
fn a_job_runs_reports_and_ends_with_its_value() {
    let t = Table::default();
    let (id, _) = t.start("ann", true, T0).unwrap();
    assert_eq!(id.len(), 32);
    assert_eq!(
        t.poll("ann", &id, at(1)).unwrap(),
        json!({ "state": "running", "fraction": 0.0, "phase": "" })
    );
    t.progress(&id, 0.5, "mixing");
    assert_eq!(t.poll("ann", &id, at(2)).unwrap()["fraction"], json!(0.5));
    assert!(t.finish(&id, json!({ "files": [] }), at(3)));
    assert_eq!(
        t.poll("ann", &id, at(4)).unwrap(),
        json!({ "state": "done", "value": { "files": [] } })
    );
    assert!(
        !t.fail(&id, "late", None, at(5)),
        "an ended job keeps its end"
    );
}

#[test]
fn a_failed_job_says_why_and_its_code() {
    let t = Table::default();
    let (id, _) = t.start("ann", true, T0).unwrap();
    t.fail(&id, "no machine", Some("no-machine"), at(1));
    assert_eq!(
        t.poll("ann", &id, at(2)).unwrap(),
        json!({ "state": "failed", "error": "no machine", "code": "no-machine" })
    );
}

// 3-B3: a job nobody polls for a minute is stopped; each poll is the heartbeat.
#[test]
fn a_job_nobody_polls_is_stopped_after_the_heartbeat() {
    let t = Table::default();
    let (id, cancel) = t.start("ann", true, T0).unwrap();
    assert!(
        t.tick(T0 + HEARTBEAT).is_empty(),
        "the minute itself is alive"
    );
    t.poll("ann", &id, at(50));
    assert!(t.tick(at(100)).is_empty(), "a poll moved the deadline");
    assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(
        t.tick(at(50) + HEARTBEAT + Duration::from_secs(1)),
        std::slice::from_ref(&id)
    );
    assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
    let a = t.poll("ann", &id, at(120)).unwrap();
    assert_eq!(
        (a["state"].as_str(), a["code"].as_str()),
        (Some("failed"), Some("gone"))
    );
    assert!(
        !t.finish(&id, json!(1), at(121)),
        "the driver's late value does not undo the stop"
    );
}

// `prepare` and `picture` outlive the page.
#[test]
fn a_job_without_a_heartbeat_is_never_stopped() {
    let t = Table::default();
    let (id, cancel) = t.start("ann", false, T0).unwrap();
    assert!(t.tick(at(10_000)).is_empty());
    assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(
        t.poll("ann", &id, at(10_001)).unwrap()["state"],
        json!("running")
    );
}

// The record outlives a slow poll by ten minutes, then it is a 404.
#[test]
fn a_finished_job_is_forgotten_ten_minutes_after_its_end() {
    let t = Table::default();
    let (id, _) = t.start("ann", true, T0).unwrap();
    t.finish(&id, json!(1), at(5));
    assert!(t.poll("ann", &id, at(5) + KEEP).is_some());
    assert!(
        t.poll("ann", &id, at(5) + KEEP + Duration::from_secs(1))
            .is_none()
    );
    assert!(
        t.poll("ann", &id, at(6)).is_none(),
        "forgotten stays forgotten"
    );
    let (id2, _) = t.start("ann", true, T0).unwrap();
    t.finish(&id2, json!(1), at(5));
    t.tick(at(5) + KEEP + Duration::from_secs(1));
    assert!(t.poll("ann", &id2, at(5)).is_none(), "a tick sweeps too");
}

// An id is no capability across accounts, and a guess is no job.
#[test]
fn another_accounts_id_and_a_guessed_one_are_unknown() {
    let t = Table::default();
    let (id, _) = t.start("ann", true, T0).unwrap();
    assert!(t.poll("bob", &id, at(1)).is_none());
    assert!(
        t.poll("ann", "0123456789abcdef0123456789abcdef", at(1))
            .is_none()
    );
    assert!(t.poll("ann", "", at(1)).is_none());
    let (other, _) = t.start("ann", true, T0).unwrap();
    assert_ne!(id, other);
}

#[test]
fn an_account_runs_a_bounded_number_of_jobs() {
    let t = Table::default();
    let ids: Vec<String> = (0..MAX_RUNNING)
        .map(|_| t.start("ann", true, T0).unwrap().0)
        .collect();
    let refused = t.start("ann", true, T0).unwrap_err();
    assert_eq!((refused.status, refused.code), (503, Some("busy")));
    assert!(
        t.start("bob", true, T0).is_ok(),
        "the bound is the account's"
    );
    // A record that ended is not a driver that returned: the place is held until the driver releases it.
    t.finish(&ids[0], json!(1), at(1));
    assert!(t.start("ann", true, at(1)).is_err());
    assert_eq!(t.running("ann"), MAX_RUNNING);
    t.release(&ids[0], at(2));
    assert_eq!(t.running("ann"), MAX_RUNNING - 1);
    assert!(
        t.start("ann", true, at(2)).is_ok(),
        "a released job frees its place"
    );
}

// A stopped job keeps its place until its driver returns, so a page cannot out-run its drivers by going quiet.
#[test]
fn a_stopped_job_holds_its_place_until_its_driver_returns() {
    let t = Table::default();
    let ids: Vec<String> = (0..MAX_RUNNING)
        .map(|_| t.start("ann", true, T0).unwrap().0)
        .collect();
    t.tick(at(100));
    assert!(t.start("ann", true, at(100)).is_err());
    t.release(&ids[0], at(101));
    assert!(t.start("ann", true, at(101)).is_ok());
}

// A change set adopts only what a job finished with its value: neither a running job's file nor a stopped one's.
#[test]
fn only_a_job_that_ended_with_its_value_is_finished() {
    let t = Table::default();
    let (run, _) = t.start("ann", true, T0).unwrap();
    let (stopped, _) = t.start("ann", true, T0).unwrap();
    let (failed, _) = t.start("ann", true, T0).unwrap();
    let (done, _) = t.start("ann", false, T0).unwrap();
    t.fail(&failed, "no", None, at(1));
    t.finish(&done, json!(1), at(1));
    t.tick(at(100));
    t.finish(&run, json!(1), at(101));
    // `run` was stopped by the tick, so its late value changed nothing.
    for id in [&run, &stopped, &failed] {
        assert!(!t.finished("ann", id), "{id}");
    }
    assert!(t.finished("ann", &done));
    assert!(
        !t.finished("bob", &done),
        "another account's job is no one's"
    );
    assert!(!t.finished("ann", "nope"));
}

// A driver that returned with no answer leaves a failed record, never a running one.
#[test]
fn a_released_job_with_no_answer_is_failed() {
    let t = Table::default();
    let (id, _) = t.start("ann", true, T0).unwrap();
    t.release(&id, at(1));
    assert_eq!(t.poll("ann", &id, at(2)).unwrap()["state"], json!("failed"));
    assert!(!t.done(&id));
}
