//! chains/frozen.rs — `listFrozenChains`: the account's chains, then the libraries' cold ones, each step as the page lists it.
//! What a reader gets wrong: a bypassed step lists `bypass: true` and any other step has no `bypass` key at all.

use serde_json::{Map, Value, json};
use song::modules::{Module, ParamValue};
use song::routing::ChainInput;
use song::templates::TemplateStep;

use super::live::refusals;
use super::{Chains, number};
use crate::libraries::ACCOUNT_LIBRARY;
use crate::space::Declined;

/// What a step is a use of: the module it extends, else the plugin it names itself.
fn module_label(m: &Module) -> String {
    match (&m.extends, m.plugin.as_ref().and_then(|p| p.name.as_ref())) {
        (Some(e), _) => e.clone(),
        (None, Some(n)) => n.clone(),
        _ => String::new(),
    }
}

fn param(v: &ParamValue) -> Value {
    match v {
        ParamValue::Number(n) => number(*n),
        ParamValue::Text(t) => Value::from(t.as_str()),
    }
}

/// A step in the list's terms: the engine's values when it names one and sets any, else the module's own.
fn step_of(s: &TemplateStep) -> Value {
    let m = &s.module;
    let engine = m.engine.as_ref().map(|e| e.0.as_str());
    let engine_params = engine
        .and_then(|e| m.engines.as_ref()?.0.get(e))
        .and_then(|set| set.params.as_ref());
    let mut params = Map::new();
    for (k, v) in engine_params.or(m.params.as_ref()).into_iter().flatten() {
        params.insert(k.clone(), param(v));
    }
    let mut out = Map::new();
    out.insert("id".into(), s.name.clone().into());
    out.insert("module".into(), module_label(m).into());
    if let Some(e) = engine {
        out.insert("engine".into(), e.into());
    }
    out.insert("params".into(), Value::Object(params));
    if s.bypass {
        out.insert("bypass".into(), true.into());
    }
    Value::Object(out)
}

fn input_of(i: &ChainInput) -> Value {
    let mut out = json!({ "channel": i.channel });
    if let Some(m) = &i.monitor {
        out["monitor"] = json!(m);
    }
    out
}

/// `{chains: [...]}`: the account's own sorted by ref, then the libraries' cold ones sorted by ref.
pub fn frozen_chains(c: &Chains<'_>) -> Result<Value, Declined> {
    let live = refusals(c)?;
    let (mut mine, mut cold) = (Vec::new(), Vec::new());
    for (i, d) in c.dirs.iter().enumerate() {
        if let Some(m) = c.unreadable(i) {
            // A chain that does not read is listed with its complaint, no steps and no cold flag.
            if d.library == ACCOUNT_LIBRARY {
                mine.push(json!({
                    "ref": d.ref_, "name": d.path, "mine": true,
                    "liveRefusal": live[i].clone().map_or(Value::Null, Value::from),
                    "cold": false, "steps": [], "unreadable": m,
                }));
            }
            continue;
        }
        let t = c.template(i)?;
        // Every step must be offline for the chain to be cold; the first that is not ends the look.
        let mut all_offline = !t.steps.is_empty();
        for s in &t.steps {
            if !all_offline {
                break;
            }
            all_offline = c.facts(s)?.offline;
        }
        let is_mine = d.library == ACCOUNT_LIBRARY;
        let mut row = Map::new();
        row.insert("ref".into(), d.ref_.clone().into());
        row.insert("name".into(), d.path.clone().into());
        row.insert("mine".into(), is_mine.into());
        row.insert(
            "liveRefusal".into(),
            live[i].clone().map_or(Value::Null, Value::from),
        );
        row.insert("cold".into(), all_offline.into());
        row.insert(
            "steps".into(),
            Value::Array(t.steps.iter().map(step_of).collect()),
        );
        if let Some(input) = &t.input {
            row.insert("input".into(), input_of(input));
        }
        if is_mine {
            mine.push(Value::Object(row));
        } else if all_offline {
            cold.push(Value::Object(row));
        }
    }
    mine.extend(cold);
    Ok(json!({ "chains": mine }))
}
