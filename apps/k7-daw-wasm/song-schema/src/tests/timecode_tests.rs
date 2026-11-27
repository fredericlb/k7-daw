// timecode_tests.rs — the cases of the TypeScript oracle (k7-wasm's timecode.test.ts), plus the JS quirks the port reproduces on purpose.

use crate::*;

fn sig(num: u32, den: u32) -> TimeSig {
    TimeSig::new(num, den).unwrap()
}

fn pos(tc: &str) -> f64 {
    parse_position(tc, sig(4, 4)).unwrap()
}

fn near(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() < eps, "{a} != {b}");
}

#[test]
fn reads_a_one_based_position_in_four_four() {
    assert_eq!(pos("1:1:1:1"), 0.0);
    assert_eq!(pos("2:1:1:1"), 4.0);
    assert_eq!(pos("1:2:1:1"), 1.0);
    assert_eq!(pos("1:1:2:1"), 0.5);
    assert_eq!(pos("1:1:1:2"), 0.25);
    assert_eq!(pos("1:1:2:2"), 0.75);
    assert_eq!(pos("2:1"), pos("2:1:1:1"));
    assert_eq!(pos("1:1:3"), 1.0);
}

#[test]
fn scales_the_beat_by_the_denominator() {
    assert_eq!(parse_position("2:1:1", sig(6, 8)).unwrap(), 3.0);
    assert_eq!(parse_position("1:2:1", sig(6, 8)).unwrap(), 0.5);
}

#[test]
fn counts_a_duration_from_zero() {
    let dur = |tc| parse_duration(tc, sig(4, 4)).unwrap();
    assert_eq!(dur("0:0:0:0"), 0.0);
    assert_eq!(dur("1:0:0:0"), 4.0);
    assert_eq!(dur("0:1:0:0"), 1.0);
    assert_eq!(dur("0:0:2:0"), 1.0);
    assert_eq!(dur("0:0:0:2"), 0.5);
}

#[test]
fn reads_ticks_as_a_remainder_never_shifted() {
    near(pos("1.5"), 5.0 / TICKS_PER_QUARTER, 1e-9);
    assert_eq!(pos("1:1.0"), 0.0);
    assert_eq!(parse_duration("0:0.0", sig(4, 4)).unwrap(), 0.0);
}

#[test]
fn reads_a_deep_chain_by_the_divide_by_four_rule() {
    near(pos("1:1:1:1:3"), 2.0 / 64.0, 1e-9);
    assert!(parse_duration("0:0:0:0:0:1:3:2", sig(4, 4)).unwrap() > 0.0);
}

#[test]
fn refuses_what_is_not_a_timecode_with_the_typescript_message() {
    for bad in ["a:b", "", "1:1.2.3", "1:1.", ".5", "1:", ":1", "1::2", "1:1\n2", "١:١"] {
        let e = parse_position(bad, sig(4, 4)).unwrap_err();
        assert_eq!(
            e,
            SchemaError::BadTimecode(format!(
                "bad timecode '{bad}' — expected \"mesure:noire:croche:doublecroche\", optionally \".ticks\""
            ))
        );
    }
}

#[test]
fn trims_as_javascript_does() {
    assert_eq!(pos("\u{feff} 1:2 \t\n"), 1.0);
    assert_eq!(pos("\u{a0}1:2\u{3000}"), 1.0);
    // U+0085 is white space to Rust and not to JavaScript.
    assert!(parse_position("\u{85}1:2", sig(4, 4)).is_err());
}

#[test]
fn reads_a_huge_field_as_a_float() {
    let big = "9".repeat(30);
    assert!(pos(&format!("{big}:1")).is_finite());
    assert!(pos(&format!("{}:1", "9".repeat(400))).is_infinite());
}

#[test]
fn recognises_the_shape_without_trimming() {
    assert!(is_timecode("1:1:1"));
    assert!(is_timecode("0:0.120"));
    assert!(!is_timecode(" 1:1"));
    assert!(!is_timecode("1:1."));
    assert!(!is_timecode(""));
}

#[test]
fn writes_positions_and_durations() {
    let at = |b| beats_to_position(b, sig(4, 4)).unwrap();
    assert_eq!(at(0.375), "1:1:1:2.120");
    assert_eq!(at(0.0), "1:1");
    assert_eq!(at(4.0), "2:1");
    assert_eq!(at(0.5), "1:1:2");
    assert_eq!(at(0.25), "1:1:1:2");
    assert_eq!(at(0.75), "1:1:2:2");
    assert_eq!(at(126.999755859375), "32:4");
    assert_eq!(at(10.0 / TICKS_PER_QUARTER), "1:1.10");
    assert_eq!(at(0.125), "1:1.120");
    assert_eq!(beats_to_duration(1.0, sig(4, 4)).unwrap(), "0:1");
    assert_eq!(beats_to_duration(0.0, sig(4, 4)).unwrap(), "0:0");
    assert_eq!(beats_to_duration(-3.0, sig(4, 4)).unwrap(), "0:0");
}

#[test]
fn round_trips_on_every_signature() {
    for (num, den) in [(4, 4), (3, 4), (6, 8), (7, 8)] {
        for b in [0.0, 1.0, 2.5, 4.0, 7.0, 12.25, 0.0625] {
            let s = sig(num, den);
            near(parse_position(&beats_to_position(b, s).unwrap(), s).unwrap(), b, 1e-9);
            near(parse_duration(&beats_to_duration(b, s).unwrap(), s).unwrap(), b, 1e-9);
        }
    }
}

#[test]
fn refuses_beats_that_are_not_a_time() {
    assert!(beats_to_position(f64::NAN, sig(4, 4)).is_err());
    assert!(beats_to_position(f64::INFINITY, sig(4, 4)).is_err());
    assert!(beats_to_position(1e300, sig(4, 4)).is_err());
    assert_eq!(beats_to_position(-1e300, sig(4, 4)).unwrap(), "1:1");
    assert_eq!(beats_to_duration(f64::NEG_INFINITY, sig(4, 4)).unwrap(), "0:0");
}

#[test]
fn rounds_a_beat_to_six_decimals() {
    assert_eq!(round_beat(0.1 + 0.2), 0.3);
    assert_eq!(round_beat(1.0000004), 1.0);
    assert_eq!(round_beat(1.0000006), 1.000001);
    assert_eq!(round_beat(-2.5), -2.5);
    assert_eq!(round_beat(3.0 + 1.0 / 3.0).to_string(), "3.333333");
}

#[test]
fn a_time_signature_needs_both_parts() {
    assert!(TimeSig::new(0, 4).is_err());
    assert!(TimeSig::new(4, 0).is_err());
    assert_eq!(sig(6, 8).bar(), 3.0);
}

// The float meter is the signature's reading where both fit a TimeSig, and reaches a numerator no TimeSig holds.
#[test]
fn a_float_meter_agrees_with_a_signature_and_reaches_past_u32() {
    for (n, d) in [(4u32, 4u32), (3, 4), (7, 8), (5, 4)] {
        for tc in ["1:1", "2:3", "3:2:2.5"] {
            assert_eq!(
                parse_position_on(tc, f64::from(n), f64::from(d)).unwrap(),
                parse_position(tc, sig(n, d)).unwrap()
            );
        }
        assert_eq!(
            beats_to_position_on(9.5, f64::from(n), f64::from(d)).unwrap(),
            beats_to_position(9.5, sig(n, d)).unwrap()
        );
    }
    assert_eq!(
        parse_position_on("2:1", 99_999_999_999.0, 4.0).unwrap(),
        99_999_999_999.0
    );
}

#[test]
fn reads_a_duration_on_a_float_meter_as_on_its_integer_one() {
    assert_eq!(
        parse_duration_on("1:2", 4.0, 4.0).unwrap(),
        parse_duration("1:2", sig(4, 4)).unwrap()
    );
    assert_eq!(parse_duration_on("1:0", 7.0, 8.0).unwrap(), 3.5);
    assert!(parse_duration_on("1:x", 4.0, 4.0).is_err());
}

#[test]
fn states_the_pattern_its_matcher_follows() {
    assert_eq!(TIMECODE_PATTERN, r"^[0-9]+(:[0-9]+)*(\.[0-9]+)?$");
    for (s, ok) in [
        ("1", true),
        ("1:2:3:4:5", true),
        ("0:0.120", true),
        ("1.", false),
        (":1", false),
        ("1:", false),
    ] {
        assert_eq!(is_timecode(s), ok, "{s}");
    }
}

#[test]
fn a_remainder_that_rounds_up_never_overflows_a_level() {
    // Rounding the sub-beat remainder on its own once wrote `32:3:3`.
    for b in [0.9999, 3.9999, 126.999755859375, 0.4999, 0.2499] {
        let tc = beats_to_position(b, sig(4, 4)).unwrap();
        let f: Vec<u32> = tc.split('.').next().unwrap().split(':').map(|n| n.parse().unwrap()).collect();
        assert!(f[1] <= 4, "{tc}");
        if let Some(q) = f.get(2) {
            assert!(*q <= 2, "{tc}");
        }
        if let Some(s) = f.get(3) {
            assert!(*s <= 2, "{tc}");
        }
    }
}

#[test]
fn every_time_it_can_write_reads_back_within_half_a_tick_and_a_triplet_is_exact() {
    for beats in [0.0, 0.125, 0.25, 1.0, 1.5, 4.0, 7.33, 61.7] {
        let tc = beats_to_position(beats, sig(4, 4)).unwrap();
        near(parse_position(&tc, sig(4, 4)).unwrap(), beats, 1e-3);
    }
    let third = 1.0 / 3.0;
    near(parse_position(&beats_to_position(third, sig(4, 4)).unwrap(), sig(4, 4)).unwrap(), third, 1e-6);
}

#[test]
fn the_shape_matches_exactly_where_the_parser_reads_a_timecode() {
    for tc in ["1", "1:2", "1:1:1:1", "0:0.120", "1:1:1:1:3", "1.5"] {
        assert!(is_timecode(tc), "{tc}");
        assert!(parse_position(tc, sig(4, 4)).unwrap().is_finite(), "{tc}");
    }
    for tc in ["", "a:b", "1:1.", ".5", "1:1.2.3", ":1", "1:"] {
        assert!(!is_timecode(tc), "{tc}");
        assert!(matches!(parse_position(tc, sig(4, 4)), Err(SchemaError::BadTimecode(_))), "{tc}");
    }
}

#[test]
fn a_beat_rounds_as_math_round_does_over_a_spread_of_values() {
    for x in [0.0, 0.125, 7.999999999, 1e-7, 123.4567895] {
        assert_eq!(round_beat(x), (x * 1e6).round() / 1e6, "{x}");
    }
}
