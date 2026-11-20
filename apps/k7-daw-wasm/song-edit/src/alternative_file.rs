// alternative_file.rs — switching and adding a module's attribute sets by editing its k7x in place, over any scope.
// A reader would get wrong where a set's fields live: the module's root while the first is on, else its `alternatives` entry.

use crate::error::{Res, fail};
use crate::module_file::{alternative_names, current_alternative, pick_alternative};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};

/// The keys an alternative may carry beyond `name` and `extends`: the module fields of the schema.
const SET_KEYS: [&str; 39] = [
    "plugin",
    "io",
    "inputs",
    "outputs",
    "type",
    "shader",
    "shape",
    "font",
    "align",
    "background",
    "shadow",
    "params",
    "rack",
    "tap_outs",
    "kit",
    "cold",
    "model_url",
    "channel",
    "camera",
    "device",
    "frame",
    "source",
    "monitor",
    "gain",
    "mix",
    "input_volume",
    "output_volume",
    "base",
    "ui",
    "automations",
    "content",
    "events",
    "color",
    "disabled",
    "tracks",
    "strudel",
    "video_source",
    "lut",
    "frozen",
];
const MORE_KEYS: [&str; 2] = ["engine", "engines"];

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn data_of(doc: &Scope) -> Res<Value> {
    Ok(doc.to_js()?.unwrap_or(Value::Map(vec![])))
}

/// Switch which set is on: one value rewritten, and back to the first clears the key.
///
/// # Errors
/// A name no set answers to.
pub fn set_alternative(doc: &mut Scope, name: &str) -> Res<()> {
    let names = alternative_names(&data_of(doc)?);
    let at = pick_alternative(&names, Some(&Value::Str(name.to_owned())));
    if names[at] != name {
        return fail(format!("no alternative \"{name}\""));
    }
    if at == 0 {
        doc.delete_in(&[key("alternative")], false)?;
    } else {
        doc.set_in(
            &[key("alternative")],
            &Value::Str(name.to_owned()),
            WriteOptions::default(),
        )?;
    }
    Ok(())
}

/// What a switch does to the params when that is all it does.
#[derive(Debug, PartialEq)]
pub struct ParamsSwitch {
    pub params: Vec<(String, f64)>,
    pub inherited: Vec<String>,
}

fn stated(raw: &Value, at: usize) -> Option<Vec<(String, f64)>> {
    if at == 0 {
        return Some(vec![]);
    }
    let set = match raw.get("alternatives") {
        Some(Value::List(l)) => l.get(at - 1).cloned().unwrap_or(Value::Map(vec![])),
        _ => Value::Map(vec![]),
    };
    let Value::Map(fields) = set else { return Some(vec![]) };
    if fields.iter().any(|(k, _)| k != "name" && k != "params") {
        return None;
    }
    match fields.iter().find(|(k, _)| k == "params").map(|(_, v)| v) {
        None | Some(Value::Null) => Some(vec![]),
        Some(Value::Map(m)) => m
            .iter()
            .map(|(k, v)| match v {
                Value::Num(n) => Some((k.clone(), *n)),
                _ => None,
            })
            .collect(),
        Some(_) => None,
    }
}

/// Read off the module's data alone, so a running engine can take the switch as values.
#[must_use]
pub fn params_switch(raw: &Value, name: &str) -> Option<ParamsSwitch> {
    let names = alternative_names(raw);
    let from = stated(raw, current_alternative(raw))?;
    let to = stated(raw, pick_alternative(&names, Some(&Value::Str(name.to_owned()))))?;
    let own = raw.get("params");
    let mut params = to.clone();
    let mut inherited = Vec::new();
    for (k, _) in &from {
        if to.iter().any(|(t, _)| t == k) {
            continue;
        }
        match own.and_then(|o| o.get(k)) {
            Some(Value::Num(n)) => match params.iter_mut().find(|(p, _)| p == k) {
                Some(slot) => slot.1 = *n,
                None => params.push((k.clone(), *n)),
            },
            None => inherited.push(k.clone()),
            Some(_) => return None,
        }
    }
    Some(ParamsSwitch { params, inherited })
}

fn set_path(doc: &Scope) -> Res<Vec<PathSeg>> {
    let at = current_alternative(&data_of(doc)?);
    Ok(if at == 0 {
        vec![]
    } else {
        vec![key("alternatives"), PathSeg::Index(at - 1)]
    })
}

fn params_path(doc: &Scope, engine: Option<&str>) -> Res<Vec<PathSeg>> {
    let mut p = set_path(doc)?;
    if let Some(e) = engine.filter(|e| !e.is_empty()) {
        p.extend([key("engines"), key(e)]);
    }
    p.push(key("params"));
    Ok(p)
}

/// Set one param on the set that is on, under `engines.<engine>` when the module runs one; `None` removes it.
///
/// # Errors
/// What the document's writers fail with.
pub fn set_alternative_param(doc: &mut Scope, name: &str, value: Option<&Value>, engine: Option<&str>) -> Res<()> {
    let mut path = params_path(doc, engine)?;
    path.push(key(name));
    match value {
        None | Some(Value::Null) => doc.delete_in(&path, false).map(|_| ())?,
        Some(v) => doc.set_in(&path, v, WriteOptions::default())?,
    }
    Ok(())
}

/// Remove every number the set that is on states; a file param stays.
///
/// # Errors
/// What the document's writers fail with.
pub fn clear_alternative_params(doc: &mut Scope, engine: Option<&str>) -> Res<()> {
    let path = params_path(doc, engine)?;
    let Some(Value::Map(params)) = doc.get_in(&path)? else {
        return Ok(());
    };
    let numbers: Vec<&String> = params
        .iter()
        .filter(|(_, v)| matches!(v, Value::Num(_)))
        .map(|(k, _)| k)
        .collect();
    if numbers.is_empty() {
        return Ok(());
    }
    for k in numbers {
        let mut p = path.clone();
        p.push(key(k));
        doc.delete_in(&p, false)?;
    }
    if matches!(doc.get_in(&path)?, Some(Value::Map(m)) if m.is_empty()) {
        doc.delete_in(&path, false)?;
    }
    Ok(())
}

/// Set one field on the set that is on.
///
/// # Errors
/// What the document's writers fail with.
pub fn set_alternative_field(doc: &mut Scope, field: &str, value: &str) -> Res<()> {
    let mut p = set_path(doc)?;
    p.push(key(field));
    Ok(doc.set_in(&p, &Value::Str(value.to_owned()), WriteOptions::default())?)
}

/// The fields a copy starts from: only what the current set states, never the merged result.
fn copied_fields(raw: &Value) -> Vec<(String, Value)> {
    let at = current_alternative(raw);
    if at > 0 {
        return match raw.get("alternatives") {
            Some(Value::List(l)) => match l.get(at - 1) {
                Some(Value::Map(m)) => m.iter().filter(|(k, _)| k != "name").cloned().collect(),
                _ => vec![],
            },
            _ => vec![],
        };
    }
    match raw {
        Value::Map(m) => m
            .iter()
            .filter(|(k, _)| k != "extends" && k != "name")
            .filter(|(k, _)| SET_KEYS.contains(&k.as_str()) || MORE_KEYS.contains(&k.as_str()))
            .cloned()
            .collect(),
        _ => vec![],
    }
}

/// Add a set and switch to it, answering the name it got: empty by default.
///
/// # Errors
/// A name the module already has.
pub fn add_alternative(doc: &mut Scope, name: Option<&str>, copy: bool, blank_events: bool) -> Res<String> {
    let raw = data_of(doc)?;
    let names = alternative_names(&raw);
    let chosen = name.map_or_else(|| (names.len() + 1).to_string(), str::to_owned);
    if names.contains(&chosen) {
        return fail(format!("this module already has an alternative \"{chosen}\""));
    }
    let mut entry: Vec<(String, Value)> = Vec::new();
    if let Some(n) = name {
        entry.push(("name".to_owned(), Value::Str(n.to_owned())));
    }
    if copy {
        for (k, v) in copied_fields(&raw) {
            match entry.iter_mut().find(|(e, _)| *e == k) {
                Some(slot) => slot.1 = v,
                None => entry.push((k, v)),
            }
        }
    } else if blank_events {
        entry.push(("events".to_owned(), Value::List(vec![])));
    }
    let entry = Value::Map(entry);
    if matches!(raw.get("alternatives"), Some(Value::List(_))) {
        doc.add_in(&[key("alternatives")], &entry, WriteOptions::default())?;
    } else {
        doc.set_in(
            &[key("alternatives")],
            &Value::List(vec![entry]),
            WriteOptions::default(),
        )?;
    }
    doc.set_in(
        &[key("alternative")],
        &Value::Str(chosen.clone()),
        WriteOptions::default(),
    )?;
    Ok(chosen)
}
