// curves_adsr.rs — the envelope: its shape against the time since a trigger, and the five corners laid at each firing; from adsrCurve.test.ts.
// A reader would get wrong that the envelope is in MILLISECONDS: the tempo moves where its corners fall in beats.

use crate::curves_support;

use curves_support::{adsr, bake44, budget, fire, pairs, r44};
use song::curves::{EnvelopeFire, bake_adsr, bake_curve};
use song_schema::Kind;

/// 100ms attack, 100ms decay to 0.5, 200ms hold, 100ms release: half a second in all.
const ENV: &str = "type(\"adsr\")\nattack(100)\ndecay(100)\nsustain(200)\nrelease(100)\nlevel(0.5)\n";

// 120 BPM: a beat is 500ms, so the whole envelope above is exactly one beat.
const TEMPO: f64 = 120.0;

fn unit(text: &str, ms: f64) -> f64 {
    match r44(text).curve().kind() {
        Kind::Adsr(a) => a.unit(ms),
        _ => f64::NAN,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// adsrCurve.test.ts "rises, falls to the level, holds it, then leaves".
#[test]
fn the_shape_rises_falls_to_the_level_holds_and_leaves() {
    assert_eq!(unit(ENV, 0.0), 0.0);
    for (ms, want) in [
        (50.0, 0.5),
        (100.0, 1.0),
        (150.0, 0.75),
        (200.0, 0.5),
        (350.0, 0.5),
        (450.0, 0.25),
    ] {
        assert!(close(unit(ENV, ms), want), "{ms}");
    }
    assert_eq!(unit(ENV, 500.0), 0.0);
}

// adsrCurve.test.ts "is silent once it is over, and before it starts".
#[test]
fn the_shape_is_silent_before_and_after() {
    assert_eq!(unit(ENV, 5000.0), 0.0);
    assert_eq!(unit(ENV, -1.0), 0.0);
}

// adsrCurve.test.ts "survives a stage of zero, rather than dividing by it".
#[test]
fn a_stage_of_zero_is_not_divided_by() {
    let snap = "type(\"adsr\")\nattack(0)\ndecay(0)\nsustain(200)\nrelease(100)\n";
    assert!(unit(snap, 0.0).is_finite());
    assert!(close(unit(snap, 1.0), 0.5));
}

// adsrCurve.test.ts "puts the five corners at one trigger, in beats".
#[test]
fn five_corners_are_laid_at_one_trigger() {
    assert_eq!(
        pairs(&adsr(ENV, &[4.0], TEMPO)),
        [(4.0, 0.0), (4.2, 1.0), (4.4, 0.5), (4.8, 0.5), (5.0, 0.0)]
    );
}

// adsrCurve.test.ts "repeats it at every trigger".
#[test]
fn the_envelope_repeats_at_every_trigger() {
    let b = adsr(ENV, &[0.0, 2.0], TEMPO);
    assert_eq!(b.points[0], [0.0, 0.0]);
    assert_eq!(b.points.iter().find(|p| p[0] == 2.0).copied(), Some([2.0, 0.0]));
    assert_eq!(b.points.iter().filter(|p| p[1] == 1.0).count(), 2);
}

// adsrCurve.test.ts "holds a running envelope at the value it REACHED, mid-ramp".
#[test]
fn a_running_envelope_is_held_at_the_value_it_reached() {
    let b = adsr(ENV, &[0.0, 0.3], TEMPO);
    let before = b.points.iter().rfind(|p| p[0] < 0.3).expect("a point");
    assert!(close(before[1], 0.75) && (before[0] - 0.3).abs() < 1e-4);
}

// adsrCurve.test.ts "CUTS a running envelope where the next trigger lands".
#[test]
fn the_next_trigger_cuts_a_running_envelope() {
    let b = adsr(ENV, &[0.0, 0.5], TEMPO);
    let cut = b.points.iter().rfind(|p| p[0] < 0.5).expect("a point");
    assert!(close(cut[1], 0.5));
    assert_eq!(b.points.iter().find(|p| p[0] == 0.5).copied(), Some([0.5, 0.0]));
    let after: Vec<[f64; 2]> = b.points.iter().filter(|p| p[0] >= 0.5).copied().collect();
    assert_eq!(after, [[0.5, 0.0], [0.7, 1.0], [0.9, 0.5], [1.3, 0.5], [1.5, 0.0]]);
}

// adsrCurve.test.ts "takes the triggers in any order".
#[test]
fn the_triggers_come_in_any_order() {
    assert_eq!(adsr(ENV, &[2.0, 0.0], TEMPO), adsr(ENV, &[0.0, 2.0], TEMPO));
}

// adsrCurve.test.ts "answers an empty trigger list with an empty curve, not a flat line".
#[test]
fn no_trigger_is_an_empty_curve() {
    assert!(adsr(ENV, &[], TEMPO).points.is_empty());
}

// adsrCurve.test.ts "is measured in MILLISECONDS, so the tempo moves it".
#[test]
fn the_tempo_moves_where_the_corners_fall() {
    let last = |t: f64| adsr(ENV, &[0.0], t).points.last().expect("a point")[0];
    assert!(close(last(60.0), 0.5) && close(last(240.0), 2.0));
}

// adsrCurve.test.ts "maps a bipolar envelope onto -1..1".
#[test]
fn a_bipolar_envelope_bottoms_out_at_minus_one() {
    let b = adsr(&format!("{ENV}polarity(\"bipolar\")\n"), &[0.0], TEMPO);
    assert!(b.bipolar);
    assert_eq!(b.points[0], [0.0, -1.0]);
    assert_eq!(b.points[1], [0.2, 1.0]);
}

// adsrCurve.test.ts "scales every corner the bake writes".
#[test]
fn amplitude_scales_every_corner() {
    let b = adsr(&format!("{ENV}amplitude(0.5)\n"), &[0.0], 120.0);
    assert_eq!(b.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max), 0.5);
}

// decision 216: asked for song-time breakpoints an envelope is the flat answer, the bottom when bipolar.
#[test]
fn an_envelope_asked_for_a_song_time_bake_is_flat() {
    assert_eq!(pairs(&bake44(ENV, 16.0, &[])), [(0.0, 0.0)]);
    assert_eq!(
        pairs(&bake44(&format!("{ENV}polarity(\"bipolar\")\n"), 16.0, &[])),
        [(0.0, -1.0)]
    );
    let cut = format!("{ENV}polarity(\"bipolar\")\ncut(\"positive\")\n");
    assert_eq!(
        pairs(&bake_curve(&r44(&cut), 16.0, &[], &mut budget()).expect("bakes")),
        [(0.0, 0.0)]
    );
}

// decision 209: a firing that may not cut is ignored while one runs; one with a window is cut where it ends.
#[test]
fn a_firing_without_retrigger_waits_for_the_running_one_and_a_window_cuts() {
    let c = r44("type(\"adsr\")\nattack(1000)\n");
    let plain = |retrigger| EnvelopeFire {
        at: 1.0,
        stop: None,
        retrigger,
    };
    let once = bake_adsr(&c, &[fire(0.0), plain(false)], 120.0, &mut budget()).expect("bakes");
    let twice = bake_adsr(&c, &[fire(0.0), plain(true)], 120.0, &mut budget()).expect("bakes");
    assert!(once.points.len() < twice.points.len());
    let windowed = EnvelopeFire {
        at: 0.0,
        stop: Some(1.0),
        retrigger: true,
    };
    let cut = bake_adsr(&c, &[windowed], 120.0, &mut budget()).expect("bakes");
    assert!(cut.points.last().expect("a point")[0] < 1.0);
    let past = EnvelopeFire {
        at: 2.0,
        stop: Some(1.0),
        retrigger: true,
    };
    assert!(
        bake_adsr(&c, &[past], 120.0, &mut budget())
            .expect("bakes")
            .points
            .is_empty()
    );
}

// decision 209 / curveBake.ts: a corner on a coincident beat is nudged past it, and a repeated value is dropped.
#[test]
fn a_coincident_corner_is_nudged_and_a_repeated_value_dropped() {
    let snap = "type(\"adsr\")\nattack(0)\ndecay(0)\nsustain(500)\nrelease(0)\nlevel(0.5)\n";
    let b = adsr(snap, &[0.0], TEMPO);
    assert_eq!(
        pairs(&b),
        [
            (0.0, 0.0),
            (0.000001, 1.0),
            (0.000002, 0.5),
            (1.0, 0.5),
            (1.000001, 0.0)
        ]
    );
}
