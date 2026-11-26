// tests_rules.rs — the track, stems, records, picture, rules and module ops through the bytes the ABI carries.
// A reader would get wrong that the rules are tested here: they are tested in their crates, and this proves the wire (names, shapes, nulls).

use crate::Session;
use crate::ops::{ERROR, OK};
use serde_json::{Value, json};

fn ask(op: &str, args: Value) -> Value {
    let mut s = Session::new();
    let mut out = Vec::new();
    let status = s.handle(json!({ "op": op, "args": args }).to_string().as_bytes(), &mut out);
    assert_eq!(status, OK, "{op}: {}", String::from_utf8_lossy(&out));
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn a_track_name_answers_its_label_and_its_kind() {
    assert_eq!(ask("track.label", json!({ "name": "3.ge.lead" })), "lead");
    assert_eq!(
        ask("track.kind", json!({ "name": "ge.lead" })),
        json!({ "prefix": "ge", "family": "guitar", "label": "guitare électrique" })
    );
    assert_eq!(ask("track.kind", json!({ "name": "lead" })), Value::Null);
}

#[test]
fn a_deck_names_its_stems() {
    assert_eq!(ask("stems.lane", json!({ "deck": "deckA", "stem": "bass" })), "stemBassA");
    assert_eq!(ask("stems.platter", json!({ "deck": "deckB", "stem": "vocals" })), "platterVocalsB");
    assert_eq!(ask("stems.deck_platter", json!({ "deck": "deckA" })), "platterA");
    assert_eq!(ask("stems.control", json!({ "deck": "deckA", "stem": "drums" })), "deckA.stemDrums");
    assert_eq!(ask("stems.file", json!({ "stem": "other" })), "other.wav");
    assert_eq!(
        ask("stems.lane_of", json!({ "id": "stemOtherB", "decks": ["deckA", "deckB"] })),
        json!({ "deck": "deckB", "stem": "other" })
    );
    assert_eq!(ask("stems.lane_of", json!({ "id": "x", "decks": ["deckA"] })), Value::Null);
}

#[test]
fn a_stem_that_is_not_one_is_a_bad_request() {
    let mut s = Session::new();
    let mut out = Vec::new();
    let bad = json!({ "op": "stems.file", "args": { "stem": "guitar" } }).to_string();
    assert_eq!(s.handle(bad.as_bytes(), &mut out), ERROR);
}

#[test]
fn a_set_list_resolves_against_the_files_and_keeps_its_items() {
    let files = [
        "records/a/a.flac",
        "records/a/stems/drums.wav",
        "records/a/stems/bass.wav",
        "records/a/stems/other.wav",
        "records/a/stems/vocals.wav",
    ];
    let rows = ask(
        "records.resolve",
        json!({ "items": [{ "song": "records/a/a.flac", "variant": "live" }, { "song": "../x.wav" }], "files": files }),
    );
    assert_eq!(
        rows,
        json!([
            { "item": { "song": "records/a/a.flac", "variant": "live" }, "path": "records/a/a.flac", "present": true, "stems": true },
            { "item": { "song": "../x.wav" }, "path": null, "present": false, "stems": false },
        ])
    );
    let unread = ask("records.resolve", json!({ "items": [{ "song": "records/a/a.flac" }], "files": null }));
    assert_eq!(unread[0]["present"], false);
}

#[test]
fn a_record_has_a_path_a_name_a_folder_and_matches_words() {
    assert_eq!(ask("records.path_of", json!({ "line": "/records/t.mp3/" })), "records/t.mp3");
    assert_eq!(ask("records.path_of", json!({ "line": "../t.mp3" })), Value::Null);
    assert_eq!(ask("records.is_ref", json!({ "line": "records/t.mp3" })), true);
    assert_eq!(ask("records.name", json!({ "path": "r/Hells Bells.flac" })), "Hells Bells");
    assert_eq!(ask("records.folder", json!({ "path": "r/x.flac" })), "r");
    assert_eq!(ask("records.folder", json!({ "path": "x.flac" })), Value::Null);
    assert_eq!(ask("records.matches", json!({ "path": "records/AC-DC/h.flac", "query": "ac-dc h" })), true);
    let tree = json!([{ "path": "r", "dir": true, "children": [
        { "path": "r/a.flac", "dir": false },
        { "path": "r/stems", "dir": true, "children": [{ "path": "r/stems/d.wav", "dir": false }] },
    ] }]);
    assert_eq!(ask("records.audio_files_of", json!({ "tree": tree, "withStems": false })), json!(["r/a.flac"]));
    assert_eq!(
        ask("records.audio_files_of", json!({ "tree": tree, "withStems": true })),
        json!(["r/a.flac", "r/stems/d.wav"])
    );
}

#[test]
fn a_picture_line_parses_and_resolves() {
    assert_eq!(ask("picture.aspect_of", json!({ "w": 1280, "h": 720 })), json!({ "name": "16:9", "w": 1920.0, "h": 1080.0 }));
    assert_eq!(ask("picture.aspect_of", json!({ "w": 640, "h": 480 })), Value::Null);
    let s = ask("picture.parse_settings", json!({ "words": ["size=720p", "QUALITY=max", "from=1:30"] }));
    assert_eq!(s, json!({ "size": { "height": 720.0 }, "quality": "max", "from": 90.0 }));
    assert_eq!(ask("picture.parse_settings", json!({ "words": ["size=big"] })), Value::Null);
    let r = ask(
        "picture.resolve_settings",
        json!({ "settings": s, "scene": { "viewport": [1920, 1080], "fps": 30, "durationSec": 60 } }),
    );
    assert_eq!(r, json!({ "from": 90.0, "frameCount": 1.0, "size": [1280.0, 720.0] }));
}

#[test]
fn a_text_states_its_window_and_its_tags() {
    let text = "from(\"2:1\")\nto(\"4:1\")\nnotes([])\n";
    let win = ask("rules.seq_window", json!({ "text": text, "num": 4, "den": 4 }));
    assert_eq!(win, json!({ "from": 4.0, "to": 12.0 }));
    assert_eq!(ask("rules.seq_window", json!({ "text": "name(\"x\")", "num": 4, "den": 4 })), Value::Null);
    assert_eq!(ask("rules.window_span", json!({ "win": { "from": 4, "to": null }, "extent": 16 })), json!({ "from": 4.0, "to": 16.0 }));
    assert_eq!(ask("rules.window_span", json!({ "win": { "from": 16, "to": null }, "extent": 16 })), Value::Null);
    assert_eq!(ask("rules.project_tags", json!({ "text": "project({ tags: [\"live\", 3] })\n" })), json!(["live"]));
    assert_eq!(ask("rules.project_tags", json!({ "text": "project([\n" })), json!([]));
}

#[test]
fn a_module_is_keyed_referenced_and_given_its_files() {
    assert_eq!(ask("module.key_from_path", json!({ "path": "modules/audio/amp/nam/module.k7x" })), "amp/nam");
    assert_eq!(ask("module.key_from_path", json!({ "path": "audio/x" })), Value::Null);
    assert_eq!(ask("module.library_ref", json!({ "library": "k7-core", "rest": "audio/a" })), "@k7-core/audio/a");
    assert_eq!(ask("module.shader_uniform", json!({ "param": "P1" })), "uP1");
    let files = ["a.nam", "c.onnx", "@k7-core/irs/h.wav"];
    assert_eq!(ask("module.assets_for_param", json!({ "files": files, "param": "onnx" })), json!(["c.onnx"]));
    assert_eq!(ask("module.assets_for_param", json!({ "files": files, "param": "ir" })), json!(["@k7-core/irs/h.wav"]));
    assert_eq!(ask("module.assets_for_param", json!({ "files": files, "param": "x", "family": "models" })), json!(["a.nam"]));
}
