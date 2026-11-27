// tests_buffer.rs — the buffer, paths, intake, kit, clipboard, draft and note-grid ops through the bytes the ABI carries.
// A reader would get wrong what is proved here: the rules have their own tests in song-edit and k7x; these pin the JSON shapes the page sends and reads.

use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

fn run(request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = Session::new().handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

fn ok(op: &str, args: Value) -> Value {
    let (status, v) = run(&json!({ "op": op, "args": args }));
    assert_eq!(status, OK, "{op}: {v}");
    v
}

const SEQ: &str =
    "// a bar of melody\nuse_rack(\"drums\")\nnotes([\n  t(0).d(1).k(60).v(0.8),\n  t(1).d(0.5).k(62),\n])\n";

fn read(text: &str) -> Value {
    ok("text.notes_read", json!({ "text": text }))
}

fn write(text: &str, notes: &Value) -> Value {
    ok("text.notes_write", json!({ "text": text, "notes": notes }))
}

#[test]
fn the_grids_read_the_notes_and_the_rack_a_sequence_names() {
    assert_eq!(
        read(SEQ),
        json!({ "notes": [{ "t": 0.0, "d": 1.0, "k": 60.0, "v": 0.8 }, { "t": 1.0, "d": 0.5, "k": 62.0 }], "useRack": "drums" })
    );
    assert_eq!(
        read("notes(\"c4 d4\")\n")["notes"],
        json!([{ "t": 0.0, "d": 1.0, "k": 60.0 }, { "t": 1.0, "d": 1.0, "k": 62.0 }])
    );
    assert_eq!(read("notes([t(0).d(1)])\n")["notes"], json!([]));
    assert_eq!(read("source(\"x.wav\")\n"), Value::Null);
    assert_eq!(read("notes([t(0).d(\n"), Value::Null);
}

#[test]
fn a_write_of_the_same_notes_leaves_the_text_as_it_was() {
    let notes = read(SEQ)["notes"].clone();
    assert_eq!(write(SEQ, &notes), json!(SEQ));
}

#[test]
fn one_note_changed_is_one_line_changed_and_the_comments_and_rack_stay() {
    let next = write(
        SEQ,
        &json!([{ "t": 0, "d": 1, "k": 60, "v": 0.8 }, { "t": 1, "d": 0.5, "k": 64 }]),
    );
    let next = next.as_str().unwrap();
    let (before, after): (Vec<_>, Vec<_>) = (SEQ.lines().collect(), next.lines().collect());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.iter().zip(&after).filter(|(a, b)| a != b).count(), 1);
    assert!(next.contains("// a bar of melody") && next.contains("use_rack(\"drums\")"));
}

#[test]
fn the_key_is_rounded_to_an_integer_and_the_times_to_the_beat_grid() {
    let next = write(SEQ, &json!([{ "t": 0.1234567, "d": 1, "k": 60.4 }]));
    let got = read(next.as_str().unwrap());
    assert_eq!(got["notes"], json!([{ "t": 0.123457, "d": 1.0, "k": 60.0 }]));
}

#[test]
fn a_pads_effects_follow_the_note_it_still_is() {
    let text = "notes([t(0).d(1).k(36).repeat(2), t(1).d(1).k(38)])\n";
    let next = write(text, &json!([{ "t": 0, "d": 1, "k": 36 }, { "t": 2, "d": 1, "k": 38 }]));
    let (status, data) = run(&json!({ "op": "text.sequence_data", "args": { "text": next } }));
    assert_eq!(status, OK);
    assert_eq!(
        data["notes"],
        json!([{ "t": 0.0, "d": 1.0, "k": 36.0, "repeat": 2.0 }, { "t": 2.0, "d": 1.0, "k": 38.0 }])
    );
}

#[test]
fn a_short_form_is_written_in_full_once_it_no_longer_says_the_notes_and_a_broken_text_is_null() {
    let next = write("notes(\"c4 d4\")\n", &json!([{ "t": 0, "d": 1, "k": 60 }]));
    assert_eq!(
        read(next.as_str().unwrap())["notes"],
        json!([{ "t": 0.0, "d": 1.0, "k": 60.0 }])
    );
    assert_eq!(write("notes([t(0).d(\n", &json!([])), Value::Null);
}

#[test]
fn a_clip_window_write_answers_the_splices_and_a_missing_symbol_answers_false() {
    let grid = json!({ "tempo": 110, "num": 4, "den": 4 });
    let text = "source(\"clips/a.wav\")\nfrom(1.32)\nto(4.36)\n";
    let v = ok(
        "text.clip_window",
        json!({ "text": text, "from": 2, "to": 5, "grid": grid }),
    );
    assert_eq!(v["text"], "source(\"clips/a.wav\")\nfrom(2)\nto(5)\n");
    assert_eq!(v["results"], json!([true]));
    assert!(v["changes"].as_array().is_some_and(|c| !c.is_empty()));
    let v = ok(
        "text.slice_window",
        json!({ "text": "extends(\"x\")\n", "symbol": "BD", "from": 0, "to": 1 }),
    );
    assert_eq!(v["results"], json!([false]));
    let v = ok(
        "text.slice_window",
        json!({ "text": "x(\n", "symbol": "BD", "from": 0, "to": 1 }),
    );
    assert!(v["error"]["message"].is_string());
}

#[test]
fn the_buffer_reads_answer_in_the_shapes_the_page_expects() {
    let c = ok("buffer.cursor", json!({ "text": "steps([\n  ext" }));
    assert_eq!(
        c,
        json!({ "path": ["steps"], "inKey": true, "valueOf": null, "inSeqItem": false, "inString": false })
    );
    assert_eq!(
        ok("buffer.scalar", json!({ "text": "source(\"a.wav\")", "key": "source" })),
        "a.wav"
    );
    let t = ok("buffer.value_tokens", json!({ "text": "x({ in: [\"a\"] })\n" }));
    assert_eq!(t[0]["field"], "in");
    assert_eq!(t[0]["quoted"], true);
    assert_eq!(ok("buffer.key_tokens", json!({ "text": "x(\n" })), json!([]));
    let r = ok(
        "buffer.reads",
        json!({ "text": "io(\"in\")\nchannel([1, 2])\n", "path": "n.k7x" }),
    );
    assert_eq!(r, json!({ "slices": false, "io": "in", "channels": [1.0, 2.0] }));
    let p = ok("buffer.preview_triggers", json!({ "text": "source(\"k.wav\")\n" }));
    assert_eq!(p[0]["req"], json!({ "kind": "source", "source": "k.wav" }));
    let b = ok(
        "buffer.clip_band",
        json!({ "text": "source(\"a.wav\")\n", "grid": { "tempo": 120, "num": 4, "den": 4 }, "editable": true }),
    );
    assert_eq!(b, json!({ "from": 0.0, "to": null, "hollow": true }));
}

#[test]
fn the_paths_answer_by_rule_and_a_tree_flattens() {
    assert_eq!(
        ok("paths.is", json!({ "rule": "seq", "path": "sequences/a.k7x" })),
        true
    );
    assert_eq!(
        ok(
            "paths.is",
            json!({ "rule": "ownModule", "path": "modules/a/module.k7x" })
        ),
        true
    );
    assert_eq!(
        ok(
            "paths.relative",
            json!({ "path": "a.k7/clips/x.k7x", "project": "a.k7" })
        ),
        "clips/x.k7x"
    );
    assert_eq!(ok("paths.rack_key", json!({ "path": "clips/kick.k7x" })), Value::Null);
    let tree = json!([
        { "path": "a", "dir": true, "children": [{ "path": "a/x.k7x" }, { "path": "a/y.wav", "kind": "binary" }] },
        { "path": "b.k7x" }
    ]);
    assert_eq!(
        ok("paths.text_files", json!({ "nodes": tree })),
        json!(["a/x.k7x", "b.k7x"])
    );
    assert_eq!(
        ok("paths.entries", json!({ "nodes": tree })),
        json!([{ "path": "a", "dir": true }, { "path": "a/x.k7x", "dir": false }, { "path": "a/y.wav", "dir": false }, { "path": "b.k7x", "dir": false }])
    );
    assert_eq!(
        ok(
            "paths.module_behind",
            json!({ "content": "extends(\"a\")", "paths": { "base:a": "m/module.k7x" }, "projectRoot": "/r" })
        ),
        "/r/m/module.k7x"
    );
}

#[test]
fn the_intake_and_the_kit_answer_in_the_shapes_the_page_expects() {
    let plan = ok(
        "intake.plan",
        json!({ "names": [{ "name": "a.wav", "folded": "a.wav" }], "lanes": [{ "id": "gtr", "kind": "audio" }], "at": "gtr" }),
    );
    assert_eq!(
        plan,
        json!({ "declare": [{ "id": "a", "upload": "a.wav", "kind": "audio" }], "place": [{ "id": "a", "lane": "gtr" }], "refused": [] })
    );
    assert_eq!(
        ok("intake.decl", json!({ "upload": "a.wav" })),
        "source(\"clips/a.wav\")\n"
    );
    assert_eq!(
        ok(
            "intake.place_line",
            json!({ "lane": "m", "id": "x", "atSec": "NaN", "lengthSec": 1 })
        ),
        "picture add m x NaN 1"
    );
    let grid = ok("kit.grid", json!({}));
    assert_eq!(grid["size"], 4);
    assert_eq!(grid["cells"][0], json!({ "row": 3, "col": 0, "note": 36.0 }));
    let issues = ok(
        "kit.issues",
        json!({ "name": "d", "pads": [], "taken": [], "hasRack": false }),
    );
    assert_eq!(
        issues[0],
        json!({ "key": "rb.noRack", "params": { "module": "instruments/k7-rack" } })
    );
    let dest = json!({ "library": "k7-core" });
    assert_eq!(
        ok("kit.upload_dir", json!({ "dest": dest, "kit": "d", "project": "p" })),
        "_libraries/k7-core/samples/d"
    );
    assert_eq!(
        ok(
            "kit.upload_dir",
            json!({ "dest": "project", "kit": "d", "project": "p" })
        ),
        "p/samples/d"
    );
}

#[test]
fn the_clipboard_and_the_draft_round_trip_through_json() {
    let clip = ok(
        "clipboard.copy_notes",
        json!({ "notes": [{ "key": 60, "time": 0, "duration": 1 }], "sequence": "riff", "loKey": 60, "hiKey": 67, "from": 0, "to": 4 }),
    );
    assert_eq!(clip["kind"], "notes");
    let pasted = ok("clipboard.paste_notes", json!({ "clip": clip, "topKey": 67, "at": 8 }));
    assert_eq!(pasted, json!([{ "t": 8.0, "d": 1.0, "k": 60.0 }]));
    let ev = ok(
        "clipboard.copy_events",
        json!({ "rows": [{ "events": [{ "ref": "k", "start": 0, "event": { "from": "2:1", "loop": 3 } }] }], "from": 0, "to": 4 }),
    );
    assert_eq!(
        ev["events"][0],
        json!({ "row": 0, "ref": "k", "start": 0.0, "from": "2:1", "loop": 3.0 })
    );
    let draft = ok("draft.of", json!({}));
    assert_eq!(draft["shape"], "sine");
    let fields = ok("draft.fields", json!({ "draft": draft }));
    assert_eq!(fields["type"], "lfo");
    let p = ok("draft.preview", json!({ "draft": draft, "num": 4, "den": 4 }));
    assert!(p["solid"].as_array().is_some_and(|s| !s.is_empty()));
    let half = json!({ "draft": { "shape": "custom", "points": [{ "time": "1:", "value": 0 }] } });
    let mut bad = draft.clone();
    bad["shape"] = json!("custom");
    bad["points"] = json!([{ "time": "1:", "value": 0 }]);
    let (status, e) = run(&json!({ "op": "draft.preview", "args": { "draft": bad, "num": 4, "den": 4 } }));
    assert_eq!((status, e["code"].as_str()), (ERROR, Some("bad_timecode")));
    assert!(half.is_object());
}
