// reads_ports_misc.rs — the clip-name and lane-input cases the page's `clipNames` and `laneInput` suites held that `reads.rs` lacked.
// A reader would get wrong that an authored name keeps its `@start` off: the placement suffix is not part of the name.

use serde_json::{Value, json};
use song::reads::*;

fn graph(chains: Value) -> RGraph {
    serde_json::from_value(json!({ "chains": chains })).expect("graph")
}

#[test]
fn a_processed_copys_number_and_a_nested_placement_id_are_read() {
    use ClipNameParts::*;
    assert_eq!(
        clip_name_parts("track-1_1.1_261030T172747-2@32"),
        Take {
            lane: "track-1".into(),
            bar: 1.0,
            copy: Some(2.0)
        }
    );
    assert_eq!(
        clip_name_parts("verse@0>track-7-seq-1@4"),
        Sequence {
            lane: "track-7".into(),
            n: 1.0
        }
    );
    assert_eq!(
        clip_name_parts("intro_gtr@8"),
        Named {
            name: "intro_gtr".into()
        }
    );
}

#[test]
fn a_midi_device_or_an_audio_input_wired_straight_to_a_lane_is_its_input() {
    let g = graph(json!({
        "keys": { "use": [{ "module": "k", "device": "" }], "out": [{ "to": "lead" }] },
        "mic": { "use": [{ "module": "m", "monitor": "on" }], "out": [{ "to": "vox" }] },
    }));
    assert!(lane_input(&g, "lead").is_some());
    assert!(lane_input(&g, "vox").is_some());
}

#[test]
fn an_input_behind_a_preamps_chain_counts_and_a_lane_fed_by_a_plugin_alone_has_none() {
    let g = graph(json!({
        "mic": { "use": [{ "module": "m", "monitor": "auto" }], "out": [{ "to": "pre" }] },
        "pre": { "use": [{ "module": "pre" }], "out": [{ "to": "vox" }] },
        "synth": { "use": [{ "module": "osc" }], "out": [{ "to": "pad" }] },
        "bass": { "from": "bass", "use": [{ "module": "amp" }], "out": [{ "to": "MIX" }] },
    }));
    assert!(lane_input(&g, "vox").is_some());
    assert!(lane_input(&g, "pad").is_none());
    assert!(lane_input(&g, "bass").is_none());
}

#[test]
fn the_input_names_its_module_its_kind_and_the_fields_it_writes() {
    let g = graph(json!({
        "mic": { "use": [{ "module": "m", "monitor": "auto", "channel": [3, 4] }], "out": [{ "to": "vox" }] },
        "keys": { "use": [{ "module": "k", "device": "Juno" }], "out": [{ "to": "lead" }] },
    }));
    let vox = lane_input(&g, "vox").expect("an input");
    assert_eq!(
        (
            vox.module.as_str(),
            vox.kind.as_str(),
            vox.channel,
            vox.monitor.as_deref(),
            vox.device
        ),
        ("m", "audio", Some(vec![3.0, 4.0]), Some("auto"), None)
    );
    assert_eq!(lane_input(&g, "lead").map(|i| i.kind), Some("midi".to_owned()));
    assert!(lane_input(&g, "pad").is_none());
}

#[test]
fn a_folder_name_keeps_what_is_a_name_and_refuses_a_backslash_and_the_empty() {
    let sfx = vec![".k7".to_owned()];
    assert_eq!(folder_name("hiver", &sfx).folder, "hiver");
    assert_eq!(folder_name("hiver.K7", &sfx).folder, "hiver");
    assert_eq!(folder_name("prise v1", &sfx).folder, "prise v1");
    assert_eq!(folder_name("répèt du jeudi", &sfx).folder, "répèt du jeudi");
    assert_eq!(folder_name("Back in Black (démo)", &sfx).folder, "Back in Black (démo)");
    assert_eq!(folder_name("a\\b", &sfx).error, "noSlash");
    assert_eq!(folder_name("", &sfx).error, "required");
    assert_eq!(folder_name(".caché", &sfx).error, "noDot");
}

#[test]
fn the_trim_brings_the_peak_to_the_target_and_is_never_positive() {
    assert_eq!(trim_for(0.0), FIX_TARGET_DB);
    assert_eq!(trim_for(3.0), -9.0);
    assert_eq!(trim_for(12.0), -18.0);
    assert!(trim_for(-20.0) <= FIX_TARGET_DB);
    assert!(trim_for(-60.0) <= FIX_TARGET_DB);
    assert_eq!(trim_for(f64::NEG_INFINITY), FIX_TARGET_DB);
    assert_eq!(trim_for(-3.0), FIX_TARGET_DB);
}

#[test]
fn the_outs_volume_moves_relative_to_the_fader_and_stays_in_its_field() {
    assert_eq!(fixed_master(3.0, 6.0, -100.0, 12.0), -9.0);
    assert_eq!(fixed_master(-4.0, 0.0, -100.0, 12.0), -10.0);
    assert_eq!(fixed_master(0.0, 0.0, -100.0, 12.0), FIX_TARGET_DB);
    assert_eq!(fixed_master(-98.0, 0.0, -100.0, 12.0), -100.0);
    assert!(fixed_master(12.0, 0.0, -100.0, 12.0) < 12.0);
    for peak in [-60.0, -6.0, 0.0, 12.0] {
        assert!(fixed_master(-3.0, peak, -100.0, 12.0) <= -3.0);
    }
    assert_eq!(fixed_master(0.0, FIX_TARGET_DB, -100.0, 12.0), FIX_TARGET_DB);
}

#[test]
fn only_a_monitored_input_that_reaches_a_channel_can_feed_back() {
    assert!(gives_channel("node param IN @channel 1,2"));
    assert!(!gives_channel("node param IN @monitor on"));
    assert!(!gives_channel("node param IN Volume -6"));
    let audible = |monitor: &str, channel: Option<[u32; 1]>| {
        let mut m = json!({ "module": "m", "monitor": monitor });
        if let Some(c) = channel {
            m["channel"] = json!(c);
        }
        graph(json!({ "main": { "use": [m] } }))
    };
    assert!(has_audible_input(&audible("on", Some([1]))));
    assert!(!has_audible_input(&audible("on", None)));
    assert!(!has_audible_input(&audible("auto", Some([1]))));
}
