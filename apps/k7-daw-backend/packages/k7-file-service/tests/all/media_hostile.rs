//! Hostile input to the slicing and the routes' clamps (2421, 2423, 2427): each answered, none hangs, none panics.

use std::time::{Duration, Instant};

use k7_file_service::media::region::{Envelope, beat_grid, region_waveform};
use k7_file_service::media::warp::{WarpMap, beat_at_source, source_at_beat, take_probes};
use k7_file_service::media::{clamp_buckets, clamp_sensitivity};
use song::clips::ClipDoc;

use crate::media_region::{clip, warped};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn run(
    c: &ClipDoc,
    frames: f64,
    seconds: f64,
    buckets: usize,
) -> k7_file_service::media::region::Region {
    let peak = [0.25; 40];
    let e = Envelope {
        sample_rate: 48000.0,
        frames,
        seconds,
        peak: &peak,
        rms: &peak,
        bands: None,
        transients: &[1.0, 5.0],
    };
    region_waveform(&e, c, 120.0, 4.0, 4.0, buckets)
}

#[test]
fn a_file_of_two_to_the_31_frames_or_more_reads_as_empty_as_the_typescript_does() {
    for frames in [2f64.powi(31), 3e9, 1e12] {
        let r = run(&clip(), frames, 12.0, 16);
        assert!(
            r.peak.iter().all(|&p| p == 0.0),
            "{frames}: ToInt32 makes it 0 frames"
        );
    }
    // 5e9 wraps to 705,032,704, a file that is not empty.
    assert!(run(&clip(), 5e9, 12.0, 16).peak.iter().any(|&p| p > 0.0));
}

#[test]
fn a_clip_of_non_finite_numbers_is_drawn_not_panicked_on() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in 0..4 {
            let mut c = clip();
            match field {
                0 => c.offset = v,
                1 => c.source_from = Some(v),
                2 => c.source_to = Some(v),
                _ => c.length = if v.is_infinite() { 0.0 } else { v },
            }
            let r = run(&c, 480_000.0, 10.0, 16);
            assert!(!r.peak.is_empty(), "field {field} = {v}");
            let _ = beat_grid(&r);
        }
    }
    let nan_marker = warped(vec![(0.0, 0.0), (f64::NAN, 1.0), (8.0, 4.0)]);
    assert!(!run(&nan_marker, 480_000.0, 10.0, 16).peak.is_empty());
}

#[test]
fn a_length_of_a_billion_beats_is_a_grid_too_long_to_draw() {
    let c = ClipDoc {
        length: 1e9,
        ..clip()
    };
    let r = run(&c, 480_000.0, 10.0, 16);
    assert!(beat_grid(&r).is_err());
    let inf = ClipDoc {
        length: f64::INFINITY,
        ..clip()
    };
    assert!(beat_grid(&run(&inf, 480_000.0, 10.0, 16)).is_err());
    let start = ClipDoc {
        start: f64::INFINITY,
        ..clip()
    };
    assert!(beat_grid(&run(&start, 480_000.0, 10.0, 16)).is_err());
}

fn ramp(n: usize) -> Vec<(f64, f64)> {
    (0..n).map(|i| (i as f64 * 0.25, i as f64 * 0.1)).collect()
}

// Markers spread over the whole 10 s file, so a walk reads a share of them per column.
fn spread(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| (i as f64 * 8.0 / n as f64, i as f64 * 10.0 / n as f64))
        .collect()
}

// The markers read by one draw at `buckets` columns; the marks and the onsets read a share per marker whatever the columns.
fn probes_of(n: usize, buckets: usize) -> usize {
    let c = warped(spread(n));
    take_probes();
    let r = run(&c, 480_000.0, 10.0, buckets);
    assert!(!r.peak.is_empty());
    take_probes()
}

#[test]
fn the_slicing_reads_a_logarithmic_number_of_markers_per_column() {
    // A thousand columns against sixteen leaves the columns' own reads; 4n markers add two per lookup to a bisect.
    let per = |n: usize| probes_of(n, 1000) - probes_of(n, 16);
    let (small, big) = (per(2_000), per(8_000));
    assert!(small > 0, "the lookups were counted");
    assert!(
        big < 2 * small,
        "{big} reads at 8,000 markers against {small} at 2,000"
    );
}

#[test]
fn a_lookup_on_ordered_markers_reads_a_logarithmic_number_of_them() {
    let per = |n: usize| {
        let w = WarpMap::new(&spread(n));
        take_probes();
        for k in 0..500 {
            let x = k as f64 * 0.02;
            w.beat_at_source(x);
            w.source_at_beat(x * 0.8);
        }
        take_probes()
    };
    let (small, big) = (per(2_000), per(8_000));
    assert!(small > 0, "the lookups were counted");
    assert!(
        big < 2 * small,
        "{big} reads at 8,000 markers against {small} at 2,000"
    );
}

#[test]
fn ten_thousand_and_forty_thousand_markers_slice_four_thousand_buckets_in_a_blink() {
    for n in [10_000usize, 40_000] {
        let m = ramp(n);
        let c = warped(m.clone());
        let t = Instant::now();
        let r = run(&c, 480_000.0, 10.0, 4000);
        assert!(t.elapsed() < BOUND, "{n} markers took {:?}", t.elapsed());
        assert_eq!(r.peak.len(), 4000.min(r.peak.len()));
        // The map is ordered on both axes, so both lookups are the bisect, and it agrees with the walk.
        let w = WarpMap::new(&m);
        assert!(w.bisects_sources() && w.bisects_beats());
        for k in 0..200 {
            let x = k as f64 * n as f64 * 0.1 / 200.0;
            assert_eq!(w.beat_at_source(x), beat_at_source(&m, x));
            assert_eq!(w.source_at_beat(x), source_at_beat(&m, x));
        }
    }
}

#[test]
fn markers_that_are_not_monotonic_are_walked_and_answered() {
    let mut m = ramp(5_000);
    m.swap(100, 4_000);
    let w = WarpMap::new(&m);
    assert!(!w.bisects_sources() || !w.bisects_beats());
    for x in [0.0, 1.0, 250.0, 400.5, 499.0, f64::NAN] {
        let (a, b) = (beat_at_source(&m, x), w.beat_at_source(x));
        assert!(a == b || (a.is_nan() && b.is_nan()));
    }
    assert!(!run(&warped(m), 480_000.0, 10.0, 4000).peak.is_empty());
}

#[test]
fn the_routes_clamps_answer_for_hostile_numbers() {
    assert_eq!(clamp_buckets(Some("-1")), 16);
    assert_eq!(clamp_buckets(Some("1e9")), 4000);
    assert_eq!(clamp_buckets(Some("abc")), 1000);
    assert_eq!(clamp_buckets(Some("Infinity")), 4000);
    assert_eq!(clamp_buckets(Some("-Infinity")), 16);
    assert_eq!(clamp_buckets(None), 1000);
    assert_eq!(clamp_sensitivity(Some("NaN")), None);
    assert_eq!(clamp_sensitivity(Some("Infinity")), None);
    assert_eq!(clamp_sensitivity(Some("1e9")), Some(8.0));
    assert_eq!(clamp_sensitivity(None), None);
}
