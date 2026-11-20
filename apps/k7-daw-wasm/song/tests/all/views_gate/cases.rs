// views_gate/cases.rs — the negative corpus of the views: cases laid out as module cases, and what each expects of the views.
// A reader would get wrong that an expectation is by file and key, never by sentence, and that `ts: true` hands the case to the TypeScript.

use serde_json::Value;
use song::lanes::rig::Rig;
use std::fs;
use std::path::Path;

pub struct ViewCase {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub vsts_none: bool,
    pub rig_json: Value,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
}

impl ViewCase {
    pub fn rig(&self) -> Rig {
        Rig::from_json(&self.rig_json)
    }
}

/// The cases of this corpus, then the TypeScript-answerable cases of the document corpus that need no render (named `doc-…`).
pub fn cases() -> Vec<ViewCase> {
    let own: Vec<Value> = serde_json::from_str(include_str!("../views_negative.json")).expect("views_negative.json");
    let doc: Vec<Value> = serde_json::from_str(include_str!("../doc_negative.json")).expect("doc_negative.json");
    let doc = doc
        .into_iter()
        .filter(|c| c["ts"] == true && c.get("renders").is_none())
        .map(|mut c| {
            c["name"] = Value::String(format!("doc-{}", c["name"].as_str().unwrap_or("")));
            c["category"] = Value::String("doc".into());
            c["expect"] = Value::Null;
            c
        });
    let all: Vec<Value> = own.into_iter().chain(doc).collect();
    all.iter()
        .map(|c| ViewCase {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().unwrap_or(true),
            vsts_none: c["vsts_none"].as_bool().unwrap_or(false),
            rig_json: c.get("rig").cloned().unwrap_or_else(|| serde_json::json!({})),
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
pub fn lay(base: &Path, case: &ViewCase) {
    for (rel, bytes) in &case.files {
        let path = base.join(crate::support::placed(rel));
        fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        fs::write(path, bytes).expect("write the file");
    }
    fs::create_dir_all(base.join("ws/proj")).expect("the project");
}
