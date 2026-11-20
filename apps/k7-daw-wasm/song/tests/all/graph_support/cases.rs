// graph_support/cases.rs — the negative corpus of 2k: cases laid out as module cases, and what each expects.
// A reader would get wrong that an expectation is exact FILE, fault and issue SETS and JSON pointers, never a sentence's words.

use super::gate::{Ran, Via};
use serde_json::Value;
use song::Roots;
use song::graph::Handed;
use std::fs;
use std::path::Path;

/// One project of graph_negative.json: its files, rig and handed values, whether the TypeScript can answer it, and what the Rust must give.
pub struct GraphCase {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub rig: Value,
    /// Given values for the graph; a case without them is drawn over the Rust's own compile.
    pub handed: Option<Handed>,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
    /// `ready`: the automatic renders are made before the case is drawn; `waiting`: they are not. Absent: no render cache.
    pub renders: Option<String>,
}

pub fn graph_cases() -> Vec<GraphCase> {
    let all: Vec<Value> = serde_json::from_str(include_str!("../graph_negative.json")).expect("graph_negative.json");
    all.iter()
        .map(|c| GraphCase {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().expect("mem"),
            rig: c.get("rig").cloned().unwrap_or_else(|| serde_json::json!({})),
            handed: c
                .get("handed")
                .map(|h| serde_json::from_value(h.clone()).expect("handed")),
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
            renders: c.get("renders").and_then(Value::as_str).map(str::to_owned),
        })
        .collect()
}

impl GraphCase {
    /// The roots of a case the TypeScript does not answer: a render cache under `home/` when the case names one.
    pub fn roots(&self, base: &Path) -> Roots {
        let mut roots = crate::support::case_roots(base, None);
        if self.renders.is_some() {
            roots.renders = Some(crate::support::spell(&base.join("home/.renders")));
        }
        roots
    }

    /// Given values when the case states them, else the compile's own.
    pub fn via(&self) -> Via<'_> {
        self.handed.as_ref().map_or(Via::Compile, Via::Given)
    }
}

/// Writes the case's files under `base`, each where `support::placed` puts it.
pub fn lay(base: &Path, case: &GraphCase) {
    for (rel, bytes) in &case.files {
        let path = base.join(crate::support::placed(rel));
        fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        fs::write(path, bytes).expect("write the file");
    }
    fs::create_dir_all(base.join("ws/proj")).expect("the project");
    // The TypeScript's dump compiles the folder whatever it holds; a folder project of the hosts holds its root file (811).
    let root = base.join("ws/proj/project.k7x");
    if !root.exists() {
        fs::write(root, "").expect("the root file");
    }
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

/// The differences between what a case expects and what the run gave.
pub fn check(case: &GraphCase, ran: &Ran) -> Vec<String> {
    let mut out = Vec::new();
    let e = &case.expect;
    let name = &case.name;
    let refused = e.get("refused");
    if refused.is_some() != ran.refused {
        out.push(format!(
            "{name}: refused is {}, expected {}",
            ran.refused,
            refused.is_some()
        ));
        return out;
    }
    if let Some(r) = refused {
        if let Some(files) = r.get("files")
            && strings(files) != ran.refused_files
        {
            out.push(format!(
                "{name}: refused files {:?}, expected {:?}",
                ran.refused_files,
                strings(files)
            ));
        }
        if let Some(faults) = r.get("faults")
            && strings(faults) != ran.refused_faults
        {
            out.push(format!(
                "{name}: refused faults {:?}, expected {:?}",
                ran.refused_faults,
                strings(faults)
            ));
        }
        return out;
    }
    if let Some(files) = e.get("findings")
        && strings(files) != ran.finding_files()
    {
        out.push(format!(
            "{name}: findings {:?}, expected {:?}",
            ran.finding_files(),
            strings(files)
        ));
    }
    if let Some(issues) = e.get("issues") {
        let mut got = ran.issues.clone();
        got.sort();
        if strings(issues) != got {
            out.push(format!("{name}: issues {got:?}, expected {:?}", strings(issues)));
        }
    }
    for (ptr, want) in e.get("pointers").and_then(Value::as_object).into_iter().flatten() {
        match ran.json.pointer(ptr) {
            Some(got) if same(got, want) => {}
            got => out.push(format!("{name}: {ptr} is {got:?}, expected {want}")),
        }
    }
    for ptr in strings(e.get("absent").unwrap_or(&Value::Null)) {
        if ran.json.pointer(&ptr).is_some() {
            out.push(format!("{name}: {ptr} is there, expected absent"));
        }
    }
    if e.get("capped")
        .and_then(Value::as_bool)
        .is_some_and(|c| c != ran.capped)
    {
        out.push(format!("{name}: capped is {}", ran.capped));
    }
    out
}
