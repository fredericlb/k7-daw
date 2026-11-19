// doc_golden.rs — every golden `engine` document decodes into the Rust ProjectDoc, unknown fields denied, and re-encodes equal.
// A reader would get wrong that this is the decode half only: the shape walker also runs over every compiled document (doc_negative.rs).

use serde_json::Value;
use song::compare::{Rules, compare};
use song::doc::ProjectDoc;
use std::path::{Path, PathBuf};

use crate::doc_support;

use doc_support::{has_null, integers};

fn files_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("a folder")
        .map(|e| e.expect("an entry").path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files_in(&p, out);
        } else if p.extension().is_some_and(|e| e == "json") {
            out.push(p);
        }
    }
}

fn golden() -> Vec<(PathBuf, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../apps/k7/backend-rs/fixtures/golden");
    let mut paths = Vec::new();
    files_in(&root, &mut paths);
    paths
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).expect("a fixture");
            let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (p, v)
        })
        .collect()
}

#[test]
fn every_golden_engine_document_decodes_strictly_and_re_encodes_equal() {
    let all = golden();
    let (mut docs, mut nulls, mut found) = (0, 0, 0);
    let mut failures = Vec::new();
    for (path, file) in &all {
        let engine = &file["engine"];
        if engine.is_null() {
            nulls += 1;
            continue;
        }
        let name = path.display();
        match serde_json::from_value::<ProjectDoc>(engine.clone()) {
            Err(e) => failures.push(format!("{name}: {e}")),
            Ok(doc) => {
                docs += 1;
                let back = serde_json::to_value(&doc).expect("a value");
                let diff = compare(engine, &back, &Rules::default());
                if !diff.is_empty() {
                    failures.push(format!("{name}: {:?}", diff.first()));
                }
                if has_null(&back) {
                    failures.push(format!("{name}: a null in the re-encoded document"));
                }
                let (n, bad) = integers(&back);
                found += n;
                if !bad.is_empty() {
                    failures.push(format!("{name}: not an integer at {bad:?}"));
                }
                if !doc.finite() {
                    failures.push(format!("{name}: not finite"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!((all.len(), docs, nulls), (174, 172, 2));
    assert!(found > 1000, "the integer walker saw {found} fields");
}

#[test]
fn a_key_added_to_a_document_is_refused_at_every_level() {
    let all = golden();
    let engine = all
        .iter()
        .map(|(_, f)| &f["engine"])
        .find(|e| !e.is_null() && e["tracks"][0]["plugins"][0].is_object());
    let engine = engine.expect("a document with a plugin");
    for pointer in ["", "/tracks/0", "/tracks/0/plugins/0", "/settings"] {
        let mut v = engine.clone();
        v.pointer_mut(pointer).expect("a node")["extra"] = Value::Bool(true);
        assert!(serde_json::from_value::<ProjectDoc>(v).is_err(), "{pointer}");
    }
}

#[test]
fn a_float_where_the_engine_wants_an_integer_is_refused() {
    let mut doc = serde_json::to_value(ProjectDoc::empty()).expect("a value");
    doc["sampleRate"] = serde_json::json!(48000.5);
    assert!(serde_json::from_value::<ProjectDoc>(doc).is_err());
}
