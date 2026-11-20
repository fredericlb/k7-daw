// resolve_tests.rs — a spec as the file holds it, decoded and resolved against a time signature.

use crate::*;

fn sig() -> TimeSig {
    TimeSig::new(4, 4).unwrap()
}

fn spec(json: &str) -> CurveSpec {
    serde_json::from_str(json).unwrap()
}

fn resolve(json: &str) -> Result<Curve, SchemaError> {
    Curve::resolve(&spec(json), sig())
}

const SAW: &str = r#"{"type":"lfo","polarity":"unipolar","amplitude":1,"shape":"saw","frequency":1,"phase":0}"#;

#[test]
fn decodes_every_kind_and_ignores_unknown_fields() {
    let lfo = spec(
        r#"{"type":"lfo","polarity":"bipolar","amplitude":0.5,"cut":"positive","shape":"sine","frequency":2,"phase":0.25,"duration":"2:0","conditions":[{"type":"start","at":"1:1"}]}"#,
    );
    assert!(matches!(
        lfo,
        CurveSpec::Lfo(LfoSpec {
            cut: Some(Cut::Positive),
            ..
        })
    ));
    let custom = spec(r#"{"type":"custom","polarity":"unipolar","amplitude":1,"points":[{"time":"1:1","value":0}]}"#);
    assert!(matches!(custom, CurveSpec::Custom(_)));
    let onoff = spec(r#"{"type":"onoff","polarity":"unipolar","amplitude":1,"points":[]}"#);
    assert!(matches!(onoff, CurveSpec::Onoff(_)));
    let adsr = spec(
        r#"{"type":"adsr","polarity":"unipolar","amplitude":1,"attack":1,"decay":1,"sustain":1,"release":1,"level":0.5}"#,
    );
    assert!(matches!(adsr, CurveSpec::Adsr(_)));
}

#[test]
fn refuses_a_spec_that_is_not_one() {
    for bad in [
        r#"{"type":"lfo"}"#,
        r#"{"type":"noise","polarity":"unipolar","amplitude":1}"#,
        r#"{"type":"lfo","polarity":"sideways","amplitude":1,"shape":"saw","frequency":1,"phase":0}"#,
    ] {
        assert!(serde_json::from_str::<CurveSpec>(bad).is_err(), "{bad}");
    }
}

#[test]
fn an_absent_cut_is_none_and_is_not_written_back() {
    let c = resolve(SAW).unwrap();
    assert_eq!(c.sample(0.0), 0.0);
    let back = serde_json::to_string(&spec(SAW)).unwrap();
    assert!(!back.contains("cut") && !back.contains("duration"), "{back}");
}

#[test]
fn a_duration_that_is_absent_empty_or_zero_runs_for_ever() {
    let with = |d: &str| {
        resolve(&format!(
            r#"{{"type":"lfo","polarity":"unipolar","amplitude":1,"shape":"saw","frequency":1,"phase":0,"duration":"{d}"}}"#
        ))
        .unwrap()
        .span()
    };
    let endless = Span { beats: 5.0, solid: 4.0 };
    assert_eq!(resolve(SAW).unwrap().span(), endless);
    assert_eq!(with(""), endless);
    assert_eq!(with("0:0"), endless);
    assert_eq!(with("2:0"), Span { beats: 8.0, solid: 8.0 });
}

#[test]
fn resolves_points_in_beats_and_sorts_them_without_touching_the_spec() {
    let json = r#"{"type":"custom","polarity":"unipolar","amplitude":1,"points":[{"time":"2:1","value":1},{"time":"1:1","value":0}]}"#;
    let s = spec(json);
    let c = Curve::resolve(&s, sig()).unwrap();
    assert!((c.sample(2.0) - 0.5).abs() < 1e-12);
    let CurveSpec::Custom(PointsSpec { points, .. }) = s else {
        unreachable!()
    };
    assert_eq!(points[0].time, "2:1");
}

#[test]
fn a_bad_timecode_carries_the_typescript_message() {
    let e = resolve(r#"{"type":"custom","polarity":"unipolar","amplitude":1,"points":[{"time":"x","value":1}]}"#);
    assert_eq!(
        e.unwrap_err(),
        SchemaError::BadTimecode(
            "bad timecode 'x' — expected \"mesure:noire:croche:doublecroche\", optionally \".ticks\"".into()
        )
    );
}

#[test]
fn a_time_too_large_for_a_float_is_refused() {
    let big = "9".repeat(400);
    let json =
        format!(r#"{{"type":"custom","polarity":"unipolar","amplitude":1,"points":[{{"time":"{big}:1","value":1}}]}}"#);
    assert!(matches!(resolve(&json), Err(SchemaError::BadTimecode(_))));
}
