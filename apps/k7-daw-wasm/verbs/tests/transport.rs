// transport.rs — the transport, the arm, the click and filing a recording on real fixtures: what is asked of the host, what is written.
// A reader would get wrong that a transport verb plays: it asks the host, and only the click's two verbs write a file.

mod support;

use serde_json::json;
use song_edit::Fs;
use support::Project;

#[test]
fn each_transport_move_is_asked_and_writes_nothing() {
    let mut p = Project::open("mute-solo");
    for action in ["play", "stop", "toggle", "rewind", "panic"] {
        let ran = p.ok(&format!("transport {action}"));
        assert_eq!(ran.answer.asks, [json!({ "member": "transport", "args": [action] })]);
        assert_eq!(ran.before, ran.after);
        assert!(ran.answer.effects.is_empty());
    }
}

#[test]
fn an_arm_needs_a_lane_with_an_input() {
    let mut p = Project::open("mute-solo");
    p.refused("transport arm kicks", "\"kicks\" has no input — nothing to record");
    p.refused("transport arm nobody", "no track \"nobody\"");
    p.refused("transport arm kicks maybe", "usage: transport arm <track> [on|off]");
    // An `IN` node on channel 1 feeding the lane, as `track input` wires one.
    let mic = format!("{}/nodes/mic.k7x", p.dir);
    p.fs.write_atomic(&mic, b"extends(\"@meta/IN\")\nchannel([1])\n")
        .unwrap();
    let graph = format!("{}/graph.k7x", p.dir);
    let text =
        p.fs.text(&graph)
            .unwrap()
            .replace("modules({ ", "modules({ mic: [\"kicks\"], ");
    p.fs.write_atomic(&graph, text.as_bytes()).unwrap();
    assert_eq!(p.track("kicks").input_channels, [1]);
    let ran = p.ok("transport arm kicks on");
    assert_eq!(ran.answer.asks, [json!({ "member": "arm", "args": ["kicks", true] })]);
    assert_eq!(ran.answer.log, ["kicks: arm asked"]);
    assert_eq!(ran.before, ran.after);
    let off = p.ok("transport arm kicks off");
    assert_eq!(off.answer.asks[0]["args"], json!(["kicks", false]));
    let toggle = p.ok("transport arm kicks");
    assert_eq!(toggle.answer.asks[0]["args"], json!(["kicks", null]));
}

#[test]
fn the_metronome_flips_in_settings_and_the_mix_is_pushed() {
    let mut p = Project::open("markers-settings");
    assert!(p.doc().settings.metronome);
    let ran = p.round_trip("transport metronome");
    assert_eq!(ran.answer.log, ["metronome off"]);
    assert_eq!(ran.answer.effects, ["applyProject"]);
    p.ok("transport metronome");
    assert!(!p.doc().settings.metronome);
    assert!(p.text("settings.k7x").contains("metronome(false)"));
    p.ok("transport metronome");
    assert!(p.doc().settings.metronome);
}

#[test]
fn the_click_level_is_kept_to_two_decimals() {
    let mut p = Project::open("markers-settings");
    let ran = p.round_trip("transport metronome volume 0.6333");
    assert_eq!(ran.answer.log, ["metronome level 0.63"]);
    p.ok("transport metronome volume 0.6333");
    assert_eq!(p.doc().settings.metronome_level, 0.63);
    assert!(p.text("settings.k7x").contains("0.63"));
    assert!(p.doc().settings.metronome, "the switch is left alone");
    p.refused(
        "transport metronome volume 0.1",
        "to silence it use `transport metronome`",
    );
    p.refused("transport metronome volume 2", "the click's level runs 0.2 to 1");
    p.refused("transport metronome volume", "usage: transport metronome volume");
}

#[test]
fn a_take_is_filed_by_the_host() {
    let mut p = Project::open("mute-solo");
    let ran = p.ok("project record save \"/tmp/a take.wav\" pic.webm");
    assert_eq!(
        ran.answer.asks,
        [json!({ "member": "saveTake", "args": [{ "file": "/tmp/a take.wav", "video": "pic.webm" }] })]
    );
    assert_eq!(ran.answer.log, ["saving /tmp/a take.wav and pic.webm"]);
    assert_eq!(ran.before, ran.after);
    let picture = p.ok("project record save none pic.webm");
    assert_eq!(picture.answer.asks[0]["args"], json!([{ "video": "pic.webm" }]));
    p.refused("project record save none", "usage: project record save");
    p.refused("project record save a b c", "usage: project record save");
    p.refused("project record save \"bad\\q\"", "usage: project record save");
    let unclosed = p.ok("project record save \"open");
    assert_eq!(unclosed.answer.asks[0]["args"], json!([{ "file": "\"open" }]));
}
