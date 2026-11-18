// lanes_rig.rs — the rig from the host's JSON and the labels it resolves (decisions 250, 251): never failing, bounded.
// A reader would get wrong that this file is the rig's shape only: what a channel or a label may be is lanes_rig_js.rs, against the TypeScript.

use crate::lanes_support;

use lanes_support::fast;
use serde_json::{Value, json};
use song::lanes::rig::{CueOutput, Rig, resolve_cue_output, resolve_input};
use song::modules::Input;

fn rig(v: Value) -> Rig {
    Rig::from_json(&v)
}

fn sample() -> Rig {
    rig(json!({
        "channels": {
            "in": [{"ch": [3, 4], "label": "synth"}, {"ch": [1], "label": "guitar"}, {"ch": [9], "label": "synth"}],
            "out": [{"ch": [5, 6], "label": "phones"}]
        },
        "cueOutput": "phones"
    }))
}

// 250: a non-object is the empty rig.
#[test]
fn what_is_not_an_object_is_the_empty_rig() {
    for v in [Value::Null, json!("x"), json!(7), json!([1, 2]), json!(true)] {
        assert_eq!(rig(v), Rig::default());
    }
}

// 250: a channel is a whole number above zero that fits u32; the rest is dropped, and an entry left with no channel goes.
#[test]
fn a_channel_is_an_integer_above_zero_that_fits_and_the_rest_is_dropped() {
    let r = rig(json!({"channels": {"in": [
        {"ch": [1e300, 0, -1, 1.5, null, false, "", [3, 4]], "label": "none"},
        {"ch": [1, 0, 2, 4294967296u64, 4294967295u64, 3.0], "label": "some"},
        {"ch": 5, "label": "scalar"},
        {"ch": [1], "label": ""},
        {"ch": [1]},
        {"label": "no channels"},
        "junk", null, 4
    ]}}));
    assert_eq!(r.channels_in.len(), 2, "{:?}", r.channels_in);
    assert_eq!(r.channels_in[0].ch, [1, 2, u32::MAX, 3]);
    assert_eq!(
        (r.channels_in[1].label.as_str(), r.channels_in[1].ch.clone()),
        ("scalar", vec![5])
    );
    assert!(r.channels_out.is_empty() && r.cue_output.is_none());
}

// 250: at most 64 channels per label and 4,096 labels; keys other than channels and cueOutput are ignored.
#[test]
fn the_caps_bound_what_a_rig_keeps() {
    let many: Vec<u32> = (1..=100_000).collect();
    let r = fast("100,000 channels", 5, || {
        rig(json!({"channels": {"in": [{"ch": many, "label": "wide"}]}, "other": 1}))
    });
    assert_eq!(r.channels_in[0].ch.len(), 64);
    let labels: Vec<Value> = (0..1_000_000)
        .map(|n| json!({"ch": [1], "label": format!("l{n}")}))
        .collect();
    let r = fast("1,000,000 labels", 5, || rig(json!({"channels": {"in": labels}})));
    assert_eq!(r.channels_in.len(), 4_096);
    let big = "x".repeat(1 << 20);
    let r = rig(json!({"channels": {"out": [{"ch": [1], "label": big}]}}));
    assert_eq!(r.channels_out.len(), 1);
}

// 250: cueOutput is a list of channels or a trimmed label; every other shape is none.
#[test]
fn the_cue_output_of_every_wrong_shape_is_none() {
    assert_eq!(
        rig(json!({"cueOutput": [3, 4]})).cue_output,
        Some(CueOutput::Channels(vec![3, 4]))
    );
    assert_eq!(
        rig(json!({"cueOutput": "  phones "})).cue_output,
        Some(CueOutput::Label("phones".into()))
    );
    for v in [
        json!([]),
        json!([0, -1, "x", 1.5]),
        json!(""),
        json!("   "),
        json!(3),
        json!({"a": 1}),
        json!(null),
        json!(true),
    ] {
        assert_eq!(rig(json!({"cueOutput": v})).cue_output, None, "{v}");
    }
}

// 251: a label resolves to its first entry's channels; an absent one reports itself; numbers pass through.
#[test]
fn labels_resolve_first_wins_and_an_unknown_one_reports_itself() {
    let r = sample();
    let by = |l: &str| resolve_input(Some(&Input::Label(l.to_owned())), &r);
    assert_eq!(by("synth").channels, [3, 4]);
    assert_eq!(by("guitar").channels, [1]);
    let missing = by("ghost");
    assert_eq!(
        (missing.channels.is_empty(), missing.unknown_label.as_deref()),
        (true, Some("ghost"))
    );
    assert_eq!(by("").unknown_label, None);
    assert_eq!(resolve_input(None, &r).channels, Vec::<u32>::new());
    let nums = resolve_input(Some(&Input::Channels(vec![2, 0, 7, u64::MAX])), &r);
    assert_eq!((nums.channels, nums.unknown_label), (vec![2, 7], None));
    assert_eq!(resolve_cue_output(&r).channels, [5, 6]);
    assert_eq!(resolve_cue_output(&Rig::default()).channels, Vec::<u32>::new());
    let ghost = rig(json!({"cueOutput": "nowhere"}));
    assert_eq!(resolve_cue_output(&ghost).unknown_label.as_deref(), Some("nowhere"));
    // An output label is not an input label.
    let both = rig(json!({"channels": {"out": [{"ch": [1], "label": "x"}]}, "cueOutput": "x"}));
    assert_eq!(
        resolve_input(Some(&Input::Label("x".into())), &both)
            .unknown_label
            .as_deref(),
        Some("x")
    );
    assert_eq!(resolve_cue_output(&both).channels, [1]);
}

// 289: a resolve builds no index, so 100,000 calls on a rig of 4,096 labels per direction stay cheap.
#[test]
fn a_resolve_on_a_full_rig_builds_no_index() {
    let labels: Vec<Value> = (0..5_000)
        .map(|n| json!({"ch": [n + 1], "label": format!("l{n}")}))
        .collect();
    let r = rig(json!({"channels": {"in": labels.clone(), "out": labels}, "cueOutput": [3, 4]}));
    let n = Input::Channels(vec![1, 2]);
    let (a, b) = fast("100,000 resolves", 5, || {
        let mut last = (Vec::new(), Vec::new());
        for _ in 0..100_000 {
            last = (resolve_input(Some(&n), &r).channels, resolve_cue_output(&r).channels);
        }
        last
    });
    assert_eq!((a, b), (vec![1, 2], vec![3, 4]));
    assert_eq!(r.channels_in.len(), 4_096);
}
