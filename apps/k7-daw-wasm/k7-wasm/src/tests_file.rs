// tests_file.rs — the file.* and fields.* operations, through the same bytes the ABI carries.

use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

fn run(request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = Session::new().handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

#[test]
fn validates_a_buffer_as_the_shape_the_editor_draws() {
    let (status, v) =
        run(&json!({"op": "file.validate", "args": {"path": "modules/a/module.k7x", "text": "paramz(1)\n"}}));
    assert_eq!(status, OK);
    assert_eq!(
        v,
        json!({
            "ok": false,
            "issues": [{"path": "paramz", "message": v["issues"][0]["message"], "line": 1, "col": 1}],
            "depth": "schema",
            "malformed": false,
        })
    );
    assert!(v["issues"][0]["message"].as_str().unwrap().contains("paramz"));
}

#[test]
fn an_issue_leaves_out_a_position_it_does_not_have() {
    let (_, v) = run(&json!({"op": "file.validate", "args": {"path": "notes.txt", "text": "anything"}}));
    assert_eq!(
        v,
        json!({"ok": true, "issues": [], "depth": "none", "malformed": false})
    );
}

#[test]
fn counts_a_column_in_utf16_units() {
    let text = "params({ n: \"é😀\", a: ])\n";
    let (_, v) = run(&json!({"op": "file.validate", "args": {"path": "modules/a/module.k7x", "text": text}}));
    assert_eq!(v["malformed"], json!(true));
    let col = v["issues"][0]["col"].as_u64().unwrap() as usize;
    let before: String = text.chars().take_while(|c| *c != ']').collect();
    assert!(
        col > before.encode_utf16().count() - 3 && col <= before.encode_utf16().count() + 2,
        "{col}"
    );
}

#[test]
fn answers_the_fields_and_values_of_a_kind() {
    let (status, v) = run(&json!({"op": "fields.kind", "args": {"path": "song/chains/gtr.k7x"}}));
    assert_eq!((status, v), (OK, json!("chain")));
    let (_, v) = run(&json!({"op": "fields.kind", "args": {"path": "song/notes.txt"}}));
    assert_eq!(v, Value::Null);
    let (_, v) = run(&json!({"op": "fields.at", "args": {"path": "song/chains/gtr.k7x", "at": ["subchains"]}}));
    assert_eq!(v, json!(["id", "steps"]));
    let (_, v) =
        run(&json!({"op": "fields.values", "args": {"path": "song/chains/gtr.k7x", "at": [], "field": "start"}}));
    assert_eq!(v, json!(["true", "MIX", "RENDER"]));
    let (status, v) = run(&json!({"op": "fields.at", "args": {"path": "song/notes.txt", "at": []}}));
    assert_eq!((status, v), (OK, json!([])));
}

#[test]
fn refuses_a_request_whose_arguments_do_not_decode() {
    let (status, v) = run(&json!({"op": "file.validate", "args": {"path": 3}}));
    assert_eq!((status, &v["code"]), (ERROR, &json!("bad_request")));
}
