//! support — what the crate's tests share: the native k7x half, a seam that panics, and the settle vectors' readers.
//! What a reader gets wrong: texts in a vector are indices into its `texts` table.
#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use k7_replica::ops::{Js, JsError, K7x};
use k7_sync::settle::Native;
use k7_sync::settle::Settled;
use serde_json::{Map, Value};

/// The native k7x half, one for the whole test binary.
pub fn settler() -> Arc<Native> {
    static POOL: OnceLock<Arc<Native>> = OnceLock::new();
    POOL.get_or_init(|| Arc::new(Native)).clone()
}

/// A seam that panics when asked: proof that a native answer never crossed into JavaScript.
pub struct Panicking;

impl Js for Panicking {
    fn settle(&self, _: Option<String>, _: Value, _: Vec<String>) -> Result<Settled, JsError> {
        panic!("the seam was asked to settle")
    }

    fn apply(&self, _: Option<String>, _: Value) -> Result<Option<String>, JsError> {
        panic!("the seam was asked to apply")
    }
}

impl K7x for Panicking {
    fn k7x_op_for(&self, file: &str, _: &str, _: &str) -> Result<Option<Value>, JsError> {
        panic!("the seam was asked for {file}")
    }
}

/// A committed fixture of the backend's `fixtures/` folder, parsed.
pub fn fixture(name: &str) -> Value {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A settle vector's arguments with its text indices resolved, keys in the order node sent them.
pub fn resolve(file: &Value, args: &Value) -> Map<String, Value> {
    let texts = file["texts"].as_array().unwrap();
    let mut out = args.as_object().unwrap().clone();
    for k in ["current", "before", "after"] {
        if let Some(i) = out.get(k).and_then(Value::as_u64) {
            out[k] = texts[i as usize].clone();
        }
    }
    out
}
