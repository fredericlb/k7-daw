//! text_bases.rs — the room's kept versions of a text, replayed against node's trace in `settle-vectors.json`
//! (apps/k7-daw-backend/docs/parity.md, "The settle"). What a reader gets wrong: sizes are UTF-16 units, as `text.length` is.

use std::sync::OnceLock;

use k7_js::bases::TextBases;
use serde_json::{Value, json};

fn vectors() -> &'static Value {
    static FILE: OnceLock<Value> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/settle-vectors.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    })
}

/// A trace text: a literal, or `ch` repeated `n` times after `tag`.
fn spec(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        o => Some(format!(
            "{}{}",
            o["tag"].as_str().unwrap(),
            o["ch"]
                .as_str()
                .unwrap()
                .repeat(o["n"].as_u64().unwrap() as usize)
        )),
    }
}

#[test]
fn text_bases_keep_what_node_keeps() {
    use k7_js::number::text_hash;
    let file = vectors();
    let probes: Vec<String> = ["v29\n", "v10\n", "v9\n", "y0\n", "y1\n", "", "again\n"]
        .map(String::from)
        .to_vec();
    let mut bases = TextBases::new();
    let mut slots = std::collections::HashMap::new();
    let mut files: Vec<String> = Vec::new();
    for (i, s) in file["bases"].as_array().unwrap().iter().enumerate() {
        let step = &s["step"];
        let f = step["file"].as_str().unwrap_or_default();
        match step["do"].as_str().unwrap() {
            "record" => {
                if !files.iter().any(|x| x == f) {
                    files.push(f.to_string());
                }
                bases.record(f, spec(&step["text"]).as_deref());
            }
            "versions" => {
                slots.insert(
                    step["slot"].as_str().unwrap().to_string(),
                    bases.versions(f),
                );
            }
            "restore" => {
                let kept = step["slot"]
                    .as_str()
                    .and_then(|k| slots.get(k).cloned().flatten());
                bases.restore(f, kept);
            }
            _ => {
                let saved = serde_json::to_string(&bases.to_json()).unwrap();
                bases = TextBases::from_json(&serde_json::from_str(&saved).unwrap());
            }
        }
        let kept: Vec<Value> = bases
            .to_json()
            .iter()
            .map(|(f, v)| {
                let each: Vec<Value> = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| {
                        let t = t.as_str().unwrap();
                        json!([t.encode_utf16().count(), text_hash(t)])
                    })
                    .collect();
                json!([f, each])
            })
            .collect();
        assert_eq!(Value::from(kept), s["kept"], "step {i}: {step}");
        let found: Vec<Value> = files
            .iter()
            .flat_map(|f| {
                probes.iter().map(|p| match bases.find(f, &text_hash(p)) {
                    Some(t) => Value::from(text_hash(t)),
                    None => Value::Null,
                })
            })
            .collect();
        assert_eq!(Value::from(found), s["found"], "step {i}: {step}");
    }
}
