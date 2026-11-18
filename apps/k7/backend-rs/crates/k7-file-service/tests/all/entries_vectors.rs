//! The name rules of the entries against the TypeScript's own functions (1223): every case of `vectors/entries.json`
//! (made by `js/makeVectors.mts`), the rel each route would gate and the sentence for a name it refuses.

use k7_file_service::entries::guards::{in_shared_modules, refuse_yaml};
use k7_file_service::entries::plan::{folder_rel, is_plain, renamed, upload_rel};
use serde_json::Value;

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/entries.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// `{rel}` or `{error}` as the vectors spell an answer.
fn said(r: Result<String, k7_file_service::entries::EntryError>) -> Value {
    match r {
        Ok(rel) => serde_json::json!({ "rel": rel }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

#[test]
fn replays_every_vector_of_the_entries_rules() {
    let all = vectors();
    assert!(all.len() >= 1500, "{} cases", all.len());
    let mut seen = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for c in &all {
        let f = c["fn"].as_str().unwrap();
        let name = c["name"].as_str().unwrap();
        let dir = c["dir"].as_str().unwrap_or("");
        let got = match f {
            "folder" => said(folder_rel(dir, name)),
            // The vector's rename asks the second `safePath`, which is the dir of `x` (the root) joined with the cleaned name.
            "rename" => match renamed(name) {
                Ok(clean) => serde_json::json!({ "rel": clean }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            },
            "upload" => said(upload_rel(dir, name).and_then(|rel| refuse_yaml(&rel).map(|()| rel))),
            "refuse_yaml" => {
                refuse_yaml(name).map_or_else(|e| Value::from(e.to_string()), |()| Value::Null)
            }
            "in_shared_modules" => Value::from(in_shared_modules(name)),
            "is_plain" => Value::from(is_plain(name)),
            "is_dot" => Value::from(name == "." || name == ".."),
            other => panic!("a vector of a function the crate lacks: {other}"),
        };
        assert_eq!(got, c["out"], "{f} dir={dir:?} name={name:?}");
        seen.insert(f.to_string());
        names.insert(name.to_string());
    }
    assert_eq!(seen.len(), 7, "{seen:?}");
    assert!(names.len() >= 300, "{} names", names.len());
}
