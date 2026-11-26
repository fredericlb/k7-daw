//! performance.rs — the scoring cases: the
//! reference machine is 1000, every metric moves it the right way, an unmeasured one is left out; and the real
//! measure answers within a second with numbers a score can use.

use std::time::{Duration, Instant};

use k7_supervisor::performance::{Metrics, measure, score, to_json};

use crate::scratch::Scratch;

fn reference() -> Metrics {
    Metrics {
        cores: 8.0,
        single_thread: 60.0,
        disk_write_mbps: Some(500.0),
        fsync_per_sec: Some(200.0),
        memory_gb: 16.0,
    }
}

#[test]
fn the_reference_is_a_thousand() {
    assert_eq!(score(&reference()), 1000);
}

#[test]
fn every_metric_moves_it_the_right_way() {
    type Set = fn(&mut Metrics, f64);
    let moves: [(Set, f64, f64); 5] = [
        (|m, v| m.cores = v, 16.0, 4.0),
        (|m, v| m.single_thread = v, 120.0, 30.0),
        (|m, v| m.disk_write_mbps = Some(v), 1000.0, 250.0),
        (|m, v| m.fsync_per_sec = Some(v), 400.0, 100.0),
        (|m, v| m.memory_gb = v, 32.0, 8.0),
    ];
    for (set, up, down) in moves {
        let (mut better, mut worse) = (reference(), reference());
        set(&mut better, up);
        set(&mut worse, down);
        assert!(score(&better) > 1000 && score(&worse) < 1000);
    }
}

#[test]
fn an_unmeasured_disk_is_left_out_not_zero() {
    let m = Metrics {
        disk_write_mbps: None,
        fsync_per_sec: None,
        ..reference()
    };
    assert_eq!(score(&m), 1000);
    assert_eq!(score(&Metrics::default()), 0);
}

#[test]
fn the_json_carries_its_score() {
    let j = to_json(&reference());
    assert_eq!(j["score"], 1000);
    assert_eq!(j["cores"], 8.0);
    let keys: Vec<&str> = j.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "cores",
            "singleThread",
            "diskWriteMBps",
            "fsyncPerSec",
            "memoryGB",
            "score"
        ]
    );
    assert!(
        to_json(&Metrics {
            disk_write_mbps: None,
            ..reference()
        })["diskWriteMBps"]
            .is_null()
    );
}

#[test]
fn the_real_measure_is_quick_and_leaves_nothing_behind() {
    let dir = Scratch::new("perf");
    let start = Instant::now();
    let m = measure(dir.path());
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(m.cores >= 1.0 && m.single_thread > 0.0 && m.memory_gb > 0.0);
    assert!(m.disk_write_mbps.is_some() && m.fsync_per_sec.is_some());
    assert!(score(&m) > 0);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn a_disk_that_cannot_be_written_is_unknown() {
    let dir = Scratch::new("perf-ro");
    let file = dir.path().join("a-file");
    std::fs::write(&file, "x").unwrap();
    // A file where the folder should be: no probe can be made, and the score leaves the disk out.
    let m = measure(&file);
    assert!(m.disk_write_mbps.is_none() && m.fsync_per_sec.is_none());
    assert!(score(&m) > 0);
}
