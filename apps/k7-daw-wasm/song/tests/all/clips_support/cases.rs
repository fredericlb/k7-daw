// clips_support/cases.rs — the negative corpus of the placements: cases laid out as a module case of negative.json, and what each expects.
// A reader would get wrong that an expectation is exact FILE sets, never messages, and pointers into the dump as `resolved` does.

use super::gate::Ran;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// One project of clips_negative.json: its files, whether the TypeScript can answer it, and what the Rust must give.
pub struct ClipCase {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
}

pub fn clip_cases() -> Vec<ClipCase> {
    let all: Vec<Value> = serde_json::from_str(include_str!("../clips_negative.json")).expect("clips_negative.json");
    all.iter()
        .map(|c| ClipCase {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().expect("mem"),
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

/// Writes the case's files under `base`, the project at `ws/proj` and the workspace's `_modules/` beside it.
pub fn lay(base: &Path, case: &ClipCase) {
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

/// What the case expects of a run, as failures: the exact file sets, then the pointers into lanes, timelines and lengths.
pub fn check(case: &ClipCase, ran: &Ran) -> Vec<String> {
    let mut bad = Vec::new();
    let e = &case.expect;
    for (key, got) in [("refused", &ran.refused), ("warned", &ran.warned)] {
        if strings(&e[key]) != *got {
            bad.push(format!("{} {key}: {got:?}, expected {:?}", case.name, strings(&e[key])));
        }
    }
    for part in ["lanes", "timelines", "lengths", "shares", "patterns"] {
        for (key, want) in e[part].as_object().into_iter().flatten() {
            let here = &ran.json[part][key];
            for (pointer, value) in want.as_object().into_iter().flatten() {
                let got = if pointer == "#len" {
                    serde_json::json!(here.as_array().map_or(0, Vec::len))
                } else {
                    here.pointer(pointer).cloned().unwrap_or(Value::Null)
                };
                // `{project}` and `{workspace}` in an expected string are the folders as the run spelled them.
                let workspace = ran.project.rsplit_once('/').map_or("", |(w, _)| w);
                let want = match value.as_str() {
                    Some(t) => Value::from(t.replace("{project}", &ran.project).replace("{workspace}", workspace)),
                    None => value.clone(),
                };
                if !same(&got, &want) {
                    bad.push(format!("{} {part}/{key}{pointer}: {got} is not {value}", case.name));
                }
            }
        }
    }
    bad
}
