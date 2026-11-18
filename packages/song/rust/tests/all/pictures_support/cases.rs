// pictures_support/cases.rs — the negative corpus of the pictures: cases laid out as a module case of negative.json, and what each expects.
// A reader would get wrong that an expectation is exact FILE sets and fault keys, never messages, plus pointers into the answer.

use super::gate::Ran;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// One project of pictures_negative.json: its files, whether the TypeScript can answer it, and what the Rust must give.
pub struct PicCase {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
}

pub fn pic_cases() -> Vec<PicCase> {
    let all: Vec<Value> =
        serde_json::from_str(include_str!("../pictures_negative.json")).expect("pictures_negative.json");
    all.iter()
        .map(|c| PicCase {
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

/// Writes the case's files under `base`, the project at `ws/proj`.
pub fn lay_case(base: &Path, case: &PicCase, placed: fn(&str) -> String) {
    for (rel, bytes) in &case.files {
        let path = base.join(placed(rel));
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

/// What the case expects of a run, as failures: the exact sets, then the pointers into the three answers.
pub fn check_case(case: &PicCase, ran: &Ran) -> Vec<String> {
    let mut bad = Vec::new();
    let e = &case.expect;
    for (key, got) in [
        ("refused", &ran.refused),
        ("warned", &ran.warned),
        ("faults", &ran.faults),
    ] {
        if strings(&e[key]) != *got {
            bad.push(format!("{} {key}: {got:?}, expected {:?}", case.name, strings(&e[key])));
        }
    }
    if strings(&e["stopped"]) != ran.stopped {
        bad.push(format!(
            "{} stopped: {:?}, expected {:?}",
            case.name,
            ran.stopped,
            strings(&e["stopped"])
        ));
    }
    if let Some(want) = e["capped"].as_bool()
        && want != ran.capped
    {
        bad.push(format!("{} capped: {}, expected {want}", case.name, ran.capped));
    }
    for part in ["video_sounds", "picture_sounds", "placed"] {
        for (key, want) in e[part].as_object().into_iter().flatten() {
            let here = &ran.json[part][key];
            for (pointer, value) in want.as_object().into_iter().flatten() {
                let got = if pointer == "#len" {
                    serde_json::json!(here.as_array().map_or(0, Vec::len))
                } else if pointer == "#has" {
                    serde_json::json!(!here.is_null())
                } else {
                    here.pointer(pointer).cloned().unwrap_or(Value::Null)
                };
                if !same(&got, value) {
                    bad.push(format!("{} {part}/{key}{pointer}: {got} is not {value}", case.name));
                }
            }
        }
    }
    bad
}
