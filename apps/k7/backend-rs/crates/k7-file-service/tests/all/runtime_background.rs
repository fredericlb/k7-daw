//! The registry (2630 to 2632, 2614) on a turned clock: one job per tag, the lease, a second end, a drop under three jobs.

use std::time::{Duration, Instant};

use k7_file_service::runtime::LEASE;
use k7_file_service::runtime::background::Jobs;
use k7_file_service::runtime::list::State;

fn at(o: Instant, ms: u64) -> Instant {
    o + Duration::from_millis(ms)
}

#[test]
fn ten_thousand_starts_of_one_tag_are_one_job() {
    let o = Instant::now();
    let mut j = Jobs::new(o, 0);
    let started = (0..10_000)
        .filter(|_| j.start("render /a/b.wav", o))
        .count();
    assert_eq!((started, j.running(), j.list(o).len()), (1, 1, 1));
}

#[test]
fn a_job_ended_twice_says_one_line_and_a_silent_job_stops_holding_at_the_lease() {
    let o = Instant::now();
    let mut j = Jobs::new(o, 0);
    j.start("t", o);
    assert!(j.holds(at(o, LEASE.as_millis() as u64 - 1)));
    assert!(!j.holds(at(o, LEASE.as_millis() as u64)));
    assert_eq!(j.lapsed(at(o, LEASE.as_millis() as u64)), ["t"]);
    assert!(
        j.lapsed(at(o, LEASE.as_millis() as u64 + 1)).is_empty(),
        "said once"
    );
    j.heard(at(o, LEASE.as_millis() as u64 + 5));
    assert!(j.holds(at(o, LEASE.as_millis() as u64 + 6)));
    assert_eq!(
        j.end("t", Ok("done".into()), at(o, 10)).as_deref(),
        Some("   ✓ done")
    );
    assert_eq!(j.end("t", Ok("again".into()), at(o, 11)), None);
    assert!(!j.holds(at(o, 12)));
}

#[test]
fn the_clock_alone_does_not_renew_the_lease() {
    let o = Instant::now();
    let mut j = Jobs::new(o, 0);
    j.start("t", o);
    for m in 1..=11 {
        assert_eq!(j.holds(at(o, m * 60_000)), m < 10, "minute {m}");
    }
}

#[test]
fn a_failure_names_its_tag_and_a_late_progress_is_dropped() {
    let o = Instant::now();
    let mut j = Jobs::new(o, 1000);
    j.start("render /a/b.wav", o);
    j.progress("render /a/b.wav", Some(0.5), Some("half"), at(o, 1));
    assert_eq!(j.list(at(o, 1))[0].progress, Some(0.5));
    let line = j
        .end("render /a/b.wav", Err("boom".into()), at(o, 2))
        .unwrap();
    assert_eq!(line, "   ✗ render /a/b.wav failed: boom");
    j.progress("render /a/b.wav", Some(0.9), Some("late"), at(o, 3));
    let e = &j.list(at(o, 3))[0];
    assert_eq!(
        (e.state, e.progress, e.detail.as_deref(), e.started_at),
        (State::Failed, None, Some("boom"), 1000)
    );
    assert!(j.list(at(o, 8002)).len() == 1 && j.list(at(o, 8003)).is_empty());
}

#[test]
fn a_runtime_dropped_under_three_jobs_ends_each_failed_once() {
    let o = Instant::now();
    let mut j = Jobs::new(o, 0);
    for t in ["a /x/1", "b /x/2", "c /x/3"] {
        j.start(t, o);
    }
    let lines = j.drop_all(at(o, 5));
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "   ✗ a /x/1 failed: the file service restarted");
    assert!(j.drop_all(at(o, 6)).is_empty() && !j.holds(at(o, 6)));
    assert!(j.list(at(o, 6)).iter().all(|e| e.state == State::Failed));
}
