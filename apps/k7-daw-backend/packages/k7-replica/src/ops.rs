//! ops.rs — the ops a replica sends and replays (`@studio/live-sync`'s `fileOp.ts` and `ops.ts`): `fileOpFor` (whole
//! texts, removals, "nothing changed", 1103) and the batch builder. A k7x diff and a k7x replay are `k7_sync::k7x`'s,
//! asked through `K7x`; ops and batches stay `Value`s in the TypeScript's key order (1105).
//! What a reader gets wrong: an op reaches the seam only for a k7x file whose two texts differ.

use k7_js::number::text_hash;
use k7_sync::bases::is_k7x;
use serde_json::{Map, Value, json};

pub use k7_sync::settle::{Js, JsError, Native};

/// The k7x half, beside the relay's `Js`: what `fileOpFor` answers for a k7x change.
pub trait K7x: Js {
    /// The `kind: "k7x"` op for the change, or None when only a whole text replays.
    fn k7x_op_for(&self, file: &str, before: &str, after: &str) -> Result<Option<Value>, JsError>;
}

impl K7x for Native {
    fn k7x_op_for(&self, file: &str, before: &str, after: &str) -> Result<Option<Value>, JsError> {
        Ok(k7_sync::k7x::k7x_op_for(file, before, after))
    }
}

/// `fileOpFor(file, before, after, beforeHash)`; `before_hash` None is the default parameter, Some(None) an explicit null.
pub fn file_op_for(
    k7x: &dyn K7x,
    file: &str,
    before: Option<&str>,
    after: Option<&str>,
    before_hash: Option<Option<&str>>,
) -> Option<Value> {
    if before == after && before.is_some() {
        return None;
    }
    let hash: Option<String> = match before_hash {
        Some(h) => h.map(str::to_string),
        None => before.map(text_hash),
    };
    let was = hash.clone().map_or(Value::Null, Value::String);
    let Some(after) = after else {
        return (hash.is_some() || before.is_some())
            .then(|| json!({ "kind": "remove", "file": file, "was": was }));
    };
    if let (Some(before), true) = (before, is_k7x(file)) {
        // A seam that fails costs only bandwidth: the whole text below is always correct (1104).
        if let Ok(Some(op)) = k7x.k7x_op_for(file, before, after) {
            return Some(op);
        }
    }
    if hash.as_deref() == Some(text_hash(after).as_str()) {
        return None;
    }
    Some(json!({ "kind": "text", "file": file, "text": after, "was": was }))
}

/// `applyFileOp`: the text after `op`, a k7x op replayed through the seam (`k7_sync::settle::apply`).
pub fn apply(k7x: &dyn K7x, current: Option<&str>, op: &Value) -> Result<Option<String>, JsError> {
    k7_sync::settle::apply(k7x, current, op)
}

/// True for the error of an op kind this k7 does not know (the caller asks for an update).
pub fn needs_update(e: &JsError) -> bool {
    e.name == "NeedsUpdate"
}

/// A batch as `local` takes it: `id, replica, local, ts, label, ops`; the caller adds `resolves` and `ends` (1105).
pub fn batch(
    id: &str,
    replica: &str,
    local: u64,
    ts: u64,
    label: &str,
    ops: Vec<Value>,
) -> Map<String, Value> {
    let mut b = Map::new();
    b.insert("id".into(), id.into());
    b.insert("replica".into(), replica.into());
    b.insert("local".into(), local.into());
    b.insert("ts".into(), ts.into());
    b.insert("label".into(), label.into());
    b.insert("ops".into(), Value::Array(ops));
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Never;
    impl Js for Never {
        fn settle(
            &self,
            _: Option<String>,
            _: Value,
            _: Vec<String>,
        ) -> Result<k7_sync::settle::Settled, JsError> {
            unreachable!("native half asked the seam")
        }
        fn apply(&self, _: Option<String>, _: Value) -> Result<Option<String>, JsError> {
            unreachable!("native half asked the seam")
        }
    }
    impl K7x for Never {
        fn k7x_op_for(&self, _: &str, _: &str, _: &str) -> Result<Option<Value>, JsError> {
            unreachable!("native half asked the seam")
        }
    }

    #[test]
    fn whole_texts_for_other_files_and_new_files() {
        let op = |f, b, a| file_op_for(&Never, f, b, a, None).unwrap();
        assert_eq!(op("TODOS.md", Some("a"), Some("b"))["kind"], "text");
        assert_eq!(
            op("env.yaml", Some("a: 1\n"), Some("a: 2\n"))["kind"],
            "text"
        );
        let new = op("new.k7x", None, Some("a(1)\n"));
        assert_eq!(
            new,
            json!({ "kind": "text", "file": "new.k7x", "text": "a(1)\n", "was": null })
        );
        assert_eq!(
            new.to_string(),
            r#"{"kind":"text","file":"new.k7x","text":"a(1)\n","was":null}"#
        );
    }

    #[test]
    fn a_removal_and_no_change() {
        let rm = file_op_for(&Never, "a.k7x", Some("a(1)\n"), None, None).unwrap();
        assert_eq!(
            rm,
            json!({ "kind": "remove", "file": "a.k7x", "was": text_hash("a(1)\n") })
        );
        assert_eq!(
            file_op_for(&Never, "a.k7x", Some("a(1)\n"), Some("a(1)\n"), None),
            None
        );
        assert_eq!(file_op_for(&Never, "a.k7x", None, None, None), None);
        assert_eq!(
            file_op_for(&Never, "a.k7x", None, None, Some(Some("abc"))).unwrap()["was"],
            "abc"
        );
    }

    #[test]
    fn a_batch_is_the_typescripts_object() {
        let b = batch("b1", "r", 3, 0, "edit", vec![]);
        assert_eq!(
            Value::Object(b).to_string(),
            r#"{"id":"b1","replica":"r","local":3,"ts":0,"label":"edit","ops":[]}"#
        );
        assert!(k7_sync::protocol::is_batch(&Value::Object(batch(
            "b1",
            "r",
            3,
            0,
            "edit",
            vec![]
        ))));
        assert!(needs_update(&JsError {
            name: "NeedsUpdate".into(),
            message: String::new()
        }));
    }
}
