// doc_support/cases.rs — the negative corpus of the document: projects laid out as module cases, and what each must give.
// A reader would get wrong that an expectation is exact FILE sets, never messages, and every case names a pointer proving its feature happened.

use super::gate::Ran;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// One project of doc_negative.json.
pub struct DocCase {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub rig: Value,
    pub vsts_none: bool,
    pub renders: Vec<String>,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
}

pub fn doc_cases() -> Vec<DocCase> {
    let all: Vec<Value> = serde_json::from_str(include_str!("../doc_negative.json")).expect("doc_negative.json");
    all.iter()
        .map(|c| DocCase {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().unwrap_or(true),
            rig: c.get("rig").cloned().unwrap_or_else(|| serde_json::json!({})),
            vsts_none: c["vsts_none"].as_bool().unwrap_or(false),
            renders: c["renders"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
            files: c["files"]
                .as_object()
                .expect("files")
                .iter()
                .map(|(rel, v)| {
                    let bytes = match (
                        v.get("hex").and_then(Value::as_str),
                        v.get("repeat").and_then(Value::as_array),
                    ) {
                        (Some(hex), _) => (0..hex.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
                            .collect(),
                        (_, Some(r)) => {
                            let n = usize::try_from(r[1].as_u64().expect("count")).expect("count");
                            r[0].as_str().expect("text").repeat(n).into_bytes()
                        }
                        _ => v.as_str().expect("text").as_bytes().to_vec(),
                    };
                    (rel.clone(), bytes)
                })
                .collect(),
            expect: c["expect"].clone(),
        })
        .collect()
}

/// Writes the case's files under `base`: the project at `ws/proj`, libraries under `lib/`, the scan under `home/`.
pub fn lay(base: &Path, case: &DocCase) {
    for (rel, bytes) in &case.files {
        let path = base.join(crate::support::placed(rel));
        fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        fs::write(path, bytes).expect("write the file");
    }
    fs::create_dir_all(base.join("ws/proj")).expect("the project");
}

fn strings(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    out.sort();
    out
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

/// What the case expects of a run, as failures: the exact file sets, then the pointers into the document and the freezes.
pub fn check(case: &DocCase, ran: &Ran) -> Vec<String> {
    let mut bad = Vec::new();
    let e = &case.expect;
    for (key, got) in [("stopped", &ran.stopped), ("warned", &ran.warnings)] {
        if strings(&e[key]) != *got {
            bad.push(format!("{} {key}: {got:?}, expected {:?}", case.name, strings(&e[key])));
        }
    }
    for (part, root) in [("doc", &ran.engine), ("freezes", &ran.freezes)] {
        for (pointer, value) in e[part].as_object().into_iter().flatten() {
            let got = match pointer.strip_suffix("#len") {
                Some(p) => serde_json::json!(root.pointer(p).map_or(0, |v| {
                    v.as_array()
                        .map_or_else(|| v.as_object().map_or(0, serde_json::Map::len), Vec::len)
                })),
                None => root.pointer(pointer).cloned().unwrap_or(Value::Null),
            };
            if !same(&got, value) {
                bad.push(format!("{} {part}{pointer}: {got} is not {value}", case.name));
            }
        }
    }
    bad
}
