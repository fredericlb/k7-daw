//! plugin_units.rs — the plugin verbs' parts alone, as k7-runtime's `ScanProgress_test.cpp` and
//! `PendingQueries_test.cpp` held them: the scan board and its rate limit, the pending queries, the probe slots
//! under real threads.

use k7_runtime_server::server::pending::PendingQueries;
use k7_runtime_server::server::plugin_state::{MAX_PROBES, PluginState};
use k7_runtime_server::server::scan_board::{Board, Status};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn the_board_says_a_scan_started_with_no_total_yet() {
    let b = Board::new(ms(250));
    let s = b.begin(Instant::now());
    assert!(s.scanning);
    assert_eq!((s.total, s.done), (0, 0));
}

#[test]
fn stat_only_steps_are_rate_limited() {
    let b = Board::new(ms(250));
    let t0 = Instant::now();
    b.begin(t0);
    assert!(b.step(1, 100, "", t0 + ms(10)).is_none());
    assert!(b.step(2, 100, "", t0 + ms(100)).is_none());
    let said = b.step(3, 100, "", t0 + ms(260)).unwrap();
    assert_eq!((said.done, said.total), (3, 100));
    assert!(b.step(4, 100, "", t0 + ms(300)).is_none());
}

#[test]
fn a_load_starting_is_said_at_once_and_once() {
    let b = Board::new(ms(250));
    let t0 = Instant::now();
    b.begin(t0);
    let said = b.step(5, 100, "Serum.vst3", t0 + ms(5)).unwrap();
    assert_eq!(said.current, "Serum.vst3");
    assert!(b.step(5, 100, "Serum.vst3", t0 + ms(10)).is_none());
    assert!(b.step(6, 100, "Diva.vst3", t0 + ms(20)).is_some());
}

#[test]
fn finishing_ends_the_scan_and_counts_it() {
    let b = Board::new(ms(250));
    let t0 = Instant::now();
    b.begin(t0);
    b.step(7, 9, "x.vst3", t0);
    let end = b.finish();
    assert!(!end.scanning);
    assert_eq!((end.done, end.current.as_str()), (9, ""));
    assert_eq!(end.version, 1);
    b.begin(t0);
    assert_eq!(b.finish().version, 2);
    assert_eq!(
        b.snapshot(),
        Status {
            done: 0,
            total: 0,
            version: 2,
            ..Status::default()
        }
    );
}

#[test]
fn a_waiting_query_is_joined() {
    let p = PendingQueries::default();
    let first = p.admit("list-audio-devices");
    assert!(first.is_some());
    assert!(p.admit("list-audio-devices").is_none());
    assert!(p.admit("list-audio-devices").is_none());
    let other = p.admit("get-plugin-params\ns\nfx");
    assert!(other.is_some());
    assert_eq!(p.size(), 2);
}

#[test]
fn a_query_asked_while_one_runs_is_queued_again() {
    let p = PendingQueries::default();
    p.admit("list-sessions").unwrap().started();
    p.admit("list-sessions").unwrap().started();
    assert_eq!(p.size(), 0);
    let waiting = p.admit("list-sessions").unwrap();
    waiting.started();
    // A newer one queued while the first runs is not erased by the first finishing.
    let newer = p.admit("list-sessions").unwrap();
    assert_eq!(p.size(), 1);
    drop(newer);
    assert_eq!(p.size(), 0);
}

#[test]
fn a_ticket_whose_job_never_ran_frees_its_key() {
    let p = PendingQueries::default();
    drop(p.admit("list-presets\ns\nm"));
    assert!(p.admit("list-presets\ns\nm").is_some());
}

#[test]
fn identical_queries_from_many_threads_admit_exactly_one() {
    let p = Arc::new(PendingQueries::default());
    let admitted = Arc::new(AtomicUsize::new(0));
    let tickets = Arc::new(std::sync::Mutex::new(Vec::new()));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let (p, admitted, tickets) = (p.clone(), admitted.clone(), tickets.clone());
            std::thread::spawn(move || {
                for _ in 0..200 {
                    if let Some(t) = p.admit("get-plugin-params\ns\nfx") {
                        admitted.fetch_add(1, Ordering::SeqCst);
                        tickets.lock().unwrap().push(t);
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(admitted.load(Ordering::SeqCst), 1);
}

#[test]
fn at_most_four_probes_hold_a_slot_under_contention() {
    let state = Arc::new(PluginState::default());
    let held = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let refused = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..32)
        .map(|_| {
            let (state, held, peak, refused) =
                (state.clone(), held.clone(), peak.clone(), refused.clone());
            std::thread::spawn(move || {
                for _ in 0..200 {
                    match state.take_probe() {
                        Some(slot) => {
                            let now = held.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            std::thread::yield_now();
                            held.fetch_sub(1, Ordering::SeqCst);
                            drop(slot);
                        }
                        None => {
                            refused.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert!(peak.load(Ordering::SeqCst) <= MAX_PROBES);
    assert!(refused.load(Ordering::SeqCst) > 0);
    assert_eq!(state.probes_in_use(), 0);
}
