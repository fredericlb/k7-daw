// tests_k7x.rs — the k7x.* ops through the bytes the ABI carries, and k7x's frozen edit vectors replayed as `k7x.edit` batches.
// A reader would get wrong what the replay proves: the op's JSON shapes are the vectors' own, so the page's ops are the ones they hold.

use crate::ops::OK;
use crate::*;
use serde_json::{Value, json};

fn run(s: &mut Session, request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = s.handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../k7x/tests/vectors/edit.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

const EDITS: [&str; 11] = [
    "setIn",
    "setSourceIn",
    "addIn",
    "replaceSource",
    "insertIn",
    "insertSourceIn",
    "moveIn",
    "renameKey",
    "applyIn",
    "deleteIn",
    "setHeader",
];

#[test]
fn every_vector_edit_replays_as_a_one_op_batch() {
    let mut s = Session::new();
    let (mut ran, mut skipped) = (0, 0);
    for c in vectors()["cases"].as_array().unwrap() {
        if c["chain"] == json!(true) {
            continue;
        }
        let text = &c["text"];
        for (op, want) in c["ops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(c["results"].as_array().unwrap())
        {
            if !EDITS.contains(&op["op"].as_str().unwrap()) {
                continue;
            }
            // A number JSON cannot spell never crosses: the page refuses it, or sends -0 as 0.
            if op.to_string().contains("\"$num\"") {
                skipped += 1;
                continue;
            }
            let (status, a) = run(&mut s, &json!({"op": "k7x.edit", "args": {"text": text, "ops": [op]}}));
            // An index no usize holds is refused by the page before it crosses (rustEdit.md).
            if status != OK {
                skipped += 1;
                continue;
            }
            let at = format!("{} {op}", c["name"]);
            match (&a["error"], want.get("e")) {
                (Value::Null, None) => assert_eq!(a["text"], want["t"], "{at}"),
                (e, Some(m)) if !e.is_null() => assert_eq!(&e["message"], m, "{at}"),
                (e, _) => panic!("{at}: got {e} / {}, want {want}", a["text"]),
            }
            ran += 1;
        }
    }
    assert!(ran > 2_000, "{ran} ops");
    assert!(skipped * 100 < ran, "{skipped} ops refused");
}

#[test]
fn a_chain_replays_as_one_batch_with_its_changes_in_utf16() {
    let mut s = Session::new();
    let mut chains = 0;
    for c in vectors()["cases"].as_array().unwrap() {
        if c["chain"] != json!(true) {
            continue;
        }
        let results = c["results"].as_array().unwrap();
        if results.iter().any(|r| r.get("e").is_some()) {
            continue;
        }
        let ops: Vec<&Value> = c["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| EDITS.contains(&o["op"].as_str().unwrap()))
            .collect();
        if ops.is_empty()
            || ops.len() != c["ops"].as_array().unwrap().len()
            || c["ops"].to_string().contains("\"$num\"")
        {
            continue;
        }
        let (status, a) = run(
            &mut s,
            &json!({"op": "k7x.edit", "args": {"text": c["text"], "ops": ops}}),
        );
        assert_eq!(status, OK, "{}", c["name"]);
        assert_eq!(a["error"], Value::Null, "{}", c["name"]);
        assert_eq!(a["text"], results.last().unwrap()["t"], "{}", c["name"]);
        let changes: Vec<Value> = a["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|ch| json!([ch["start"], ch["end"], ch["insert"]]))
            .collect();
        assert_eq!(json!(changes), c["changes"], "{}", c["name"]);
        chains += 1;
    }
    assert!(chains > 10, "{chains} chains");
}

#[test]
fn a_failed_op_keeps_the_edits_before_it() {
    let mut s = Session::new();
    let ops = json!([
        {"op": "setIn", "path": ["a"], "value": 2},
        {"op": "renameKey", "path": ["b"], "key": "a"},
        {"op": "setIn", "path": ["a"], "value": 3},
    ]);
    let (status, a) = run(
        &mut s,
        &json!({"op": "k7x.edit", "args": {"text": "a(1)\nb(1)\n", "ops": ops}}),
    );
    assert_eq!(status, OK);
    assert_eq!(a["text"], "a(2)\nb(1)\n");
    assert_eq!(a["results"], json!([null]));
    assert_eq!(a["error"]["op"], 1);
    assert_eq!(a["error"]["message"], "k7x: a is already a key there");
}

#[test]
fn an_edit_on_text_that_does_not_read_answers_where_it_stops() {
    let mut s = Session::new();
    let ops = json!([{"op": "setIn", "path": ["a"], "value": 2}]);
    let (_, a) = run(
        &mut s,
        &json!({"op": "k7x.edit", "args": {"text": "\"é\"(1", "ops": ops}}),
    );
    let syntax = &a["error"]["syntax"];
    assert_eq!((&syntax["line"], &syntax["offset"]), (&json!(1), &json!(5)));
}

#[test]
fn data_and_outline_answer_a_syntax_error_as_a_value() {
    let mut s = Session::new();
    for op in ["k7x.data", "k7x.outline"] {
        let (status, a) = run(&mut s, &json!({"op": op, "args": {"text": "a(1)\nb(" }}));
        assert_eq!(status, OK);
        assert_eq!(a["syntax"]["line"], 2, "{op}");
    }
    let (_, a) = run(
        &mut s,
        &json!({"op": "k7x.data", "args": {"text": "a(-0)\nc(\"é\")\n"}}),
    );
    assert_eq!(a["data"]["c"], "é");
}

#[test]
fn the_outline_counts_utf16_and_leaves_its_layout_in_the_cells() {
    let mut s = Session::new();
    let (_, a) = run(
        &mut s,
        &json!({"op": "k7x.outline", "args": {"text": "\"é\"(\"😀\")\nb([1])\n"}}),
    );
    assert_eq!(a["values"], json!(["é", "😀", "b", 1]));
    let c = &s.k7x.cells;
    // The root of two entries, then é's: key 0..3, entry 0..9, no dot, its scalar at 4..8.
    assert_eq!((c[0], c[3]), (3, 2));
    assert_eq!(&c[4..12], &[0, 3, 0, 9, u32::MAX, 0, 4, 8]);
}

#[test]
fn print_writes_the_header_the_one_line_paths_and_the_leads() {
    let mut s = Session::new();
    let value = json!({"a": {"x": 1, "y": [1, 2]}, "b": 2});
    let args =
        json!({"value": value, "header": ["top"], "inline": [["a"]], "lead": [{"path": ["b"], "lines": [" note"]}]});
    let (status, a) = run(&mut s, &json!({"op": "k7x.print", "args": args}));
    assert_eq!(status, OK);
    assert_eq!(a, "// top\na({ x: 1, y: [1, 2] })\n// note\nb(2)\n");
    let (_, e) = run(&mut s, &json!({"op": "k7x.print", "args": {"value": [1]}}));
    assert_eq!(e["code"], "bad_k7x");
}
