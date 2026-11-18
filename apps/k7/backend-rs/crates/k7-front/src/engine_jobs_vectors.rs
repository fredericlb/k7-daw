//! engine_jobs_vectors.rs — the engine jobs' decisions against `fixtures/engine-jobs-vectors.json`,
//! node's answers kept as committed: zod's words for every body.

use serde_json::Value;

use crate::routes::engine_reach::engine_jobs::{footswitch_issues, save_issues};

fn vectors() -> Value {
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/engine-jobs-vectors.json");
    serde_json::from_slice(&std::fs::read(at).expect("the vectors")).expect("JSON")
}

fn list<'a>(v: &'a Value, k: &str) -> &'a Vec<Value> {
    v[k].as_array().unwrap_or_else(|| panic!("{k}"))
}

#[test]
fn a_body_is_refused_in_zods_words() {
    let v = vectors();
    let bodies = &v["bodies"];
    let check = |k: &str, verdict: &dyn Fn(&Value) -> Option<String>| {
        for c in list(bodies, k) {
            assert_eq!(
                verdict(&c["body"]).map_or(Value::Null, Value::String),
                c["error"],
                "{k} {}",
                c["body"]
            );
        }
    };
    check("save", &save_issues);
    check("footswitch", &footswitch_issues);
}
