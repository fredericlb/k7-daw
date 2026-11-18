// templates/facts.rs — `facts_of`: a template's step resolved against the vocabulary, and what a cut asks of it.
// A reader would get wrong that nothing is judged here: `outputs` carries the TypeScript's refusal as a value, a model is only named.

use super::{Refused, TemplateStep};
use crate::files::Severity;
use crate::modules::files::FILE_PARAMS;
use crate::modules::{LoadedModule, ModuleKind, Vocabulary};
use crate::resolve::{Resolved, resolve_map};
use crate::roots::Roots;
use crate::source::Source;
use indexmap::IndexMap;

/// The four sources a separation writes, in the order a model returns them, and the one output that is not a stem.
const STEMS: [&str; 4] = ["drums", "bass", "other", "vocals"];
const INSTRUMENTAL: &str = "instrumental";

/// The plugin format of a module whose outputs are produced offline, onto disk.
const OFFLINE_TYPE: &str = "offline";

/// The key a step is resolved under: no ref can spell it, so the vocabulary never answers to it.
const KEY: &str = "\0step";

/// What a step is once resolved: the questions `isOfflineModule`, `coldStepOf`, `outputsOf` and `missingModel` ask.
#[derive(Clone, Debug, PartialEq)]
pub struct StepFacts {
    pub offline: bool,
    /// The module's own `cold: true`, which a live module may carry too.
    pub cold: bool,
    pub outputs: Result<Vec<String>, String>,
    /// The `onnx` param as authored, and the file it resolved to; none when no folder or cache held it.
    pub onnx_authored: Option<String>,
    pub onnx: Option<String>,
    /// A model named and not on this machine: the file as authored and the `model_url` when the module gives one.
    pub missing_model: Option<(String, Option<String>)>,
}

fn outputs_of(label: &str, m: &LoadedModule) -> Result<Vec<String>, String> {
    let named: Vec<&str> = m.outputs().iter().map(|p| p.name.as_str()).collect();
    let known = |n: &str| n == INSTRUMENTAL || STEMS.contains(&n);
    let kept: Vec<String> = named.iter().filter(|n| known(n)).map(|n| (*n).to_owned()).collect();
    // A module that names none of them inherited its port from its type, and separates the four.
    if kept.is_empty() {
        return Ok(STEMS.iter().map(|s| (*s).to_owned()).collect());
    }
    let strangers: Vec<&str> = named.iter().filter(|n| !known(n)).copied().collect();
    if strangers.is_empty() {
        return Ok(kept);
    }
    Err(format!(
        "step '{label}' names outputs a separation can write and ones it cannot ({}) — an output is one of the stems, or the instrumental",
        strangers.join(", ")
    ))
}

fn missing_model(m: &LoadedModule) -> Option<(String, Option<String>)> {
    let assets = m.assets.as_ref()?;
    for field in FILE_PARAMS {
        let Some(file) = assets.get(field).filter(|f| !f.is_empty()) else {
            continue;
        };
        let have = if field == "model" { &m.model } else { &m.onnx };
        if have.as_deref().is_some_and(|h| !h.is_empty()) {
            continue;
        }
        let url = m
            .authored
            .model_url
            .as_ref()
            .map(|u| u.0.clone())
            .filter(|u| !u.is_empty());
        return Some((file.clone(), url));
    }
    None
}

/// The step resolved against `vocab`; a resolution that errs or reaches a limit refuses, never a partial answer.
pub fn facts_of(
    source: &dyn Source,
    roots: &Roots,
    vocab: &Vocabulary,
    step: &TemplateStep,
) -> Result<StepFacts, Refused> {
    let mut bare = LoadedModule::bare(ModuleKind::Plugin, "", &step.name);
    bare.category = None;
    bare.kind = None;
    bare.authored = step.module.clone();
    let one: IndexMap<String, LoadedModule> = std::iter::once((KEY.to_owned(), bare)).collect();
    let done = resolve_map(source, roots, &one, &vocab.modules)?;
    if done.capped {
        return Err(Refused::Limit);
    }
    if let Some(f) = done.findings.iter().find(|f| f.severity == Severity::Error) {
        return Err(Refused::Native(f.message.clone()));
    }
    // A fold the resolver cut at its depth is a limit the TypeScript has no counterpart of.
    if done
        .findings
        .iter()
        .any(|f| f.message.starts_with("extends goes more than"))
    {
        return Err(Refused::Limit);
    }
    let Some(m): Option<&Resolved> = done.modules.get(KEY) else {
        return Err(Refused::Limit);
    };
    let plugin = m.authored.plugin.as_ref().and_then(|p| p.plugin_type.as_deref());
    Ok(StepFacts {
        offline: plugin.is_some_and(|t| t.to_lowercase() == OFFLINE_TYPE),
        cold: m.authored.cold == Some(true),
        outputs: outputs_of(&step.label, m),
        onnx_authored: m.assets.as_ref().and_then(|a| a.get("onnx")).cloned(),
        onnx: m.onnx.clone(),
        missing_model: missing_model(m),
    })
}

#[cfg(test)]
mod tests {
    use super::{LoadedModule, ModuleKind, missing_model, outputs_of};
    use crate::modules::Port;
    use serde_json::Value;

    // 1p-D28: the TypeScript's own answers over hand-built modules that carry `C:/` paths, none of them on a disk.
    #[test]
    fn modules_spelled_on_a_c_drive_answer_as_the_typescript_did() {
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../apps/k7/backend-rs/crates/k7-file-service")
            .join("tests/vectors")
            .join("chains.json");
        let all: Value = serde_json::from_str(&std::fs::read_to_string(at).expect("chains.json")).expect("json");
        let rows = all["windows"]["modules"].as_array().expect("rows");
        assert!(rows.len() >= 5);
        for row in rows {
            let m = &row["module"];
            let mut got = LoadedModule::bare(ModuleKind::Plugin, "", "s");
            let text = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_owned);
            got.onnx = text("onnx");
            got.assets = m["assets"].as_object().map(|a| {
                a.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                    .collect()
            });
            got.authored.model_url = text("model_url").and_then(|u| serde_json::from_value(Value::from(u)).ok());
            got.authored.outputs = m["outputs"].as_array().map(|o| {
                o.iter()
                    .map(|p| Port {
                        name: p["name"].as_str().unwrap_or("").to_owned(),
                        kind: crate::modules::PortType::Audio,
                        mandatory: None,
                    })
                    .collect()
            });
            let outputs = match outputs_of("s", &got) {
                Ok(v) => serde_json::json!(v),
                Err(e) => serde_json::json!({ "error": e }),
            };
            assert_eq!(row["outputs"], outputs, "{m}");
            let missing = missing_model(&got).map(|(file, url)| match url {
                Some(u) => serde_json::json!({ "file": file, "url": u }),
                None => serde_json::json!({ "file": file }),
            });
            assert_eq!(row["missing"], missing.unwrap_or(Value::Null), "{m}");
        }
    }
}
