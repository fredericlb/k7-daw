// curve_tests.rs — curvePreview.test.ts' cases, plus the wave, envelope and interpolation they lean on.

use crate::*;

fn sig() -> TimeSig {
    TimeSig::new(4, 4).unwrap()
}

fn curve(kind: Kind) -> Curve {
    Curve::new(Polarity::Unipolar, 1.0, Cut::None, kind, sig())
}

fn lfo(shape: Shape, frequency: f64, duration: Option<f64>) -> Curve {
    curve(Kind::Lfo {
        shape,
        frequency,
        phase: 0.0,
        duration,
    })
}

fn env(level: f64) -> Adsr {
    Adsr {
        attack: 10.0,
        decay: 10.0,
        sustain: 10.0,
        release: 10.0,
        level,
    }
}

fn p(beat: f64, value: f64) -> Point {
    Point { beat, value }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn spans_a_cycle_and_a_quarter_for_an_endless_oscillator() {
    assert_eq!(lfo(Shape::Saw, 1.0, None).span(), Span { beats: 5.0, solid: 4.0 });
}

#[test]
fn spans_the_whole_duration_when_it_has_one() {
    assert_eq!(lfo(Shape::Saw, 1.0, Some(8.0)).span(), Span { beats: 8.0, solid: 8.0 });
}

#[test]
fn draws_a_non_positive_frequency_as_a_frequency_of_one() {
    assert_eq!(lfo(Shape::Saw, 0.0, None).span(), Span { beats: 5.0, solid: 4.0 });
    assert_eq!(lfo(Shape::Saw, -3.0, None).span(), Span { beats: 5.0, solid: 4.0 });
}

#[test]
fn samples_across_the_span_wrapping_into_the_next_cycle() {
    let pts = preview(&lfo(Shape::Saw, 1.0, None), 10, None).solid;
    // The solid part stops at x 0.8: the first eight samples, then the edge.
    near(pts[4].y, 0.5);
    let all = preview(&lfo(Shape::Saw, 1.0, None), 10, None);
    assert_eq!(all.solid.len() + all.faded.len(), 11 + 1);
    near(all.faded[all.faded.len() - 1].y, 0.25);
}

#[test]
fn puts_a_saws_drop_in_the_solid_part_the_boundary_in_both() {
    let saw = Curve::new(
        Polarity::Unipolar,
        0.8,
        Cut::Positive,
        Kind::Lfo {
            shape: Shape::Saw,
            frequency: 0.5,
            phase: 0.0,
            duration: None,
        },
        sig(),
    );
    let CurvePreview { solid, faded, .. } = preview(&saw, 48, None);
    let end = *solid.last().unwrap();
    near(end.x, 0.8);
    near(end.y, 0.0);
    assert!(solid[solid.len() - 2].y > 0.7);
    assert_eq!(faded[0], end);
}

#[test]
fn draws_a_line_per_beat_a_bar_on_every_fourth_none_for_an_envelope() {
    let lines = preview(&lfo(Shape::Sine, 1.0, None), 160, None).beats;
    let bars: Vec<bool> = lines.iter().map(|l| l.bar).collect();
    assert_eq!(bars, [true, false, false, false, true, false]);
    near(lines[5].x, 1.0);
    assert!(preview(&curve(Kind::Adsr(env(1.0))), 160, None).beats.is_empty());
}

#[test]
fn folds_the_song_into_an_endless_oscillators_cycle() {
    let at = playhead(&lfo(Shape::Saw, 1.0, None), 10.0).unwrap();
    near(at.x, 0.4);
    near(at.y, 0.5);
}

#[test]
fn holds_at_the_end_of_a_finite_curve_and_has_no_playhead_for_an_envelope() {
    let at = playhead(&lfo(Shape::Saw, 1.0, Some(4.0)), 30.0).unwrap();
    assert_eq!(at.x, 1.0);
    assert!(playhead(&curve(Kind::Adsr(env(1.0))), 3.0).is_none());
    assert!(playhead(&lfo(Shape::Saw, 1.0, None), -1.0).is_none());
}

#[test]
fn answers_the_playhead_inside_the_preview_when_asked() {
    let c = lfo(Shape::Saw, 1.0, None);
    assert_eq!(preview(&c, 160, None).playhead, None);
    assert_eq!(preview(&c, 160, Some(10.0)).playhead, playhead(&c, 10.0));
}

#[test]
fn waves_hit_their_known_values() {
    let at = |s, beat| lfo(s, 1.0, None).sample(beat);
    near(at(Shape::Sine, 1.0), 1.0);
    near(at(Shape::Triangle, 1.0), 0.5);
    near(at(Shape::Triangle, 2.0), 1.0);
    near(at(Shape::Square, 1.0), 1.0);
    near(at(Shape::Square, 3.0), 0.0);
}

#[test]
fn adsr_runs_attack_decay_hold_release_then_zero() {
    let e = env(0.5);
    near(e.unit(5.0), 0.5);
    near(e.unit(15.0), 0.75);
    near(e.unit(25.0), 0.5);
    near(e.unit(35.0), 0.25);
    near(e.unit(45.0), 0.0);
    near(e.unit(-1.0), 0.0);
    near(
        Adsr {
            attack: 1.0,
            decay: 2.0,
            sustain: 3.0,
            release: 4.0,
            level: 1.0,
        }
        .ms(),
        10.0,
    );
}

#[test]
fn custom_points_interpolate_flat_outside_and_coincident_take_the_later() {
    let c = curve(Kind::Custom(Points::sorted(vec![
        p(4.0, 1.0),
        p(0.0, 0.0),
        p(4.0, 0.5),
    ])));
    near(c.sample(-1.0), 0.0);
    near(c.sample(2.0), 0.5);
    near(c.sample(3.0), 0.75);
    near(c.sample(4.0), 0.5);
    near(c.sample(9.0), 0.5);
    assert_eq!(
        c.span(),
        Span {
            beats: 16.0,
            solid: 16.0
        }
    );
}

#[test]
fn sorting_keeps_coincident_points_in_order_and_treats_negative_zero_as_zero() {
    let pts = Points::sorted(vec![p(1.0, 1.0), p(-0.0, 2.0), p(0.0, 3.0), p(1.0, 4.0)]);
    let values: Vec<f64> = pts.as_slice().iter().map(|q| q.value).collect();
    assert_eq!(values, [2.0, 3.0, 1.0, 4.0]);
}

#[test]
fn onoff_holds_each_value_until_the_next() {
    let c = curve(Kind::OnOff(Points::sorted(vec![p(1.0, 1.0), p(3.0, 0.0)])));
    near(c.sample(0.0), 1.0);
    near(c.sample(2.0), 1.0);
    near(c.sample(3.0), 0.0);
}

#[test]
fn a_lookup_in_no_points_is_zero() {
    near(curve(Kind::Custom(Points::sorted(Vec::new()))).sample(1.0), 0.0);
    near(curve(Kind::OnOff(Points::sorted(Vec::new()))).sample(1.0), 0.0);
}

#[test]
fn polarity_amplitude_and_cut_shape_the_output() {
    let mut c = lfo(Shape::Saw, 1.0, None);
    c.polarity = Polarity::Bipolar;
    c.amplitude = 0.5;
    near(c.sample(0.0), -0.5);
    c.cut = Cut::Positive;
    near(c.sample(0.0), 0.0);
    near(cut_output(-2.0, Cut::Negative), -2.0);
    near(cut_output(2.0, Cut::Negative), 0.0);
}

#[test]
fn an_envelope_previews_over_its_own_length() {
    let pts = preview(&curve(Kind::Adsr(env(0.5))), 110, None);
    assert_eq!(pts.span, Span { beats: 0.0, solid: 0.0 });
    assert!(pts.faded.is_empty());
    assert_eq!(pts.solid.len(), 111);
    near(pts.solid[0].y, 0.0);
}

#[test]
fn thins_the_beat_lines_of_a_huge_span_to_whole_bars() {
    let tiny = Kind::Lfo {
        shape: Shape::Saw,
        frequency: 1e-9,
        phase: 0.0,
        duration: None,
    };
    let long = Kind::Lfo {
        shape: Shape::Saw,
        frequency: 1.0,
        phase: 0.0,
        duration: Some(4.0e8),
    };
    for kind in [tiny, long] {
        let lines = preview(&curve(kind), 1, None).beats;
        assert!(lines.len() <= 4 * 160 + 1, "{}", lines.len());
        assert!(lines.len() > 100);
        assert!(lines.iter().all(|l| l.bar));
        near(lines[0].x, 0.0);
        assert!(lines[lines.len() - 1].x <= 1.0 + 1e-9);
    }
}

#[test]
fn keeps_every_beat_line_while_they_are_few() {
    assert_eq!(preview(&lfo(Shape::Saw, 1.0, Some(400.0)), 160, None).beats.len(), 401);
}
