// doc_support/walk.rs — the emitted-document walker: no `null` anywhere and an integer at every integer pointer (decisions 302, 354).
// A reader would get wrong that a JSON number written `1.0` is not an integer here: the engine reads these as `int`, and the engine server's parse drops a float.

use serde_json::Value;

pub fn has_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.iter().any(has_null),
        Value::Object(o) => o.values().any(has_null),
        _ => false,
    }
}

/// The pointer patterns of every field the engine reads as an integer (decision 302).
pub const INTEGER_POINTERS: &[&str] = &[
    "/sampleRate",
    "/timeSigNumerator",
    "/timeSigDenominator",
    "/settings/countIn",
    "/tracks/*/inputChannels/*",
    "/tracks/*/outputChannels/*",
    "/tracks/*/inputFrom/pair",
    "/tracks/*/plugins/*/rack/*/note",
    "/tracks/*/plugins/*/tapOuts/*/note",
    "/tracks/*/plugins/*/tapOuts/*/pair",
    "/masterPlugins/*/rack/*/note",
    "/masterPlugins/*/tapOuts/*/note",
    "/masterPlugins/*/tapOuts/*/pair",
    "/tracks/*/clips/*/notes/*/key",
    "/tracks/*/clips/*/notes/*/channel",
    "/tracks/*/clips/*/notes/*/repeat",
    "/tracks/*/clips/*/notes/*/skip",
    "/tracks/*/clips/*/event/loop",
];

/// Every integer pointer of `v` that holds something other than an integer, and how many it found.
pub fn integers(v: &Value) -> (usize, Vec<String>) {
    fn walk(v: &Value, ptr: &str, found: &mut usize, bad: &mut Vec<String>) {
        if INTEGER_POINTERS.iter().any(|p| song::compare::matches(p, ptr)) {
            *found += 1;
            if !(v.is_u64() || v.is_i64()) {
                bad.push(ptr.to_owned());
            }
        }
        match v {
            Value::Array(a) => a
                .iter()
                .enumerate()
                .for_each(|(i, x)| walk(x, &format!("{ptr}/{i}"), found, bad)),
            Value::Object(o) => o.iter().for_each(|(k, x)| walk(x, &format!("{ptr}/{k}"), found, bad)),
            _ => {}
        }
    }
    let (mut found, mut bad) = (0, Vec::new());
    walk(v, "", &mut found, &mut bad);
    (found, bad)
}

/// What is wrong with an emitted document's shape, and how many integer fields it checked.
pub fn shape_failures(name: &str, doc: &Value) -> (usize, Vec<String>) {
    let mut bad = Vec::new();
    if has_null(doc) {
        bad.push(format!("{name}: a null in the emitted document"));
    }
    let (found, wrong) = integers(doc);
    if !wrong.is_empty() {
        bad.push(format!("{name}: not an integer at {wrong:?}"));
    }
    (found, bad)
}
