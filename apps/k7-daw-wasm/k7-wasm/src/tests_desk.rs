// tests_desk.rs — the desk.* ops through the bytes the ABI carries: the request shapes the page sends and the answers it reads.
// A reader would get wrong what is proved here: the rules are `turntables`'s own tests; these are the JSON seams.

use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

fn run(s: &mut Session, request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = s.handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

fn ask(op: &str, args: Value) -> Value {
    let (status, v) = run(&mut Session::new(), &json!({ "op": op, "args": args }));
    assert_eq!(status, OK, "{op}: {v}");
    v
}

#[test]
fn a_beat_grid_is_read_from_its_onsets() {
    let v = ask(
        "desk.beat_phase",
        json!({ "transients": [0.25, 0.75, 1.25, 1.75], "bpm": 120.0 }),
    );
    assert!((v["phase"].as_f64().unwrap() - 0.25).abs() < 1e-9);
    assert_eq!(v["strength"], 1.0);
    assert_eq!(v["confident"], false);
}

#[test]
fn an_absent_tempo_is_a_missing_key() {
    assert_eq!(ask("desk.can_loop", json!({})), json!(false));
    assert_eq!(
        ask(
            "desk.can_loop",
            json!({ "tempo": { "natural": 120.0, "seconds": 10.0, "target": 120.0 } })
        ),
        json!(true)
    );
}

#[test]
fn the_stand_in_document_holds_the_decks_and_the_click() {
    let loads = json!({ "deckA": { "path": "/a.wav", "title": "A", "artist": null, "id": "a" }, "deckB": null });
    let v = ask("desk.document", json!({ "loads": loads, "tempos": {}, "playing": {} }));
    let ids: Vec<&str> = v["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["deckA", "deckB", "metronome"]);
    assert_eq!(v["tracks"][0]["plugins"][0]["deckFile"], "/a.wav");
}

#[test]
fn a_document_that_is_not_one_is_a_bad_request() {
    let (status, _) = run(
        &mut Session::new(),
        &json!({ "op": "desk.document", "args": { "compiled": { "nope": 1 }, "loads": {} } }),
    );
    assert_eq!(status, ERROR);
}

#[test]
fn a_scratch_waits_out_the_pace_and_a_release_never_waits() {
    assert_eq!(crate::typed_desk::k7_g_scratch_delay(4.0, 1.0, f64::NAN), 12.0);
    assert_eq!(crate::typed_desk::k7_g_scratch_delay(4.0, f64::NAN, f64::NAN), 0.0);
    assert_eq!(crate::typed_desk::k7_g_scratch_delay(40.0, 1.0, 16.0), 0.0);
}

#[test]
fn a_set_list_names_its_issue_by_key() {
    let v = ask("desk.resolve_set", json!({}));
    assert_eq!(v["issues"], json!(["notFound"]));
    let v = ask(
        "desk.resolve_set",
        json!({ "text": "title(\"Set\")\nitems([\"Fred/a.flac\"])\n", "files": ["Fred/a.flac"] }),
    );
    assert_eq!(v["entries"][0]["id"], "Fred/a.flac");
    assert_eq!(v["entries"][0]["file"], "Fred/a.flac");
}
