// lanes_live.rs — each track's live input against the rig (decisions 257, 258): liveInput.test.ts and the monitor and gain readings.
// A reader would get wrong that the first input on a track wins, and that an input in the master lands on `@master`.

use crate::lanes_support;

use lanes_support::{inputs, inputs_with};
use serde_json::{Value, json};
use song::lanes::live::{LiveInput, MonitorMode, input_landing_track, live_inputs_of, monitor_mode};
use song::lanes::ownership::chain_ownership;
use song::modules::Monitor;
use song::routing::{Chain, Out};

const AMP: &str = "plugin({ type: \"internal\", name: \"Chorus\" })\n";

fn live_with(rec: &str, modules: &str, rig: &Value) -> indexmap::IndexMap<String, LiveInput> {
    let graph = format!("modules({{ {modules} }})\n");
    let files: Vec<(&str, &str)> = vec![
        ("nodes/mic.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/pad.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/amp.k7x", AMP),
        ("nodes/rec.k7x", rec),
        ("graph.k7x", &graph),
    ];
    let i = inputs_with(&files, rig);
    let lanes = i.lanes();
    let own = chain_ownership(&i, &lanes);
    live_inputs_of(&i, &lanes, &own)
}

fn live(rec: &str, modules: &str) -> indexmap::IndexMap<String, LiveInput> {
    live_with(rec, modules, &json!({}))
}

// liveInput.test.ts "lands on the track of the chain it sits in" and "feeds no other track".
#[test]
fn an_input_lands_on_the_track_of_the_chain_it_sits_in_and_no_other() {
    let l = live(
        "io(\"in\")\nchannel([1])\n",
        "mic: [\"rec\"], rec: [\"OUT\"], pad: [\"OUT\"]",
    );
    assert_eq!(l.keys().collect::<Vec<_>>(), ["mic"]);
    assert_eq!(l["mic"].channels, [1]);
}

// liveInput.test.ts "MAKES its own track" and "lands on the track its run ends at".
#[test]
fn an_input_with_no_lane_lands_on_its_own_chain_and_one_feeding_a_lane_on_that_lane() {
    let own = live("io(\"in\")\nchannel([2])\n", "rec: [\"OUT\"]");
    assert_eq!(own.keys().collect::<Vec<_>>(), ["rec"]);
    let feeds = live("io(\"in\")\nchannel([1])\n", "rec: [\"mic\"], mic: [\"OUT\"]");
    assert_eq!(feeds.keys().collect::<Vec<_>>(), ["mic"]);
}

// liveInput.test.ts "lands on the AUDIO half": the key is the lane's name; the half is 2h's.
#[test]
fn a_split_lane_is_keyed_by_the_lane_node() {
    let l = live(
        "io(\"in\")\nchannel([1])\n",
        "\"mic#audio\": [\"OUT\"], \"mic#midi\": [\"amp\"], mic: [\"rec\"], rec: [\"OUT\"]",
    );
    assert_eq!(l.keys().collect::<Vec<_>>(), ["mic"]);
}

// 257: a label resolved, a label absent, numbers, a pair of numbers, no channel.
#[test]
fn channels_come_from_a_label_numbers_or_nothing() {
    let rig = json!({"channels": {"in": [{"ch": [3, 4], "label": "synth"}]}});
    let m = "mic: [\"rec\"], rec: [\"OUT\"]";
    assert_eq!(
        live_with("io(\"in\")\nchannel(\"synth\")\n", m, &rig)["mic"].channels,
        [3, 4]
    );
    assert!(
        live_with("io(\"in\")\nchannel(\"ghost\")\n", m, &rig)["mic"]
            .channels
            .is_empty()
    );
    assert_eq!(live("io(\"in\")\nchannel([5, 6])\n", m)["mic"].channels, [5, 6]);
    assert!(live("io(\"in\")\n", m)["mic"].channels.is_empty());
    // An element past u32 is dropped.
    assert_eq!(live("io(\"in\")\nchannel([7, 99999999999])\n", m)["mic"].channels, [7]);
}

// 257: monitor each of true, false, "on", "off", "auto", absent; gain set and absent; a gain not finite is 0.
#[test]
fn monitor_and_gain_are_read_the_one_way() {
    let m = "mic: [\"rec\"], rec: [\"OUT\"]";
    let mode = |line: &str| live(&format!("io(\"in\")\n{line}"), m)["mic"].monitor;
    assert_eq!(mode("monitor(true)\n"), MonitorMode::On);
    assert_eq!(mode("monitor(false)\n"), MonitorMode::Off);
    assert_eq!(mode("monitor(\"on\")\n"), MonitorMode::On);
    assert_eq!(mode("monitor(\"off\")\n"), MonitorMode::Off);
    assert_eq!(mode("monitor(\"auto\")\n"), MonitorMode::Auto);
    assert_eq!(mode(""), MonitorMode::Auto);
    let gain = |line: &str| live(&format!("io(\"in\")\n{line}"), m)["mic"].gain;
    assert_eq!((gain("gain(-6)\n"), gain("")), (-6.0, 0.0));
    assert_eq!(monitor_mode(Some(&Monitor::Mode("loud".into()))), MonitorMode::Auto);
    assert_eq!(monitor_mode(None).name(), "auto");
}

// 257: two inputs on one lane: the first in chain order wins.
#[test]
fn the_first_input_on_a_track_wins() {
    let files: Vec<(&str, &str)> = vec![
        ("nodes/mic.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/a.k7x", "io(\"in\")\nchannel([1])\n"),
        ("nodes/b.k7x", "io(\"in\")\nchannel([2])\n"),
        ("graph.k7x", "modules({ a: [\"mic\"], b: [\"mic\"], mic: [\"OUT\"] })\n"),
    ];
    let i = inputs(&files);
    let lanes = i.lanes();
    let l = live_inputs_of(&i, &lanes, &chain_ownership(&i, &lanes));
    assert_eq!(l["mic"].channels, [1]);
    assert_eq!(l.len(), 1);
}

// 257: an input in the master chain lands on the master's owner.
#[test]
fn an_input_in_the_master_lands_on_the_master() {
    let l = live(
        "io(\"in\")\nchannel([1])\n",
        "mic: [\"MIX\"], MIX: [\"rec\"], rec: [\"OUT\"]",
    );
    assert!(l.contains_key("@master"), "{:?}", l.keys().collect::<Vec<_>>());
}

// 258: `from`, else the first `to` that is a lane, the `to` unsplit.
#[test]
fn the_landing_track_is_the_source_else_the_first_lane_run_to() {
    let is_lane = |id: &str| id == "mic";
    let out = |to: &str| Out {
        to: to.to_owned(),
        gain: 1.0,
        from_port: None,
    };
    let from = Chain {
        from: Some("x".into()),
        out: vec![out("mic")],
        ..Chain::default()
    };
    assert_eq!(input_landing_track(&from, &is_lane), Some("x"));
    let to = Chain {
        out: vec![out("OUT"), out("mic")],
        ..Chain::default()
    };
    assert_eq!(input_landing_track(&to, &is_lane), Some("mic"));
    // A port on a lane name is not a lane.
    let ported = Chain {
        out: vec![out("mic#audio")],
        ..Chain::default()
    };
    assert_eq!(input_landing_track(&ported, &is_lane), None);
    let blank = Chain {
        from: Some(String::new()),
        out: vec![out("mic")],
        ..Chain::default()
    };
    assert_eq!(input_landing_track(&blank, &is_lane), Some("mic"));
}
