// curves_bake.rs — a song-time curve sampled and baked: the grid, the verbatim points, polarity and amplitude; from curves.test.ts.
// A reader would get wrong that sampling is the pilot's (Curve::sample): these tests hold that the bake reads it and adds nothing.

use crate::curves_support;

use curves_support::{bake44, pairs, r44};

fn sample(text: &str, beat: f64) -> f64 {
    r44(text).curve().sample(beat)
}

const SINE: &str = "type(\"lfo\")\n";

// curves.test.ts "unipolar sine sits in 0..1, centred at 0.5 at the origin".
#[test]
fn a_unipolar_sine_is_centred_at_half_at_the_origin() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(close(sample(SINE, 0.0), 0.5));
    assert!(close(sample(SINE, 1.0), 1.0));
    assert!(close(sample(SINE, 2.0), 0.5));
    assert!(close(sample(SINE, 3.0), 0.0));
}

// curves.test.ts "bipolar swings -1..1".
#[test]
fn a_bipolar_sine_swings_from_minus_one_to_one() {
    let bip = "type(\"lfo\")\npolarity(\"bipolar\")\n";
    assert!(sample(bip, 0.0).abs() < 1e-9);
    assert!((sample(bip, 1.0) - 1.0).abs() < 1e-9);
    assert!((sample(bip, 3.0) + 1.0).abs() < 1e-9);
}

// curves.test.ts "frequency is per BAR: 0.5 cycle/bar spans two bars" and "phase offsets the start".
#[test]
fn frequency_is_per_bar_and_phase_is_a_head_start() {
    let half = "type(\"lfo\")\nfrequency(0.5)\n";
    assert!((sample(half, 2.0) - 1.0).abs() < 1e-9);
    assert!((sample(half, 4.0) - 0.5).abs() < 1e-9);
    assert!((sample("type(\"lfo\")\nphase(0.25)\n", 0.0) - 1.0).abs() < 1e-9);
}

// curves.test.ts "triangle", "saw" and "square": the other shapes.
#[test]
fn the_triangle_the_saw_and_the_square_are_their_waves() {
    let tri = "type(\"lfo\")\nshape(\"triangle\")\n";
    assert_eq!([0.0, 2.0, 4.0, 1.0].map(|b| sample(tri, b)), [0.0, 1.0, 0.0, 0.5]);
    let saw = "type(\"lfo\")\nshape(\"saw\")\n";
    assert_eq!([0.0, 2.0, 3.0].map(|b| sample(saw, b)), [0.0, 0.5, 0.75]);
    let sq = "type(\"lfo\")\nshape(\"square\")\n";
    assert_eq!([0.0, 1.0, 2.0, 3.0].map(|b| sample(sq, b)), [1.0, 1.0, 0.0, 0.0]);
    let sqb = "type(\"lfo\")\nshape(\"square\")\npolarity(\"bipolar\")\n";
    assert_eq!([sample(sqb, 0.0), sample(sqb, 2.0)], [1.0, -1.0]);
}

const RAMP: &str = "type(\"custom\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1)])\n";

// curves.test.ts "interpolates linearly between points" and "holds flat before the first and after the last point".
#[test]
fn a_custom_curve_is_linear_between_points_and_flat_outside() {
    assert!((sample(RAMP, 2.0) - 0.5).abs() < 1e-9);
    assert!((sample(RAMP, 4.0) - 1.0).abs() < 1e-9);
    assert!((sample(RAMP, -10.0)).abs() < 1e-9);
    assert!((sample(RAMP, 100.0) - 1.0).abs() < 1e-9);
}

// curves.test.ts "bipolar remaps the authored 0..1 onto -1..1" and "sorts out-of-order points before reading".
#[test]
fn a_bipolar_custom_curve_remaps_and_unsorted_points_are_sorted() {
    let bip = "type(\"custom\")\npolarity(\"bipolar\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1)])\n";
    assert_eq!([0.0, 2.0, 4.0].map(|b| sample(bip, b)), [-1.0, 0.0, 1.0]);
    let unsorted = "type(\"custom\")\npoints([time(\"2:1:1\").value(1), time(\"1:1:1\").value(0)])\n";
    assert!((sample(unsorted, 2.0) - 0.5).abs() < 1e-9);
}

// curves.test.ts "emits a custom curve's authored points verbatim (beat, output)".
#[test]
fn a_custom_curve_bakes_its_authored_points_verbatim() {
    let b = bake44(
        "type(\"custom\")\npoints([time(\"1:1:1\").value(0), time(\"3:1:1\").value(1)])\n",
        100.0,
        &[],
    );
    assert!(!b.bipolar);
    assert_eq!(pairs(&b), [(0.0, 0.0), (8.0, 1.0)]);
}

// curves.test.ts "carries polarity so the engine can map the output range".
#[test]
fn a_bake_carries_its_polarity() {
    assert!(bake44("type(\"lfo\")\npolarity(\"bipolar\")\n", 8.0, &[]).bipolar);
    assert!(!bake44(SINE, 8.0, &[]).bipolar);
}

// curves.test.ts "samples an LFO on a grid over the song, output within the polarity range"; 24 breakpoints a bar.
#[test]
fn an_lfo_bakes_on_a_grid_over_the_song_within_its_range() {
    let b = bake44("type(\"lfo\")\nshape(\"triangle\")\n", 16.0, &[]);
    assert!(b.points.len() > 16);
    assert_eq!(b.points[0], [0.0, 0.0]);
    assert!(b.points.iter().all(|p| (0.0..=1.0).contains(&p[1])));
    assert!(b.points[b.points.len() - 1][0] >= 16.0);
    // Four bars at 24 a bar, both ends: 97 points (the TypeScript's accumulation, kept).
    assert_eq!(b.points.len(), 97);
}

// decision 218's grid rule: a short song is widened to four bars, a long one is capped at 2048 steps.
#[test]
fn the_grid_spans_four_bars_at_least_and_is_capped_for_a_long_song() {
    assert_eq!(bake44(SINE, 0.0, &[]).points.len(), 97);
    let long = bake44(SINE, 1e300, &[]);
    assert!(
        long.points.len() <= 2050 && long.points.len() >= 2048,
        "{}",
        long.points.len()
    );
}

// curves.test.ts "scales a unipolar curve's top" and "scales a bipolar curve around 0".
#[test]
fn amplitude_scales_the_top_and_the_swing() {
    let sq = |p: &str| format!("type(\"lfo\")\nshape(\"square\")\npolarity(\"{p}\")\namplitude(0.5)\n");
    assert_eq!([sample(&sq("unipolar"), 0.0), sample(&sq("unipolar"), 2.0)], [0.5, 0.0]);
    assert_eq!([sample(&sq("bipolar"), 0.0), sample(&sq("bipolar"), 2.0)], [0.5, -0.5]);
}

// decision 209 order: a custom curve with equal beats keeps the file's order, both values written.
#[test]
fn coincident_points_keep_the_files_order() {
    let b = bake44(
        "type(\"custom\")\npoints([time(\"2:1:1\").value(1), time(\"2:1:1\").value(0.25), time(\"1:1:1\").value(0)])\n",
        8.0,
        &[],
    );
    assert_eq!(pairs(&b), [(0.0, 0.0), (4.0, 0.25), (4.0, 0.25)]);
}
