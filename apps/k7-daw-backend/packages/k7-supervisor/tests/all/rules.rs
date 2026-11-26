//! rules.rs — the pure rules: the crash loop reaches safe mode at its third quick crash and backs off meanwhile,
//! a long run starts the count again, a failed start is no crash; the candidate ports and a held port.

use std::time::{Duration, Instant};

use k7_supervisor::crash_loop::{self, CrashLoop, QUICK};
use k7_supervisor::ports;

const S: fn(u64) -> Duration = Duration::from_secs;

#[test]
fn quick_crashes_back_off_and_reach_safe_mode() {
    let mut c = CrashLoop::default();
    let mut t = Instant::now();
    c.started(t);
    c.died(t + S(2));
    assert_eq!(
        (c.quick_crashes(), c.safe_mode(), c.due(t + S(2))),
        (1, false, true)
    );
    t += S(2);
    c.started(t);
    c.died(t + S(2));
    assert_eq!((c.quick_crashes(), c.safe_mode()), (2, false));
    assert!(!c.due(t + S(3)) && c.due(t + S(4)));
    t += S(4);
    c.started(t);
    c.died(t + S(2));
    assert_eq!((c.quick_crashes(), c.safe_mode()), (3, true));
    assert!(!c.due(t + S(5)) && c.due(t + S(6)));
}

#[test]
fn the_backoff_is_capped_and_a_long_run_counts_again() {
    let mut c = CrashLoop::default();
    let mut t = Instant::now();
    for _ in 0..10 {
        c.started(t);
        c.died(t + S(1));
        t += S(1);
    }
    assert!(!c.due(t + S(29)) && c.due(t + S(30)));
    c.started(t);
    c.died(t + QUICK);
    assert_eq!(c.quick_crashes(), 1);
    assert!(c.safe_mode(), "safe mode outlives a long run");
    c.reset();
    assert!(!c.safe_mode() && !c.up() && c.quick_crashes() == 0);
}

#[test]
fn a_failed_start_is_no_crash() {
    let mut c = CrashLoop::default();
    let t = Instant::now();
    for _ in 0..5 {
        c.start_failed(t);
    }
    assert_eq!((c.quick_crashes(), c.safe_mode()), (0, false));
    assert!(!c.due(t + S(15)) && c.due(t + S(16)));
    c.died(t);
    assert_eq!(c.quick_crashes(), 0);
    assert!(
        !crash_loop::wedged(crash_loop::WEDGED_MS - 1.0)
            && crash_loop::wedged(crash_loop::WEDGED_MS)
    );
}

#[test]
fn the_candidate_ports_follow_the_first() {
    let p = ports::candidates(8766, false);
    assert_eq!((p.len(), p[0], p[9]), (ports::PORT_SPAN, 8766, 8775));
    assert_eq!(ports::candidates(8766, true), [8766]);
    assert_eq!(ports::candidates(65534, false), [65534, 65535]);
    assert!(ports::candidates(0, false).is_empty());
}

#[test]
fn a_loopback_only_listener_holds_its_port_and_answers() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    assert!(!ports::bindable(port));
    assert_eq!(ports::probe(port), ports::Probe::Up);
    drop(l);
    assert_eq!(ports::probe(port), ports::Probe::Down);
    assert!(ports::bindable(port));
}
