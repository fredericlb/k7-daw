// module_file.rs — the writers of a module file: a list on one line, a curve bound to a param, one value set or cleared.
// A reader would get wrong where an automation lives: on the alternative that is ON when it swaps the module or has its own list.

use crate::error::{Res, fail};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions, js_number};

/// Every alternative's name, the module's OWN fields first and answering to "1".
#[must_use]
pub fn alternative_names(m: &Value) -> Vec<String> {
    let mut out = vec!["1".to_owned()];
    if let Some(Value::List(list)) = m.get("alternatives") {
        for (i, a) in list.iter().enumerate() {
            out.push(match a.get("name") {
                Some(Value::Str(s)) if !s.is_empty() => s.clone(),
                _ => (i + 2).to_string(),
            });
        }
    }
    out
}

/// Which one `want` names, a NAME winning over a number and a miss falling back to the first.
#[must_use]
pub fn pick_alternative(names: &[String], want: Option<&Value>) -> usize {
    let s = match want {
        None | Some(Value::Null) => return 0,
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Num(n)) => js_number(*n),
        Some(Value::Bool(b)) => b.to_string(),
        Some(_) => "[object Object]".to_owned(),
    };
    if let Some(i) = names.iter().position(|n| *n == s) {
        return i;
    }
    match s.trim().parse::<f64>() {
        Ok(n) if n.fract() == 0.0 && n >= 1.0 && n <= names.len() as f64 => n as usize - 1,
        _ => 0,
    }
}

/// Which alternative is ON, as an index into `alternative_names`.
#[must_use]
pub fn current_alternative(m: &Value) -> usize {
    pick_alternative(&alternative_names(m), m.get("alternative"))
}

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

/// Set a list of numbers in the file, written on ONE line whatever its length.
///
/// # Errors
/// An empty path, or the write's.
pub fn set_module_list(doc: &mut Scope, path: &[PathSeg], values: &[f64]) -> Res<()> {
    if path.is_empty() {
        return fail("setModuleList: empty path");
    }
    let list = Value::List(values.iter().map(|v| Value::Num(*v)).collect());
    Ok(doc.set_in(path, &list, WriteOptions { inline: true })?)
}

/// Where the resolved node reads its bindings: the set that is on when it swaps the module or has its own list.
fn automations_home(doc: &Scope) -> Res<Vec<PathSeg>> {
    let data = doc
        .to_js()?
        .filter(|d| matches!(d, Value::Map(_)))
        .unwrap_or(Value::Map(vec![]));
    let at = current_alternative(&data);
    if at == 0 {
        return Ok(vec![]);
    }
    let home = vec![key("alternatives"), PathSeg::Index(at - 1)];
    let set = doc.get_in(&home)?;
    Ok(match set {
        Some(s @ Value::Map(_)) if s.get("extends").is_some() || s.get("automations").is_some() => home,
        _ => vec![],
    })
}

/// `String(x)` of a bound param, as the TypeScript compares it with the param's name.
pub(crate) fn text_of(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_owned(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Num(n)) => js_number(*n),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::List(l)) => l.iter().map(|x| text_of(Some(x))).collect::<Vec<_>>().join(","),
        Some(Value::Map(_)) => "[object Object]".to_owned(),
    }
}

/// Bind a curve to one of a module's params, or take it away (`None`): keyed by PARAM.
///
/// # Errors
/// What the document's writers fail with.
pub fn set_module_automation(doc: &mut Scope, param: &str, curve: Option<&str>, trigger: Option<&str>) -> Res<()> {
    let mut at_key = automations_home(doc)?;
    at_key.push(key("automations"));
    let items = match doc.get_in(&at_key)? {
        Some(Value::List(l)) => l,
        _ => vec![],
    };
    let at = items.iter().position(|it| text_of(it.get("param")) == param);
    let Some(curve) = curve else {
        let Some(at) = at else { return Ok(()) };
        // The empty list goes with the last entry: `automations([])` says something was emptied.
        if items.len() == 1 {
            doc.delete_in(&at_key, false)?;
        } else {
            let mut p = at_key.clone();
            p.push(PathSeg::Index(at));
            doc.delete_in(&p, false)?;
        }
        return Ok(());
    };
    let mut entry = vec![
        ("param".to_owned(), Value::Str(param.to_owned())),
        ("curve".to_owned(), Value::Str(curve.to_owned())),
    ];
    // Only when it is not the default: an absent trigger already means the module being automated.
    if let Some(t) = trigger.filter(|t| !t.is_empty()) {
        entry.push(("trigger".to_owned(), Value::Str(t.to_owned())));
    }
    let entry = Value::Map(entry);
    match at {
        Some(i) => {
            let mut p = at_key;
            p.push(PathSeg::Index(i));
            Ok(doc.apply_in(&p, &entry)?)
        }
        None => Ok(doc.add_in(&at_key, &entry, WriteOptions::default())?),
    }
}

/// Set one value in the file, leaving every other byte alone; `None` (null) removes the key.
///
/// # Errors
/// An empty path, or the write's.
pub fn set_module_value(doc: &mut Scope, path: &[PathSeg], value: Option<&Value>) -> Res<()> {
    if path.is_empty() {
        return fail("setModuleValue: empty path");
    }
    match value {
        None | Some(Value::Null) => doc.delete_in(path, false).map(|_| ()).map_err(Into::into),
        Some(v) => Ok(doc.set_in(path, v, WriteOptions::default())?),
    }
}
