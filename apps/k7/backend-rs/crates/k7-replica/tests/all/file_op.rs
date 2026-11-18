//! file_op.rs — the native `file_op_for` held to node: every `fileOpFor` vector of `settle-vectors.json` (1113),
//! the answers of a file that is not k7x given a seam that panics, and the rest through the native k7x half.
//! What a reader gets wrong: the vector holds the answer's envelope, or its SHA-256 when it is long.

use crate::support;

use k7_replica::ops::{K7x, file_op_for};
use k7_sync::bases::is_k7x;
use k7_sync::json::stringify;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::{Panicking, fixture, resolve, settler};

/// True when the envelope of `answer` is what the vector holds.
fn holds(v: &Value, answer: Option<Value>) -> bool {
    let envelope = stringify(&json!({ "ok": answer }));
    match v.get("expect").and_then(Value::as_str) {
        Some(e) => e == envelope,
        None => {
            format!("{:x}", Sha256::digest(envelope.as_bytes())) == v["sha256"].as_str().unwrap()
        }
    }
}

#[test]
fn every_file_op_for_is_nodes_with_the_native_half_never_crossing() {
    let file = fixture("settle-vectors.json");
    let pool = settler();
    let (mut total, mut plain, mut through, mut hashed) = (0, 0, 0, 0);
    let mut wrong = Vec::new();
    for (i, v) in file["vectors"].as_array().unwrap().iter().enumerate() {
        if v["fn"] != "fileOpFor" {
            continue;
        }
        let a = resolve(&file, &v["args"]);
        let text = |k: &str| a.get(k).and_then(Value::as_str);
        let name = a["file"].as_str().unwrap();
        let hash = a.get("beforeHash").map(Value::as_str);
        let k7x: &dyn K7x = if is_k7x(name) { &*pool } else { &Panicking };
        let got = file_op_for(k7x, name, text("before"), text("after"), hash);
        if !holds(v, got.clone()) {
            wrong.push(format!(
                "vector {i} ({}): {:.300} / {:.300}",
                v["from"],
                stringify(&json!({ "ok": got })),
                v["expect"]
            ));
        }
        total += 1;
        if is_k7x(name) {
            through += 1;
        } else {
            plain += 1;
        }
        hashed += usize::from(hash.is_some());
    }
    assert!(
        wrong.is_empty(),
        "{} of {total}:\n{}",
        wrong.len(),
        wrong[..wrong.len().min(20)].join("\n")
    );
    println!(
        "fileOpFor vectors: {total} ({plain} not k7x, {through} k7x, {hashed} with beforeHash)"
    );
    assert_eq!((total, plain, through, hashed), (1048, 105, 943, 16));
}

#[test]
fn a_failing_seam_answers_the_whole_text() {
    use k7_replica::ops::{Js, JsError};
    use k7_sync::settle::Settled;
    struct Dead;
    impl Js for Dead {
        fn settle(&self, _: Option<String>, _: Value, _: Vec<String>) -> Result<Settled, JsError> {
            unreachable!()
        }
        fn apply(&self, _: Option<String>, _: Value) -> Result<Option<String>, JsError> {
            unreachable!()
        }
    }
    impl K7x for Dead {
        fn k7x_op_for(&self, _: &str, _: &str, _: &str) -> Result<Option<Value>, JsError> {
            Err(JsError {
                name: "Error".into(),
                message: "dead pool".into(),
            })
        }
    }
    // 1104: a seam that errs costs bandwidth, never the edit.
    let op = file_op_for(&Dead, "a.k7x", Some("a(1)\n"), Some("a(2)\n"), None).unwrap();
    assert_eq!(op["kind"], "text");
    assert_eq!(op["text"], "a(2)\n");
}
