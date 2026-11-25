// transport.rs — the transport, the arm, the click and filing a recording on real fixtures: what is asked of the host, what is written.
// A reader would get wrong that a transport verb plays: it asks the engine route, and only the click's two verbs write a file.

mod support;

use serde_json::json;
use song_edit::Fs;
use support::Project;

#[test]
fn each_transport_move_is_asked_and_writes_nothing() {
    let mut p = Project::open("mute-solo");
    for action in ["play", "stop", "toggle", "rewind", "panic"] {
        let ran = p.ok(&format!("transport {action}"));
        assert_eq!(
            ran.answer.asks,
            [json!({ "route": "POST /verb-host/engine", "args": { "op": "transport", "action": action } })]
        );
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
    let ran = p.ok_answered("transport arm kicks on", |c| {
        assert_eq!(c["route"], "POST /verb-host/engine");
        assert_eq!(c["args"], json!({ "op": "arm", "track": "kicks", "armed": true }));
        json!({ "armed": true, "lines": ["kicks listens on in 1"] })
    });
    assert_eq!(ran.answer.log, ["kicks listens on in 1", "kicks: armed"]);
    assert_eq!(ran.calls.len(), 1);
    assert_eq!(ran.before, ran.after);
    let off = p.ok_answered("transport arm kicks off", |c| {
        assert_eq!(c["args"]["armed"], false);
        json!({ "armed": false })
    });
    assert_eq!(off.answer.log, ["kicks: disarmed"]);
    // No `armed` is an engine that never answered: its lines say so, and the verb claims nothing.
    let toggle = p.ok_answered("transport arm kicks", |c| {
        assert_eq!(c["args"], json!({ "op": "arm", "track": "kicks" }));
        json!({ "lines": ["the engine is offline"] })
    });
    assert_eq!(toggle.answer.log, ["the engine is offline"]);
    let failed = p.answered(
        "transport arm kicks on",
        |_| json!({ "error": "no engine", "code": "no-engine", "status": 409 }),
    );
    assert_eq!(failed.answer.threw.as_deref(), Some("no engine"));
    assert!(failed.answer.refused);
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
    let record = p.doc().record_dir;
    let dir = p.dir.clone();
    let ran = p.ok_answered("project record save \"/tmp/a take.wav\" pic.webm", |c| {
        assert_eq!(c["route"], "POST /verb-host/take/save");
        assert_eq!(
            c["args"],
            json!({ "recordDir": record, "file": "/tmp/a take.wav", "video": format!("{dir}/pic.webm") })
        );
        json!({ "path": format!("{record}/take-1.wav") })
    });
    assert_eq!(ran.answer.log, [format!("saved {record}/take-1.wav")]);
    assert_eq!(ran.before, ran.after);
    let picture = p.ok_answered("project record save none /films/pic.webm", |c| {
        assert_eq!(c["args"], json!({ "recordDir": record, "video": "/films/pic.webm" }));
        json!({ "path": "/films/pic.webm" })
    });
    assert_eq!(picture.answer.log, ["saved /films/pic.webm"]);
    let failed = p.answered("project record save take.wav", |_| json!({ "error": "no such take" }));
    assert_eq!(failed.answer.refuse, ["could not save the take: no such take"]);
    p.refused("project record save none", "usage: project record save");
    p.refused("project record save a b c", "usage: project record save");
    p.refused("project record save \"bad\\q\"", "usage: project record save");
    let unclosed = p.ok_answered("project record save \"open", |c| {
        assert_eq!(c["args"]["file"], "\"open");
        json!({ "path": "x" })
    });
    assert_eq!(unclosed.answer.log, ["saved x"]);
}
