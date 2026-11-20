//! The stop's plan (2670 to 2672) on a turned clock: order, a hung member, the bound of the whole, and the line it leaves.

use std::time::{Duration, Instant};

use k7_file_service::runtime::stop::{Step, Stop, leaving_line};

const MEMBERS: [&str; 4] = ["jobs", "media", "captures", "service"];

fn s(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn members_run_in_order_one_after_the_other() {
    let o = Instant::now();
    let mut p = Stop::new(&MEMBERS, None, Some(s(5)), o);
    for m in MEMBERS {
        assert_eq!(p.step(o), Step::Run(m.into()));
        assert!(matches!(p.step(o), Step::Wait(_)), "{m} is waited for");
        assert!(p.finished(m));
    }
    assert_eq!(p.step(o), Step::Done);
    assert!(p.left().is_empty() && leaving_line(&[("me", &p)]).is_none());
}

#[test]
fn a_member_that_hangs_holds_the_stop_to_the_bound_and_is_named() {
    let o = Instant::now();
    let mut p = Stop::new(&MEMBERS, None, Some(s(5)), o);
    assert_eq!(p.step(o), Step::Run("jobs".into()));
    p.finished("jobs");
    assert_eq!(p.step(o), Step::Run("media".into()));
    assert_eq!(p.step(o + s(4)), Step::Wait(o + s(5)));
    assert_eq!(p.step(o + s(5)), Step::Done);
    assert_eq!(p.left(), ["media", "captures", "service"]);
    let line = leaving_line(&[("ok", &Stop::new(&[], None, None, o)), ("me@x.org", &p)]).unwrap();
    assert_eq!(
        line,
        "stop: left with unstopped members: me@x.org (media, captures, service)"
    );
}

#[test]
fn an_own_bound_moves_on_and_a_reap_has_no_bound_in_all() {
    let o = Instant::now();
    let mut p = Stop::new(&MEMBERS, Some(s(1)), None, o);
    p.step(o);
    assert_eq!(p.step(o + s(1)), Step::Run("media".into()));
    assert_eq!(p.left(), ["jobs", "media", "captures", "service"]);
    assert!(!p.expired(o + s(3600)));
}

#[test]
fn a_member_finished_out_of_turn_is_refused() {
    let o = Instant::now();
    let mut p = Stop::new(&MEMBERS, None, None, o);
    assert!(!p.finished("service"));
    p.step(o);
    assert!(!p.finished("media") && p.finished("jobs") && !p.finished("jobs"));
}
