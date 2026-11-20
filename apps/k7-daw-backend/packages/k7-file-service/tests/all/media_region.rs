//! The slicing's own rules (2420 to 2424) beyond the vectors: the warp lookups' two paths, the display mapping, and the
//! memo signature as a key.

use k7_file_service::media::answer::{Window, wf_sig};
use k7_file_service::media::region::{Envelope, beat_grid, region_waveform};
use k7_file_service::media::warp::{WarpMap, beat_at_source, source_at_beat};
use song::clips::{ClipDoc, ClipKind, WarpDoc};

pub fn clip() -> ClipDoc {
    ClipDoc {
        id: "c".into(),
        kind: ClipKind::Audio,
        name: "c".into(),
        source: "/ws/c.wav".into(),
        start: 0.0,
        length: 8.0,
        offset: 0.0,
        source_from: None,
        source_to: None,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: vec![],
        warp: None,
        warp_off: None,
        disabled: None,
        event: None,
        comp: None,
        crossfade: None,
        nest: None,
    }
}

pub fn warped(markers: Vec<(f64, f64)>) -> ClipDoc {
    ClipDoc {
        warp: Some(WarpDoc {
            algorithm: "elastique".into(),
            markers,
        }),
        ..clip()
    }
}

fn env(peak: &[f64]) -> Envelope<'_> {
    Envelope {
        sample_rate: 48000.0,
        frames: 480_000.0,
        seconds: 10.0,
        peak,
        rms: peak,
        bands: None,
        transients: &[1.0, 2.0],
    }
}

#[test]
fn a_map_is_bisected_only_on_an_ordered_axis() {
    let ordered = WarpMap::new(&[(0.0, 0.0), (2.0, 1.0), (4.0, 3.0)]);
    assert!(ordered.bisects_sources() && ordered.bisects_beats());
    let dip = WarpMap::new(&[(0.0, 0.0), (2.0, 3.0), (3.0, 2.0), (6.0, 6.0)]);
    assert!(!dip.bisects_sources() && dip.bisects_beats());
    let nan = WarpMap::new(&[(0.0, 0.0), (f64::NAN, 1.0), (8.0, 4.0)]);
    assert!(nan.bisects_sources() && !nan.bisects_beats());
    // Equal neighbours are ordered: duplicates are the case a bisect gets wrong by landing on the other of two.
    let dup = WarpMap::new(&[(0.0, 1.0), (2.0, 1.0), (4.0, 5.0)]);
    assert!(dup.bisects_sources());
}

#[test]
fn a_bisect_lands_on_the_segment_the_walk_does_even_on_duplicates_and_nan() {
    let maps: Vec<Vec<(f64, f64)>> = vec![
        vec![(0.0, 1.0), (2.0, 1.0), (4.0, 5.0), (6.0, 5.0), (8.0, 9.0)],
        vec![(0.0, 0.0), (0.0, 2.0), (4.0, 5.0), (4.0, 6.0), (8.0, 9.0)],
        vec![(4.0, 0.0), (2.0, 1.0), (0.0, 2.0)],
        vec![(0.0, 0.0), (2.0, 3.0), (3.0, 2.0), (6.0, 6.0)],
    ];
    for m in maps {
        let w = WarpMap::new(&m);
        let mut xs: Vec<f64> = m
            .iter()
            .flat_map(|&(b, s)| [b, s, b - 0.5, s + 0.5])
            .collect();
        xs.extend([f64::NAN, -1.0, 100.0]);
        for x in xs {
            let (a, b) = (beat_at_source(&m, x), w.beat_at_source(x));
            assert!(
                a == b || (a.is_nan() && b.is_nan()),
                "beatAtSource({x}) over {m:?}: {a} against {b}"
            );
            let (a, b) = (source_at_beat(&m, x), w.source_at_beat(x));
            assert!(
                a == b || (a.is_nan() && b.is_nan()),
                "sourceAtBeat({x}) over {m:?}: {a} against {b}"
            );
        }
    }
}

#[test]
fn an_unwarped_clip_maps_a_file_second_to_the_display_by_its_in_point() {
    let peak = [0.5; 10];
    let c = ClipDoc {
        source_from: Some(2.0),
        offset: 1.0,
        length: 0.0,
        ..clip()
    };
    let r = region_waveform(&env(&peak), &c, 120.0, 4.0, 4.0, 16);
    assert_eq!(r.source_map.in_point, 3.0);
    assert_eq!(r.to_display(5.0), 2.0);
    assert_eq!(r.seconds, 7.0);
}

#[test]
fn a_clip_longer_than_its_file_shows_trailing_silence() {
    let peak = [0.5; 10];
    let c = ClipDoc {
        length: 64.0,
        ..clip()
    };
    let r = region_waveform(&env(&peak), &c, 120.0, 4.0, 4.0, 64);
    assert_eq!(r.seconds, 32.0);
    assert_eq!(r.peak.len(), 64);
    assert!(r.peak[63] == 0.0 && r.peak[0] > 0.0);
}

#[test]
fn a_grid_labels_a_bar_and_only_a_bar() {
    let peak = [0.5; 10];
    let r = region_waveform(
        &env(&peak),
        &ClipDoc {
            length: 9.0,
            ..clip()
        },
        120.0,
        4.0,
        4.0,
        16,
    );
    let g = beat_grid(&r).unwrap();
    assert_eq!(g.len(), 10);
    assert_eq!(g.iter().filter(|l| l.bar).count(), 3);
    assert_eq!(g[0].label.as_deref(), Some("1"));
    assert_eq!(g[4].label.as_deref(), Some("2"));
    assert!(g[1].label.is_none() && !g[1].bar);
}

fn sig(den: f64, window: Option<Window>) -> String {
    wf_sig("fp", &clip(), (120.0, 7.0, den), 100, window)
}

#[test]
fn the_memo_signature_is_a_key_of_every_input_the_region_reads() {
    let w = Some(Window { from: 1.0, to: 3.0 });
    assert_eq!(sig(4.0, w), sig(4.0, w));
    // The denominator decides the bars (D3): 7/4 and 7/8 must not share a memo entry.
    assert_ne!(sig(4.0, None), sig(8.0, None));
    // So does the window a `whole` draw shades (D3), and its edges one by one.
    assert_ne!(sig(4.0, None), sig(4.0, w));
    assert_ne!(sig(4.0, w), sig(4.0, Some(Window { from: 2.0, to: 3.0 })));
    assert_ne!(sig(4.0, w), sig(4.0, Some(Window { from: 1.0, to: 4.0 })));
    let moved = ClipDoc {
        start: 1.0,
        ..clip()
    };
    assert_ne!(
        sig(4.0, None),
        wf_sig("fp", &moved, (120.0, 7.0, 4.0), 100, None)
    );
    assert_ne!(
        sig(4.0, None),
        wf_sig("fp", &clip(), (120.0, 7.0, 4.0), 101, None)
    );
    assert_ne!(
        sig(4.0, None),
        wf_sig("fq", &clip(), (120.0, 7.0, 4.0), 100, None)
    );
    assert_ne!(
        sig(4.0, None),
        wf_sig(
            "fp",
            &warped(vec![(0.0, 0.0), (4.0, 2.0)]),
            (120.0, 7.0, 4.0),
            100,
            None
        )
    );
}

#[test]
fn a_marker_moved_changes_the_signature_and_the_slicing() {
    let a = warped(vec![(0.0, 0.0), (8.0, 4.0)]);
    let b = warped(vec![(0.0, 0.0), (8.0, 5.0)]);
    assert_ne!(
        wf_sig("fp", &a, (120.0, 4.0, 4.0), 16, None),
        wf_sig("fp", &b, (120.0, 4.0, 4.0), 16, None)
    );
    let peak = [0.5; 10];
    let (ra, rb) = (
        region_waveform(&env(&peak), &a, 120.0, 4.0, 4.0, 16),
        region_waveform(&env(&peak), &b, 120.0, 4.0, 4.0, 16),
    );
    assert_ne!(ra.source_map.stretch, rb.source_map.stretch);
}
