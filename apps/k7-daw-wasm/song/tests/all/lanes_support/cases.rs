// lanes_support/cases.rs — the negative corpus of 2e: cases laid out as module cases, and what each expects.
// A reader would get wrong that an expectation is exact FILE and KEY sets, never messages, and pointers into the dump.

use super::gate::Ran;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// One project of lanes_negative.json: its files, its rig, whether the TypeScript can answer it, and what the Rust must give.
pub struct LaneCase {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub rig: Value,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
}

pub fn lane_cases() -> Vec<LaneCase> {
    let all: Vec<Value> = serde_json::from_str(include_str!("../lanes_negative.json")).expect("lanes_negative.json");
    all.iter()
        .map(|c| LaneCase {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().expect("mem"),
            rig: c.get("rig").cloned().unwrap_or_else(|| serde_json::json!({})),
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

/// Writes the case's files under `base`, each where `support::placed` puts it.
pub fn lay(base: &Path, case: &LaneCase) {
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

/// What the case expects of a run, as failures: the exact sets, then the pointers into the dump's parts.
pub fn check(case: &LaneCase, ran: &Ran) -> Vec<String> {
    let mut bad = Vec::new();
    let e = &case.expect;
    for (key, got) in [
        ("stopped", &ran.stopped),
        ("warned", &ran.warned),
        ("faults", &ran.faults),
    ] {
        if strings(&e[key]) != *got {
            bad.push(format!("{} {key}: {got:?}, expected {:?}", case.name, strings(&e[key])));
        }
    }
    for part in [
        "lanes",
        "pictures",
        "video_tracks",
        "owners",
        "live",
        "assemblies",
        "console",
        "side",
    ] {
        for (key, want) in e[part].as_object().into_iter().flatten() {
            let here = if part == "owners" || part == "side" {
                &ran.json[part]
            } else {
                &ran.json[part][key]
            };
            // `owners` and `side` expect `{id: owner}` flat: the key is the pointer.
            if part == "owners" || part == "side" {
                let got = here.get(key).cloned().unwrap_or(Value::Null);
                if !same(&got, want) {
                    bad.push(format!("{} {part}/{key}: {got} is not {want}", case.name));
                }
                continue;
            }
            for (pointer, value) in want.as_object().into_iter().flatten() {
                // A pointer ending in `/#len` is the length of the list it names.
                let got = match pointer.strip_suffix("/#len") {
                    Some(list) => {
                        let at = here.pointer(list).and_then(Value::as_array);
                        serde_json::json!(at.map_or(0, Vec::len))
                    }
                    None => here.pointer(pointer).cloned().unwrap_or(Value::Null),
                };
                if !same(&got, value) {
                    bad.push(format!("{} {part}/{key}{pointer}: {got} is not {value}", case.name));
                }
            }
        }
    }
    if let Some(want) = e.get("assembly_issues") {
        let got = strings(&ran.json["assembly_issues"]);
        if got != strings(want) {
            bad.push(format!(
                "{} assembly_issues: {got:?}, expected {:?}",
                case.name,
                strings(want)
            ));
        }
    }
    for list in ["bus", "input", "cued", "cue_pair"] {
        if let Some(want) = e.get(list)
            && ran.json[list] != *want
        {
            bad.push(format!("{} {list}: {} is not {want}", case.name, ran.json[list]));
        }
    }
    if let Some(want) = e.get("tap")
        && ran.json["tap"] != *want
    {
        bad.push(format!("{} tap: {} is not {want}", case.name, ran.json["tap"]));
    }
    bad
}
