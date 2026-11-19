//! diff3.rs — every non-k7x settle of `fixtures/settle-vectors.json` answered without node's `settleFileOp` and `applyFileOp` (apps/k7/backend-rs/docs/parity.md).
//! What a reader gets wrong: the settle check runs with a `Js` that panics, so no answer can come from a host.

use std::sync::OnceLock;

use k7_sync::bases::TextBases;
use k7_sync::json::stringify;
use k7_sync::settle::{self, Js, JsError, Settled};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

fn fixture(name: &str) -> Value {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn sha(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}


/// A `Js` that must never be asked: every op here is a text or a removal.
struct NoJs;

impl Js for NoJs {
    fn settle(&self, _: Option<String>, op: Value, _: Vec<String>) -> Result<Settled, JsError> {
        panic!("a settle of {op} was asked of the host")
    }

    fn apply(&self, _: Option<String>, op: Value) -> Result<Option<String>, JsError> {
        panic!("an apply of {op} was asked of the host")
    }
}

/// settleReferee.ts's envelope for one native settle; None where the room parks what node applies.
fn envelope(current: Option<&str>, op: &Value, bases: &TextBases) -> Option<String> {
    let out = match settle::settle(&NoJs, current, op, bases) {
        Ok(st) => {
            let aside: Vec<Value> = st
                .aside
                .into_iter()
                .map(|(op, kept)| match kept {
                    None => json!({ "op": op }),
                    Some(k) => json!({ "op": op, "kept": k }),
                })
                .collect();
            // settleReferee.ts applies what settles, a throw reported as `threw`, as Room.submit does.
            match (st.apply, st.next) {
                (None, _) => json!({ "ok": { "apply": null, "aside": aside } }),
                (Some(_), Err(())) => return None,
                (Some(ap), Ok(next)) => match settle::apply(&NoJs, current, &ap) {
                    Ok(n) if n == next => {
                        json!({ "ok": { "apply": ap, "aside": aside, "next": next } })
                    }
                    Ok(n) => panic!("settle's next {next:?} is not apply's {n:?}"),
                    Err(e) => {
                        json!({ "ok": { "apply": ap, "aside": aside, "threw": { "name": e.name, "message": e.message } } })
                    }
                },
            }
        }
        Err(e) => json!({ "err": { "name": e.name, "message": e.message } }),
    };
    Some(stringify(&out))
}

#[test]
fn every_text_settle_of_the_settle_vectors_is_nodes_without_a_host() {
    static FILE: OnceLock<Value> = OnceLock::new();
    let file = FILE.get_or_init(|| fixture("settle-vectors.json"));
    let texts = file["texts"].as_array().unwrap();
    let (mut n, mut merges) = (0, 0);
    let mut wrong = Vec::new();
    for (i, v) in file["vectors"].as_array().unwrap().iter().enumerate() {
        let args = &v["args"];
        let op = &file["ops"][args["op"].as_u64().unwrap_or(u64::MAX) as usize];
        // A text op without its text never passes `is_file_op`, and node would store `undefined` (files.md).
        let textless = op["kind"] == "text" && !op["text"].is_string();
        if v["fn"] != "settle" || op.is_null() || op["kind"] == "k7x" || textless {
            continue;
        }
        let current = args["current"]
            .as_u64()
            .map(|k| texts[k as usize].as_str().unwrap());
        let kept: Vec<Value> = args["bases"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|k| texts[k.as_u64().unwrap() as usize].clone())
            .collect();
        let mut saved = Map::new();
        if let Some(f) = op["file"].as_str().filter(|_| !kept.is_empty()) {
            saved.insert(f.into(), Value::from(kept.clone()));
        }
        let bases = TextBases::from_saved(Some(&Value::Object(saved)));
        let Some(got) = envelope(current, op, &bases) else {
            continue;
        };
        let same = match v.get("expect").and_then(Value::as_str) {
            Some(e) => e == got,
            None => v["sha256"].as_str() == Some(sha(&got).as_str()),
        };
        if !same {
            wrong.push(format!(
                "vector {i} ({}): {got:.300} / {:.300}",
                v["from"], v["expect"]
            ));
        }
        merges += usize::from(!kept.is_empty() && op["kind"] == "text");
        n += 1;
    }
    assert!(
        wrong.is_empty(),
        "{} of {n}:\n{}",
        wrong.len(),
        wrong[..wrong.len().min(20)].join("\n")
    );
    assert!(n > 200 && merges > 60, "{n} settles, {merges} with bases");
}
