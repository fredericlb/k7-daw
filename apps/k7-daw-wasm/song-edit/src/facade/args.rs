// args.rs — the helpers every arm shares: a positional JSON argument read as a string, a number, a value, and an answer written back.
// A reader would get wrong that a missing argument is `null`, not a panic: the TypeScript's `undefined` is a value an arm reads.

use super::{Failure, Out};
use crate::Error;
use indexmap::IndexMap;
use k7x::edit::{PathSeg, Value};
use serde_json::{Value as Json, json};
use song::modules::types::type_meta;
use song::modules::{IoKind, LoadedModule, ModuleKind, Plugin};

static NULL: Json = Json::Null;

/// The argument at `i`, `null` when the call carries fewer.
pub fn at(a: &[Json], i: usize) -> &Json {
    a.get(i).unwrap_or(&NULL)
}

pub fn s(a: &[Json], i: usize) -> String {
    at(a, i).as_str().unwrap_or("").to_owned()
}

pub fn opt(a: &[Json], i: usize) -> Option<String> {
    at(a, i).as_str().map(str::to_owned)
}

pub fn n(a: &[Json], i: usize) -> f64 {
    at(a, i).as_f64().unwrap_or(f64::NAN)
}

pub fn b(a: &[Json], i: usize) -> bool {
    at(a, i).as_bool().unwrap_or(false)
}

/// A number, or the harvest's `{"$":"number"}` mark for an infinity; 0 when absent.
pub fn fnum(a: &[Json], i: usize) -> f64 {
    match a.get(i) {
        Some(Json::Object(o)) if o.get("$").and_then(Json::as_str) == Some("number") => match o["v"].as_str() {
            Some("-Infinity") => f64::NEG_INFINITY,
            Some("Infinity") => f64::INFINITY,
            _ => f64::NAN,
        },
        other => other.and_then(Json::as_f64).unwrap_or(0.0),
    }
}

/// A number as JSON, an infinity as the `{"$":"number"}` mark.
pub fn number(x: f64) -> Json {
    if x.is_finite() {
        json!(x)
    } else {
        json!({ "$": "number", "v": if x < 0.0 { "-Infinity" } else { "Infinity" } })
    }
}

pub fn undefined() -> Json {
    json!({ "$": "undefined" })
}

/// An optional string, `undefined` for none, as the harvest records a missing return.
pub fn opt_json(x: Option<String>) -> Json {
    x.map_or_else(undefined, |p| json!(p))
}

pub fn done<T>(r: Result<T, Error>, ok: impl FnOnce(T) -> Json) -> Out {
    match r {
        Ok(x) => Ok(ok(x)),
        Err(e) => Err(Failure::Thrown(e.to_string())),
    }
}

pub fn strs(v: &[String]) -> Json {
    json!(v)
}

pub fn params(p: &[(String, f64)]) -> Json {
    Json::Object(p.iter().map(|(k, v)| (k.clone(), json!(v))).collect())
}

pub fn string_list(j: &Json) -> Vec<String> {
    j.as_array()
        .map(|r| r.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

pub fn floats(j: &Json) -> Vec<f64> {
    j.as_array()
        .map(|a| a.iter().filter_map(Json::as_f64).collect())
        .unwrap_or_default()
}

/// A value from JSON, or None for what JSON cannot carry (an object with a `$` key).
pub fn val_strict(j: &Json) -> Option<Value> {
    Some(match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Num(n.as_f64()?),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(val_strict).collect::<Option<_>>()?),
        Json::Object(o) if o.contains_key("$") => return None,
        Json::Object(o) => Value::object(
            o.iter()
                .map(|(k, v)| Some((k.clone(), val_strict(v)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

/// A value from JSON where `undefined` is null.
pub fn val_lax(j: &Json) -> Value {
    match j {
        Json::Object(o) if o.get("$").and_then(Json::as_str) == Some("undefined") => Value::Null,
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(val_lax).collect()),
        Json::Object(o) => Value::object(o.iter().map(|(k, v)| (k.clone(), val_lax(v))).collect()),
    }
}

pub fn json_of(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Num(n) => json!(n),
        Value::Str(s) => json!(s),
        Value::List(l) => Json::Array(l.iter().map(json_of).collect()),
        Value::Map(m) => Json::Object(m.iter().map(|(k, x)| (k.clone(), json_of(x))).collect()),
    }
}

pub fn path_of(j: &Json) -> Option<Vec<PathSeg>> {
    j.as_array()?
        .iter()
        .map(|s| match s {
            Json::String(k) => Some(PathSeg::Key(k.clone())),
            Json::Number(n) => Some(PathSeg::Index(usize::try_from(n.as_u64()?).ok()?)),
            _ => None,
        })
        .collect()
}

pub fn seg_json(p: &PathSeg) -> Json {
    match p {
        PathSeg::Key(k) => json!(k),
        PathSeg::Index(i) => json!(i),
    }
}

/// The engine's modules as the host lists them: `{key: {category, file, plugin?, io?}}`.
pub fn catalogue_of(j: &Json) -> IndexMap<String, LoadedModule> {
    let mut cat = IndexMap::new();
    for (k, m) in j.as_object().into_iter().flatten() {
        let category = m["category"].as_str().unwrap_or("");
        let mut lm = LoadedModule::bare(
            type_meta(category).map_or(ModuleKind::Plugin, |t| t.kind),
            category,
            m["file"].as_str().unwrap_or(""),
        );
        if let Some(p) = m.get("plugin") {
            lm.authored.plugin = Some(Plugin {
                plugin_type: p["type"].as_str().map(str::to_owned),
                name: p["name"].as_str().map(str::to_owned),
                extra: Default::default(),
            });
        }
        if m["io"].as_str() == Some("out") {
            lm.authored.io = Some(IoKind::Out);
        }
        cat.insert(k.clone(), lm);
    }
    cat
}
