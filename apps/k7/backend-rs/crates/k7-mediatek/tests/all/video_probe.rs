//! The probe's pure half replayed over what the TypeScript answered (`makeHostVectors.mts`): ffprobe's arguments and its JSON as facts.

use std::path::Path;

use k7_mediatek::video_probe::{facts_of, probe_args, rotation_of};
use serde_json::{Value, json};

fn vectors() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/host-probe.json");
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

/// Numbers as a JavaScript client reads them: 29.97 and 29.970 are one, so are 1920 and 1920.0.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

#[test]
fn ffprobes_arguments_are_the_typescripts() {
    let v = vectors();
    let cases = v["probeArgs"].as_array().unwrap();
    assert!(cases.len() >= 6);
    for c in cases {
        let want: Vec<String> = serde_json::from_value(c["args"].clone()).unwrap();
        assert_eq!(
            probe_args(c["source"].as_str().unwrap()),
            want,
            "{}",
            c["source"]
        );
    }
}

#[test]
fn ffprobes_json_is_read_into_the_typescripts_facts() {
    let v = vectors();
    let cases = v["facts"].as_array().unwrap();
    assert!(cases.len() >= 20);
    for c in cases {
        let got = facts_of(c["json"].as_str().unwrap())
            .unwrap()
            .unwrap_or(Value::Null);
        assert!(
            same(&got, &c["facts"]),
            "{}: {got} != {}",
            c["name"],
            c["facts"]
        );
    }
}

#[test]
fn text_that_is_no_json_is_an_error_and_a_side_data_rotation_beats_the_tag() {
    assert!(facts_of("not json").is_err());
    let s = json!({ "side_data_list": [{ "rotation": 90 }], "tags": { "rotate": "180" } });
    assert_eq!(rotation_of(&s), 90);
    assert_eq!(rotation_of(&json!({ "tags": { "rotate": "180" } })), 180);
    assert_eq!(rotation_of(&json!({})), 0);
}
