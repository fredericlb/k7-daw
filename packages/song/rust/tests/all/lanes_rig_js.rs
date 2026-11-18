// lanes_rig_js.rs — the rig's channels, labels and cue pair against what the TypeScript's `normalizePreferences` answers for the same JSON.
// A reader would get wrong that the tables are Node's answers, run once and pasted: the one difference is a channel past what a u32 holds (250).

use serde_json::Value;
use song::lanes::rig::{CueOutput, Rig};

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("a JSON literal")
}

/// Channels the TypeScript keeps for `ch: <value>`, as a list of integers above zero.
const CHANNELS: &[(&str, &[u32])] = &[
    (r##""3""##, &[3]),
    (r##"" 4 ""##, &[4]),
    (r##""0x10""##, &[16]),
    (r##""1e2""##, &[100]),
    (r##"true"##, &[1]),
    (r##"[3]"##, &[3]),
    (r##"2.0"##, &[2]),
    (r##"0"##, &[]),
    (r##"-1"##, &[]),
    (r##"1.5"##, &[]),
    (r##"null"##, &[]),
    (r##"false"##, &[]),
    (r##""""##, &[]),
    (r##"[3, 4]"##, &[3, 4]),
    (r##""0b11""##, &[3]),
    (r##""0o17""##, &[15]),
    (r##""0X1f""##, &[31]),
    (r##""+5""##, &[5]),
    (r##""-5""##, &[]),
    (r##"".5e1""##, &[5]),
    (r##""5.""##, &[5]),
    (r##""1e""##, &[]),
    (r##""abc""##, &[]),
    (r##""Infinity""##, &[]),
    (r##""inf""##, &[]),
    (r##""nan""##, &[]),
    (r##""1_0""##, &[]),
    (r##""\u00a05\u2003""##, &[5]),
    (r##""\ufeff6""##, &[6]),
    (r##""\u00857""##, &[]),
    (r##"4294967295"##, &[4294967295]),
    (r##""4294967295""##, &[4294967295]),
    (r##"[[3]]"##, &[3]),
    (r##"[" 7 "]"##, &[7]),
    (r##"{}"##, &[]),
    (r##""1.e1""##, &[10]),
    (r##""0x""##, &[]),
    (r##""00012""##, &[12]),
    (r##""\u0663""##, &[]),
    (r##""0.0""##, &[]),
    (r##""-0""##, &[]),
    (r##""1e2.5""##, &[]),
    (r##"[[]]"##, &[]),
];

/// Values the TypeScript keeps as a channel above `u32::MAX`; the Rust drops them, the one difference of decision 250.
const PAST_U32: &[&str] = &[
    r##""1e300""##,
    r##"5000000000"##,
    r##"4294967296"##,
    r##""9007199254740993""##,
    r##"1e300"##,
    r##"1e+300"##,
];

/// The label the TypeScript keeps for `label: <value>`, None when the entry is dropped for an empty one.
const LABELS: &[(&str, Option<&str>)] = &[
    (r##"5"##, Some("5")),
    (r##"0"##, Some("0")),
    (r##"true"##, Some("true")),
    (r##"false"##, Some("false")),
    (r##"["a"]"##, Some("a")),
    (r##"{"a": 1}"##, Some("[object Object]")),
    (r##""x""##, Some("x")),
    (r##"null"##, None),
    (r##""""##, None),
    (r##"1.5"##, Some("1.5")),
    (r##"1e+21"##, Some("1e+21")),
    (r##"1e-07"##, Some("1e-7")),
    (r##"0"##, Some("0")),
    (r##"[1, [2, 3]]"##, Some("1,2,3")),
    (r##"[null, "a"]"##, Some(",a")),
    (r##"[]"##, None),
    (r##"[[]]"##, None),
    (r##""  s ""##, Some("  s ")),
    (r##"2"##, Some("2")),
    (r##"123456789012345680000"##, Some("123456789012345680000")),
];

/// The cue pair the TypeScript keeps for `cueOutput: <value>`: a list of channels, a trimmed label, or none.
const CUES: &[(&str, Option<&str>)] = &[
    (r##"["3", 4]"##, Some(r##"[3, 4]"##)),
    (r##"[" 5 ", true]"##, Some(r##"[5, 1]"##)),
    (r##"["x"]"##, None),
    (r##"[]"##, None),
    (r##"[0]"##, None),
    (r##"" p ""##, Some(r##""p""##)),
    (r##""   ""##, None),
    (r##"3"##, None),
    (r##"null"##, None),
    (r##"["1e1"]"##, Some(r##"[10]"##)),
];

fn channels_of(value: &str) -> Vec<u32> {
    let rig = Rig::from_json(&parse(&format!(
        r#"{{"channels": {{"in": [{{"ch": {value}, "label": "l"}}]}}}}"#
    )));
    rig.channels_in.first().map(|e| e.ch.clone()).unwrap_or_default()
}

// 250: a channel is read as `Number(value)` reads it, then kept when a whole number above zero.
#[test]
fn a_channel_is_what_javascript_number_makes_of_it() {
    for (value, want) in CHANNELS {
        assert_eq!(channels_of(value), *want, "ch: {value}");
    }
}

// 250: the same values inside a list, where each is read alone.
#[test]
fn a_channel_in_a_list_is_read_alone() {
    let rig = Rig::from_json(&parse(
        r#"{"channels": {"in": [{"ch": ["3", " 4 ", "0x10", "1e2", true, [3], 2.0, 0, -1, 1.5, null, false, "", [3, 4]], "label": "l"}]}}"#,
    ));
    assert_eq!(rig.channels_in[0].ch, [3, 4, 16, 100, 1, 3, 2]);
}

// 250: past what a channel's integer field holds, a value stays dropped; that is the one difference from the TypeScript.
#[test]
fn a_channel_past_the_integer_field_is_dropped() {
    for value in PAST_U32 {
        assert_eq!(channels_of(value), Vec::<u32>::new(), "ch: {value}");
    }
    assert_eq!(channels_of("4294967295"), [u32::MAX]);
}

// 250: a label is read as `String(value)` reads it; an entry whose label is empty goes.
#[test]
fn a_label_is_what_javascript_string_makes_of_it() {
    for (value, want) in LABELS {
        let rig = Rig::from_json(&parse(&format!(
            r#"{{"channels": {{"out": [{{"ch": 1, "label": {value}}}]}}}}"#
        )));
        let got = rig.channels_out.first().map(|e| e.label.as_str());
        assert_eq!(got, *want, "label: {value}");
    }
}

// 250: a missing label is dropped as well, and a label is never trimmed.
#[test]
fn a_missing_label_goes() {
    let rig = Rig::from_json(&parse(r#"{"channels": {"in": [{"ch": 1}, {"ch": 2, "label": null}]}}"#));
    assert!(rig.channels_in.is_empty());
}

// 250: cueOutput is a list read like channels, or a label trimmed like JavaScript's `trim`.
#[test]
fn the_cue_pair_is_what_the_typescript_keeps() {
    for (value, want) in CUES {
        let rig = Rig::from_json(&parse(&format!(r#"{{"cueOutput": {value}}}"#)));
        let got = match rig.cue_output {
            None => None,
            Some(CueOutput::Label(l)) => Some(Value::String(l)),
            Some(CueOutput::Channels(c)) => Some(serde_json::json!(c)),
        };
        assert_eq!(got, want.map(parse), "cueOutput: {value}");
    }
}
