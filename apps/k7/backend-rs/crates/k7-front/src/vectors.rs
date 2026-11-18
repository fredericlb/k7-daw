//! vectors.rs — the AI settings against `fixtures/leaf-vectors.json`,
//! the reference as committed: Rust must answer what it holds, and seal what it seals.

use serde_json::Value;

use crate::routes::ai::{Change, hint_of, parse_patch};

fn vectors() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/leaf-vectors.json")).unwrap()
}

fn cases<'a>(v: &'a Value, name: &str) -> &'a Vec<Value> {
    v[name].as_array().unwrap()
}

#[test]
fn hints_as_typescript() {
    let v = vectors();
    for c in cases(&v, "hint") {
        assert_eq!(
            hint_of(c["key"].as_str().unwrap()),
            c["hint"].as_str().unwrap(),
            "{c}"
        );
    }
}

/// A TypeScript patch field as a change: absent keeps, null or `""` clears, a string sets.
fn change(data: &Value, k: &str) -> Change {
    match data.get(k) {
        None => Change::Keep,
        Some(Value::String(s)) if !s.is_empty() => Change::Set(s.clone()),
        Some(_) => Change::Clear,
    }
}

#[test]
fn reads_an_ai_patch_as_typescript() {
    let v = vectors();
    for c in cases(&v, "aiPatch") {
        let out = &c["out"];
        match parse_patch(&c["body"]) {
            Ok((key, model)) => {
                assert_eq!(out["ok"], true, "{c}");
                assert_eq!(key, change(&out["data"], "apiKey"), "{c}");
                assert_eq!(model, change(&out["data"], "model"), "{c}");
            }
            Err(e) => assert_eq!(out["error"].as_str(), Some(e.as_str()), "{c}"),
        }
    }
}
