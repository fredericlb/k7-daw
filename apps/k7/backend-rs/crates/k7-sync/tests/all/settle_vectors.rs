//! settle_vectors.rs — the native k7x half held to node: every `settle` and `apply` vector of `settle-vectors.json`
//! (k7x and not), answered as `settleReferee.ts` answered it, envelope for envelope (apps/k7/backend-rs/docs/parity.md, "The settle").
//! What a reader gets wrong: an engine's TypeError matches by its class, since V8's words are not Rust's.

use std::sync::OnceLock;

use k7_sync::bases::TextBases;
use k7_sync::json::stringify;
use k7_sync::settle::{self, Native};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

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

/// An envelope without the message of an error the engine itself threw.
fn engine_error(envelope: &str) -> Option<Value> {
    let mut v: Value = serde_json::from_str(envelope).ok()?;
    let mut hit = false;
    for at in ["/err", "/ok/threw"] {
        if let Some(e) = v
            .pointer_mut(at)
            .filter(|e| matches!(e["name"].as_str(), Some("TypeError" | "RangeError")))
        {
            e["message"] = Value::Null;
            hit = true;
        }
    }
    hit.then_some(v)
}

fn matches(v: &Value, envelope: &str) -> bool {
    match v.get("expect").and_then(Value::as_str) {
        Some(e) if e != envelope => {
            engine_error(e).is_some() && engine_error(e) == engine_error(envelope)
        }
        Some(_) => true,
        None => {
            format!("{:x}", Sha256::digest(envelope.as_bytes())) == v["sha256"].as_str().unwrap()
        }
    }
}

fn error(e: &settle::JsError) -> Value {
    json!({ "err": { "name": e.name, "message": e.message } })
}

/// `settleReferee.ts`' `settle` operation: what applies, what is set aside, the file after; `threw` when the replay throws.
fn settle_envelope(current: Option<&str>, op: &Value, bases: &TextBases) -> Option<String> {
    let out = match settle::settle(&Native, current, op, bases) {
        Ok(st) => {
            let aside: Vec<Value> = st
                .aside
                .into_iter()
                .map(|(op, kept)| match kept {
                    None => json!({ "op": op }),
                    Some(k) => json!({ "op": op, "kept": k }),
                })
                .collect();
            match (st.apply, st.next) {
                (None, _) => json!({ "ok": { "apply": null, "aside": aside } }),
                // A text op without its text is parked by the room and never written (files.md).
                (Some(_), Err(())) if op["kind"] != "k7x" => return None,
                (Some(ap), next) => match settle::apply(&Native, current, &ap) {
                    Ok(n) => {
                        assert_eq!(next, Ok(n.clone()), "settle's next is not apply's");
                        // JSON drops node's `undefined`, which a text op without its text applies to.
                        if ap["kind"] == "text" && !ap["text"].is_string() {
                            json!({ "ok": { "apply": ap, "aside": aside } })
                        } else {
                            json!({ "ok": { "apply": ap, "aside": aside, "next": n } })
                        }
                    }
                    Err(e) => {
                        assert_eq!(next, Err(()));
                        json!({ "ok": { "apply": ap, "aside": aside, "threw": { "name": e.name, "message": e.message } } })
                    }
                },
            }
        }
        Err(e) => error(&e),
    };
    Some(stringify(&out))
}

#[test]
fn every_settle_and_apply_is_nodes() {
    let file = vectors();
    let texts = file["texts"].as_array().unwrap();
    let text = |a: &Value, k: &str| {
        a.get(k)
            .and_then(Value::as_u64)
            .map(|i| texts[i as usize].as_str().unwrap())
    };
    let (mut settles, mut applies, mut k7x) = (0, 0, 0);
    let mut wrong = Vec::new();
    for (i, v) in file["vectors"].as_array().unwrap().iter().enumerate() {
        let (fun, a) = (v["fn"].as_str().unwrap(), &v["args"]);
        if fun == "fileOpFor" {
            continue;
        }
        let op = &file["ops"][a["op"].as_u64().unwrap() as usize];
        let current = text(a, "current");
        let got = if fun == "settle" {
            let kept: Vec<Value> = a["bases"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|k| texts[k.as_u64().unwrap() as usize].clone())
                .collect();
            let mut saved = Map::new();
            if let Some(f) = op["file"].as_str().filter(|_| !kept.is_empty()) {
                saved.insert(f.into(), Value::from(kept));
            }
            let bases = TextBases::from_saved(Some(&Value::Object(saved)));
            settles += 1;
            settle_envelope(current, op, &bases)
        } else {
            applies += 1;
            Some(stringify(&match settle::apply(&Native, current, op) {
                Ok(t) => json!({ "ok": t }),
                Err(e) => error(&e),
            }))
        };
        k7x += usize::from(op["kind"] == "k7x");
        if let Some(got) = got.filter(|g| !matches(v, g)) {
            wrong.push(format!(
                "vector {i} ({} {fun}): {got:.300} / {:.300}",
                v["from"], v["expect"]
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} wrong:\n{}",
        wrong.len(),
        wrong[..wrong.len().min(20)].join("\n")
    );
    assert!(
        settles > 2000 && applies > 1000 && k7x > 2000,
        "{settles} settles, {applies} applies, {k7x} k7x"
    );
}

#[test]
fn a_k7x_op_on_an_unreadable_file_throws_and_parks() {
    let op = json!({ "kind": "k7x", "file": "a.k7x", "ops": [{ "path": ["a"], "k7x": "1" }] });
    let junk = "{{{ not k7x \u{0}\n  : : :";
    let got = settle::apply(&Native, Some(junk), &op);
    assert!(got.is_err(), "{got:?}");
    if let Ok(st) = settle::settle(&Native, Some(junk), &op, &TextBases::default()) {
        assert!(st.apply.is_none() || st.next.is_err());
    }
}
