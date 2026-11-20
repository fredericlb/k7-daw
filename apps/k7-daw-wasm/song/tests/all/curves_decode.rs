// curves_decode.rs — a curve file decoded: the kinds, parseCurve's defaults, and what the schema refuses; ported from curves.test.ts.
// A reader would get wrong that a refusal here is a Finding for the file, with no fallback: the TypeScript's clamps are unreachable.

use crate::curves_support;

use curves_support::{decode, file, json_of, same};
use song::curves::{CurveCondition, CurveFile};
use song_schema::CurveSpec;

fn refused(text: &str) {
    assert!(decode(text).is_err(), "{text} should be refused");
}

// curves.test.ts "reads an lfo with defaults (sine shape)"; decision 203.
#[test]
fn an_lfo_with_defaults_is_unipolar_sine_at_one_cycle() {
    let f = file("type(\"lfo\")\n");
    assert!(same(
        &f,
        r#"{"type":"lfo","polarity":"unipolar","amplitude":1,"shape":"sine","frequency":1,"phase":0}"#
    ));
}

// curves.test.ts "reads a shape, falling back to sine on an unknown one", read half kept; decision 202.
#[test]
fn a_shape_is_read_and_an_unknown_one_is_refused() {
    assert!(same(
        &file("type(\"lfo\")\nshape(\"saw\")\n"),
        r#"{"type":"lfo","polarity":"unipolar","amplitude":1,"shape":"saw","frequency":1,"phase":0}"#
    ));
    refused("type(\"lfo\")\nshape(\"noise\")\n");
}

// curves.test.ts "reads a custom curve, clamping point values to 0..1"; decision 203.
#[test]
fn a_custom_curve_clamps_its_point_values_and_keeps_its_polarity() {
    let f = file(
        "type(\"custom\")\npolarity(\"bipolar\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1.5), time(\"3:1:1\").value(-4)])\n",
    );
    assert!(same(
        &f,
        r#"{"type":"custom","polarity":"bipolar","amplitude":1,"points":[{"time":"1:1:1","value":0},{"time":"2:1:1","value":1},{"time":"3:1:1","value":0}]}"#
    ));
}

// curves.test.ts "rejects a bad shape": a zero frequency, no points, an unknown type; decision 202.
#[test]
fn a_zero_frequency_no_points_and_an_unknown_type_are_refused() {
    refused("type(\"lfo\")\nfrequency(0)\n");
    refused("type(\"lfo\")\nfrequency(-1)\n");
    refused("type(\"custom\")\npoints([])\n");
    refused("type(\"square\")\n");
    refused("");
    refused("polarity(\"bipolar\")\n");
    refused("type(1)\n");
}

// curves.test.ts "clamps an amplitude past the range, and reads none as the whole range": refused past it; decision 202.
#[test]
fn an_amplitude_past_the_range_is_refused_and_none_is_the_whole_range() {
    refused("type(\"lfo\")\namplitude(3)\n");
    refused("type(\"lfo\")\namplitude(-0.1)\n");
    let f = file("type(\"custom\")\npoints([time(\"1:1\").value(1)])\n");
    assert!(matches!(&f.spec, CurveSpec::Custom(p) if p.amplitude == 1.0));
    let half = file("type(\"lfo\")\namplitude(0.5)\n");
    assert!(matches!(&half.spec, CurveSpec::Lfo(l) if l.amplitude == 0.5));
}

// adsrCurve.test.ts "takes the four durations and the level".
#[test]
fn an_envelope_takes_its_four_durations_and_its_level() {
    let f = file("type(\"adsr\")\nattack(5)\ndecay(20)\nsustain(300)\nrelease(80)\nlevel(0.8)\n");
    assert!(same(
        &f,
        r#"{"type":"adsr","polarity":"unipolar","amplitude":1,"attack":5,"decay":20,"sustain":300,"release":80,"level":0.8}"#
    ));
}

// adsrCurve.test.ts "defaults the level rather than leaving the envelope silent"; decision 203.
#[test]
fn an_envelope_defaults_its_level_to_half_and_its_stages_to_zero() {
    assert!(same(
        &file("type(\"adsr\")\n"),
        r#"{"type":"adsr","polarity":"unipolar","amplitude":1,"attack":0,"decay":0,"sustain":0,"release":0,"level":0.5}"#
    ));
}

// adsrCurve.test.ts "refuses a duration that is not one, by name": a decode finding; decision 202.
#[test]
fn an_envelope_stage_that_is_not_a_duration_is_a_finding() {
    refused("type(\"adsr\")\ndecay(-5)\n");
    refused("type(\"adsr\")\nattack(1e400)\n");
    refused("type(\"adsr\")\nlevel(2)\n");
}

// curveConditions.test.ts "refuses a malformed condition by name"; decision 202.
#[test]
fn a_malformed_condition_is_refused_at_its_place() {
    let f = decode("type(\"lfo\")\nconditions([type(\"start\")])\n").expect_err("refused");
    assert_eq!((f.file.as_str(), f.line), ("curves/x.k7x", 2));
    for bad in [
        "conditions(1)",
        "conditions([1])",
        "conditions([{ at: \"1:1\" }])",
        "conditions([type(\"later\")])",
        "conditions([type(\"start\").at(\"1:1\").x(1)])",
        "conditions([type(\"start\").at(\"x\")])",
        "conditions([type(\"event\").event(\"\")])",
        "conditions([type(\"start\").at(\"1:1\").retrigger(\"yes\")])",
        "conditions([type(\"between\").start(\"1:1\")])",
    ] {
        refused(&format!("type(\"lfo\")\n{bad}\n"));
    }
}

// curveConditions.test.ts: each condition kind, with and without `retrigger`; the serialized keys are the schema's.
#[test]
fn each_condition_kind_is_read_with_its_retrigger() {
    let f = file(
        "type(\"lfo\")\nconditions([type(\"start\").at(\"3:1:1\"), type(\"end\").at(\"4:1:1\").retrigger(false), type(\"between\").start(\"1:1\").end(\"2:1\").retrigger(true), type(\"event\").event(\"Note On\")])\n",
    );
    let Some(cs) = &f.conditions else { panic!("conditions") };
    assert_eq!(cs.len(), 4);
    assert!(matches!(&cs[0], CurveCondition::Start { retrigger: None, .. }));
    assert!(matches!(
        &cs[1],
        CurveCondition::End {
            retrigger: Some(false),
            ..
        }
    ));
    assert!(matches!(
        &cs[2],
        CurveCondition::Between {
            retrigger: Some(true),
            ..
        }
    ));
    assert!(matches!(&cs[3], CurveCondition::Event { event, .. } if event == "Note On"));
    let j = json_of(&f);
    assert_eq!(j["conditions"][0], serde_json::json!({"type": "start", "at": "3:1:1"}));
    assert_eq!(j["conditions"][2]["type"], "between");
    assert_eq!(j["type"], "lfo");
}

// curveConditions.test.ts "is not carried when it is none"; decision 203: an empty list is absent too.
#[test]
fn an_empty_conditions_list_and_a_cut_of_none_are_absent() {
    let f = file("type(\"lfo\")\ncut(\"none\")\nconditions([])\n");
    let j = json_of(&f);
    assert!(j.get("cut").is_none() && j.get("conditions").is_none());
}

// curveConditions.test.ts "refuses a cut it does not know" and "keeps only its positive half".
#[test]
fn a_cut_is_kept_when_it_is_positive_or_negative_and_refused_otherwise() {
    assert_eq!(json_of(&file("type(\"lfo\")\ncut(\"positive\")\n"))["cut"], "positive");
    assert_eq!(json_of(&file("type(\"adsr\")\ncut(\"negative\")\n"))["cut"], "negative");
    refused("type(\"lfo\")\ncut(\"upper\")\n");
    refused("type(\"lfo\")\npolarity(\"both\")\n");
}

// curveConditions.test.ts "is for ever at 0, and refuses what is not a length"; decision 203.
#[test]
fn a_duration_of_no_length_is_absent_and_a_length_is_kept() {
    assert!(
        json_of(&file("type(\"lfo\")\nduration(\"0\")\n"))
            .get("duration")
            .is_none()
    );
    assert!(
        json_of(&file("type(\"lfo\")\nduration(\"0:0\")\n"))
            .get("duration")
            .is_none()
    );
    assert_eq!(json_of(&file("type(\"lfo\")\nduration(\"2:0\")\n"))["duration"], "2:0");
    refused("type(\"lfo\")\nduration(\"soon\")\n");
    refused("type(\"lfo\")\nduration(\"\")\n");
    refused("type(\"lfo\")\nduration(2)\n");
}

// curveConditions.test.ts "is read off or on … whatever the file says between": a value between is refused; decision 202.
#[test]
fn an_on_off_curve_is_unipolar_at_full_height_and_refuses_a_value_between() {
    refused("type(\"onoff\")\npoints([time(\"1:1:1\").value(0.7)])\n");
    refused("type(\"onoff\")\npoints([time(\"1:1:1\").value(0.5)])\n");
    let f = file("type(\"onoff\")\npoints([time(\"1:1:1\").value(1), time(\"2:1:1\").value(0)])\n");
    assert!(same(
        &f,
        r#"{"type":"onoff","polarity":"unipolar","amplitude":1,"points":[{"time":"1:1:1","value":1},{"time":"2:1:1","value":0}]}"#
    ));
    for extra in ["polarity(\"bipolar\")", "cut(\"positive\")", "amplitude(0.5)"] {
        refused(&format!("type(\"onoff\")\n{extra}\npoints([time(\"1:1\").value(1)])\n"));
    }
}

// decision 202: every kind refuses a key it does not know, and a point its shape.
#[test]
fn an_extra_key_and_a_malformed_point_are_refused_on_every_kind() {
    for head in ["lfo", "adsr"] {
        refused(&format!("type(\"{head}\")\nnonsense(1)\n"));
    }
    refused("type(\"custom\")\nnonsense(1)\npoints([time(\"1:1\").value(1)])\n");
    refused("type(\"onoff\")\nnonsense(1)\npoints([time(\"1:1\").value(1)])\n");
    refused("type(\"custom\")\npoints(1)\n");
    refused("type(\"custom\")\npoints([time(\"1:1\").value(\"x\")])\n");
    refused("type(\"custom\")\npoints([value(1)])\n");
    refused("type(\"custom\")\npoints([time(1).value(1)])\n");
    refused("type(\"custom\")\npoints([time(\"x\").value(1)])\n");
    refused("type(\"custom\")\npoints([time(\"1:1\").value(1).extra(1)])\n");
    refused("type(\"lfo\")\nphase(2)\n");
    refused("type(\"lfo\")\nfrequency(1e400)\n");
}

// decision 201: the file serializes as parseCurve answers, flattened, and decodes the same again.
#[test]
fn a_decoded_file_serializes_with_the_typescripts_keys() {
    let f = file(
        "type(\"lfo\")\nshape(\"square\")\nfrequency(2)\nphase(0.25)\nduration(\"1:0\")\ncut(\"positive\")\nconditions([type(\"event\").event(\"noteOn\")])\n",
    );
    let j = json_of(&f);
    assert_eq!(j["shape"], "square");
    assert_eq!(j["duration"], "1:0");
    assert_eq!(j["conditions"][0]["event"], "noteOn");
    let _: &CurveFile = &f;
}
