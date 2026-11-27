// tests_support.rs — the ops called by name through the bytes the ABI carries, for the tests that pin a rule the page used to test.
// A reader would get wrong that an edit refused by a writer is an Err of its sentence here, as the facade throws it.

use crate::ops::OK;
use crate::*;
use serde_json::{Value, json};

/// A call's status and its answer.
pub fn raw(op: &str, args: Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = Session::new().handle(json!({"op": op, "args": args}).to_string().as_bytes(), &mut out);
    (status, whole(serde_json::from_slice(&out).unwrap()))
}

/// A number JS would call whole is an integer here, so `json!` literals compare as the page's did.
pub fn whole(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9e15 => json!(f as i64),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(whole).collect()),
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, whole(v))).collect()),
        other => other,
    }
}

/// A call that must answer its value.
pub fn call(op: &str, args: Value) -> Value {
    let (status, v) = raw(op, args);
    assert_eq!(status, OK, "{op}: {v}");
    v
}

/// What a writer left: its text and each op's answer, or the sentence it refused with.
pub fn edited(op: &str, args: Value) -> Result<(String, Vec<bool>), String> {
    let v = call(op, args);
    if let Some(e) = v.get("error") {
        return Err(e["message"].as_str().unwrap().to_owned());
    }
    let results = v["results"].as_array().unwrap().iter().map(|b| b.as_bool().unwrap()).collect();
    Ok((v["text"].as_str().unwrap().to_owned(), results))
}

/// A file's data as k7x reads it.
pub fn data(text: &str) -> Value {
    call("k7x.data", json!({"text": text}))["data"].clone()
}

/// The sentence of a call that must fail with an error status.
pub fn refused(op: &str, args: Value) -> String {
    let (status, v) = raw(op, args);
    assert_eq!(status, crate::ops::ERROR, "{op} answered {v}");
    v["message"].as_str().unwrap().to_owned()
}
