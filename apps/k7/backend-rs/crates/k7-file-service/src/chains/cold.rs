//! chains/cold.rs — `coldChains` and the settings' view: the libraries' chains whose every step is offline, and why one cannot run.
//! What a reader gets wrong: a missing node file is a step too, so a folder chain with one is never cold (1p-D24).

use serde_json::{Value, json};
use song::templates::{StepFacts, TemplateStep};

use super::{ChainSettings, Chains};
use crate::space::Declined;

const INSTRUMENTAL: &str = "instrumental";

/// What an offline step needs: the model file resolved, and what the module declares it writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColdStep {
    pub step: String,
    pub model: String,
    pub outputs: Vec<String>,
}

/// `coldStepOf`: the step's model and outputs, or the TypeScript's refusal worded as it does (the `~/.k7/models` is a literal, 1p-D26).
pub fn cold_step_of(label: &str, f: &StepFacts) -> Result<ColdStep, String> {
    let authored = f.onnx_authored.as_deref().filter(|a| !a.is_empty());
    let Some(authored) = authored else {
        return Err(format!(
            "step '{label}' is an offline module but names no model — it needs an `onnx:` param"
        ));
    };
    let Some(model) = f.onnx.as_deref().filter(|m| !m.is_empty()) else {
        return Err(format!(
            "step '{label}' wants the model '{authored}', which is not on this machine — put it in ~/.k7/models (or the module's own assets/)"
        ));
    };
    Ok(ColdStep {
        step: label.to_owned(),
        model: model.to_owned(),
        outputs: f.outputs.clone()?,
    })
}

/// One offline step of a chain: what it declares it writes, its cold step or why it is not one, and the model it lacks.
pub(crate) struct Offline {
    pub outputs: Vec<String>,
    pub cold: Result<ColdStep, String>,
    pub missing_model: Option<(String, Option<String>)>,
}

/// The offline steps that run and the names of the template's missing nodes.
pub(crate) type Running = (Vec<Offline>, Vec<String>);

/// A library chain's steps as a cut reads them: `Some(offline steps)` when every step that runs is offline, else `None`.
/// The third value counts the template's missing nodes, which make a chain no cold one.
pub(crate) fn offline_steps(c: &Chains<'_>, i: usize) -> Result<Option<Running>, Declined> {
    let t = c.template(i)?;
    // A bypassed step is in the template and out of the signal.
    let running: Vec<&TemplateStep> = t.steps.iter().filter(|s| !s.bypass).collect();
    let mut out = Vec::new();
    for s in running.iter() {
        let f = c.facts(s)?;
        if !f.offline {
            return Ok(None);
        }
        out.push(Offline {
            outputs: f.outputs.clone().unwrap_or_default(),
            cold: cold_step_of(&s.label, &f),
            missing_model: f.missing_model.clone(),
        });
    }
    Ok(Some((out, t.missing.clone())))
}

/// A chain with a step that is not offline, as the cut words it: the template's missing nodes and unloadable offline steps,
/// the live steps counted (each taken for an engine plugin), and the offline steps' names.
pub(crate) fn split_running(
    c: &Chains<'_>,
    i: usize,
) -> Result<(Vec<String>, usize, Vec<String>), Declined> {
    let t = c.template(i)?;
    let (mut missing, mut live, mut cold) = (t.missing.clone(), 0, Vec::new());
    for s in t.steps.iter().filter(|s| !s.bypass) {
        let f = c.facts(s)?;
        if !f.offline {
            live += 1;
            continue;
        }
        match cold_step_of(&s.label, &f) {
            Ok(step) => cold.push(step.step),
            Err(e) => missing.push(e),
        }
    }
    Ok((missing, live, cold))
}

/// `{chains: [{ref, instrumental, unavailable, downloads}]}` by ref.
pub fn cold_chains(c: &Chains<'_>) -> Result<Value, Declined> {
    let mut rows = Vec::new();
    for (i, d) in c.dirs.iter().enumerate() {
        if c.unreadable(i).is_some() {
            continue;
        }
        let Some((steps, lost)) = offline_steps(c, i)? else {
            continue;
        };
        // Every step counts, the missing ones too: `offline.length !== steps` is false only when none is missing.
        if steps.is_empty() || !lost.is_empty() {
            continue;
        }
        let mut missing: Vec<String> = Vec::new();
        for s in &steps {
            if let Err(e) = &s.cold {
                missing.push(e.clone());
            }
        }
        let downloads: Vec<Value> = steps
            .iter()
            .filter_map(|s| s.missing_model.as_ref())
            .filter_map(|(file, url)| url.as_ref().map(|u| json!({ "file": file, "url": u })))
            .collect();
        rows.push(json!({
            "ref": d.ref_,
            "instrumental": steps.iter().any(|s| s.outputs.iter().any(|o| o == INSTRUMENTAL)),
            "unavailable": if missing.is_empty() { Value::Null } else { Value::from(missing.join("; ")) },
            "downloads": downloads,
        }));
    }
    Ok(Value::Array(rows))
}

/// The settings' view: the cold chains and the two chains the preferences name (1p-D12).
pub fn cold_chains_view(c: &Chains<'_>, prefs: &ChainSettings) -> Result<Value, Declined> {
    let chains = cold_chains(c)?;
    Ok(json!({
        "chains": chains,
        "stemsChain": prefs.stems,
        "instrumentalChain": prefs.instrumental,
    }))
}
