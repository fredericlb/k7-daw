// tests_text.rs — the text.* operations, through the same bytes the ABI carries: one gesture, the new text and its splices.
// A reader would get wrong that a refused edit is an answer, not an error: `results` says false and the text is as it was.

use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

fn run(request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = Session::new().handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

const SEQ: &str = "notes([\n  t(0).d(1).k(60),\n  t(1).d(0.5).k(64).v(0.4),\n])\n";

#[test]
fn a_note_edit_answers_the_text_and_the_splice_that_made_it() {
    let (status, v) = run(&json!({"op": "text.sequence", "args": {
        "text": SEQ,
        "ops": [{"op": "setNoteVelocity", "k": 60, "t": 0, "v": 0.75}],
    }}));
    assert_eq!(status, OK);
    let text = v["text"].as_str().unwrap();
    assert_eq!(text, SEQ.replace("k(60)", "k(60).v(0.75)"));
    // The splice is a one-line diff: inserting `.v(0.75)` at the end of the first note's key.
    let change = &v["changes"][0];
    let (start, end) = (
        change["start"].as_u64().unwrap() as usize,
        change["end"].as_u64().unwrap() as usize,
    );
    let insert = change["insert"].as_str().unwrap();
    assert_eq!(format!("{}{insert}{}", &SEQ[..start], &SEQ[end..]), text);
    assert_eq!(v["results"], json!([true]));
}

#[test]
fn a_refused_note_edit_names_the_op_and_leaves_the_text() {
    let (status, v) = run(&json!({"op": "text.sequence", "args": {
        "text": SEQ,
        "ops": [{"op": "removeNote", "k": 60, "t": 9}],
    }}));
    assert_eq!(status, OK);
    assert_eq!(v["text"], SEQ);
    assert_eq!(v["error"]["op"], 0);
    assert!(v["error"]["message"].as_str().unwrap().contains("no note k60"));
}

#[test]
fn a_short_form_is_kept_while_its_notes_are_unchanged_and_written_in_full_once_they_are_not() {
    let short = "notes(\"c4 e4\")\n";
    let (_, same) = run(&json!({"op": "text.sequence", "args": {
        "text": short, "ops": [{"op": "setNoteVelocity", "k": 60, "t": 0}],
    }}));
    assert_eq!(same["text"], short);
    let (_, full) = run(&json!({"op": "text.sequence", "args": {
        "text": short, "ops": [{"op": "setNoteVelocity", "k": 64, "t": 1, "v": 0.5}],
    }}));
    assert!(!full["text"].as_str().unwrap().contains("notes(\""));
}

#[test]
fn a_lane_edit_is_by_index_and_a_refusal_is_a_false_not_an_error() {
    let lane = "events([ref(\"a\").start(0).length(2), ref(\"b\").start(2)])\n";
    let (_, v) = run(&json!({"op": "text.video", "args": {
        "text": lane,
        "ops": [{"op": "setFieldAt", "index": 1, "field": "start", "value": 3.0004}, {"op": "removeAt", "index": 9}],
    }}));
    assert_eq!(v["text"], lane.replace("start(2)", "start(3)"));
    assert_eq!(v["results"], json!([true, false]));
    let (_, listed) = run(&json!({"op": "text.video_list", "args": {"text": lane}}));
    assert_eq!(
        listed[1],
        json!({"ref": "b", "start": 2.0, "length": 0.0, "sourceFrom": 0.0})
    );
}

#[test]
fn a_text_that_does_not_read_is_a_failure_of_a_strict_family_and_nothing_to_a_lane() {
    let (_, playlist) = run(&json!({"op": "text.playlist", "args": {
        "text": "items([", "ops": [{"op": "append", "song": "x"}],
    }}));
    assert_eq!(playlist["text"], "items([");
    assert_eq!(playlist["error"]["op"], 0);
    assert!(playlist["error"]["syntax"]["line"].is_number());
    let (_, lane) = run(&json!({"op": "text.video", "args": {
        "text": "events([ref(", "ops": [{"op": "setFieldAt", "index": 0, "field": "start", "value": 1}],
    }}));
    assert_eq!(lane["results"], json!([false]));
    assert!(lane.get("error").is_none());
}

#[test]
fn the_readers_answer_issues_beside_an_empty_value() {
    let (_, cues) = run(&json!({"op": "text.mix_cues", "args": {"text": "cues([\"../x.wav\"])\n"}}));
    assert_eq!(cues["markers"], json!({}));
    assert_eq!(cues["issues"][0]["line"], 1);
    let (_, pins) =
        run(&json!({"op": "text.pins_parse", "args": {"text": "k7-core(7)\ndrums(\"draft\")\nbad(\"x\")\n"}}));
    assert_eq!(pins, json!({"k7-core": 7.0, "drums": "draft"}));
    let (_, written) = run(&json!({"op": "text.pins_write", "args": {"pins": {"b": 2, "a": "draft"}}}));
    assert_eq!(written, "a(\"draft\")\nb(2)\n");
}

#[test]
fn a_sugar_path_expands_what_it_names_and_the_hash_op_still_answers() {
    let (_, v) = run(&json!({"op": "text.sugar_expand", "args": {
        "path": "song/sequences/m.k7x", "data": {"notes": "c4 d4"},
    }}));
    assert_eq!(v["notes"][1], json!({"t": 1.0, "d": 1.0, "k": 62.0}));
    let (status, e) = run(&json!({"op": "text.sugar_expand", "args": {
        "path": "song/sequences/m.k7x", "data": {"notes": "c4 h4"},
    }}));
    assert_eq!(status, ERROR);
    assert!(e["message"].as_str().unwrap().contains("neither a note name"));
    let (status, h) = run(&json!({"op": "text.hash", "args": {"text": "x"}}));
    assert_eq!(status, OK);
    assert!(h.is_string());
}
