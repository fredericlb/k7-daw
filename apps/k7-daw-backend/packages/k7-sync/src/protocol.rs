//! protocol.rs — the shape of a batch of ops and the log's version (`ops.ts`'s `isBatch`): a batch checked field by
//! field before a room orders it, and the op kinds this build applies.
//! What a reader gets wrong: an op's path is checked here, never trusted from the payload.

use serde_json::Value;

use crate::path::sync_path_error;

/// The version stamped on every log entry and saved room state written here.
pub const LOG_V: f64 = 1.0;

/// Every op kind this build applies.
pub const OP_KINDS: [&str; 3] = ["k7x", "text", "remove"];
/// Op kinds only an older k7 sent, never applied here.
pub const RETIRED_OP_KINDS: [&str; 1] = ["yaml"];

pub fn is_known_op_kind(kind: Option<&Value>) -> bool {
    kind.and_then(Value::as_str)
        .is_some_and(|k| OP_KINDS.contains(&k))
}

pub fn is_retired(kind: Option<&Value>) -> bool {
    kind.and_then(Value::as_str)
        .is_some_and(|k| RETIRED_OP_KINDS.contains(&k))
}

/// `String(v)` of a JSON value, `undefined` for an absent one.
pub fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => k7_js::number::to_string(n.as_f64().unwrap_or(f64::NAN)),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::Null => String::new(),
                x => js_string(Some(x)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

/// `undefined`, `null` or a string.
fn opt_text(v: Option<&Value>) -> bool {
    matches!(v, None | Some(Value::Null) | Some(Value::String(_)))
}

fn is_path_op(v: &Value) -> bool {
    let Some(o) = v.as_object() else { return false };
    let Some(path) = o.get("path").and_then(Value::as_array) else {
        return false;
    };
    if !opt_text(o.get("was")) || !path.iter().all(Value::is_string) {
        return false;
    }
    match o.get("op").and_then(Value::as_str) {
        Some("delete") => true,
        Some("set") => o.get("k7x").is_some_and(Value::is_string) && opt_text(o.get("after")),
        _ => false,
    }
}

/// One file's op, its path confined to the workspace (`isFileOp`).
pub fn is_file_op(v: &Value) -> bool {
    let Some(o) = v.as_object() else { return false };
    let Some(file) = o.get("file").and_then(Value::as_str) else {
        return false;
    };
    if sync_path_error(file).is_some() {
        return false;
    }
    match o.get("kind").and_then(Value::as_str) {
        Some("k7x") => o
            .get("ops")
            .and_then(Value::as_array)
            .is_some_and(|ops| ops.iter().all(is_path_op)),
        Some("text") => o.get("text").is_some_and(Value::is_string) && opt_text(o.get("was")),
        Some("remove") => opt_text(o.get("was")),
        _ => false,
    }
}

/// A batch whose every field has the type `ops.ts` gives it (`isBatch`).
pub fn is_batch(v: &Value) -> bool {
    let Some(o) = v.as_object() else { return false };
    let is = |k: &str, f: fn(&Value) -> bool| o.get(k).is_some_and(f);
    is("id", Value::is_string)
        && is("replica", Value::is_string)
        && is("local", Value::is_number)
        && is("ts", Value::is_number)
        && is("label", Value::is_string)
        && o.get("ops")
            .and_then(Value::as_array)
            .is_some_and(|ops| ops.iter().all(is_file_op))
        && match o.get("resolves") {
            None => true,
            Some(Value::Array(ids)) => ids.iter().all(Value::is_string),
            Some(_) => false,
        }
        && matches!(o.get("ends"), None | Some(Value::Bool(true)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches() {
        let ok = r#"{"id":"b","replica":"r","local":1,"ts":2,"label":"l","ops":[{"kind":"text","file":"a","text":"x","was":null},{"kind":"remove","file":"b"},{"kind":"k7x","file":"c.k7x","ops":[{"op":"set","path":["a"],"k7x":"1","after":null},{"op":"delete","path":[]}]}],"resolves":["1.0"],"ends":true}"#;
        assert!(is_batch(&serde_json::from_str(ok).unwrap()));
        for bad in [
            r#"{"id":"b","replica":"r","local":1,"ts":2,"label":"l","ops":[{"kind":"text","file":"../a","text":"x"}]}"#,
            r#"{"id":"b","replica":"r","local":1,"ts":2,"label":"l","ops":[],"ends":false}"#,
            r#"{"id":"b","replica":"r","local":1,"ts":2,"label":"l","ops":[],"resolves":null}"#,
            r#"{"id":"b","replica":"r","local":"1","ts":2,"label":"l","ops":[]}"#,
            r#"{"id":"b","replica":"r","local":1,"ts":2,"label":"l","ops":[{"kind":"k7x","file":"c","ops":[{"op":"set","path":[1],"k7x":"1"}]}]}"#,
        ] {
            assert!(!is_batch(&serde_json::from_str(bad).unwrap()), "{bad}");
        }
    }
}
