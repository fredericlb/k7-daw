// rows.rs — every parameter a module's widgets may bind, unioned from all the sources that know some, each row with the most specific range known.
// A reader would get wrong that rows come back UNSORTED: the page orders them with its own collator, which Rust has no copy of.

use super::write::read_params;
use crate::doc::{INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM};
use crate::files::scalar::js_space;
use crate::modules::names::VOLUME_RANGE;
use crate::modules::{builtin_plugin, builtin_plugin_for_ref};
use k7x::PathSeg;
use k7x::edit::{Document, Value as K7};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Track intrinsics a widget can bind besides plugin params: on a track node the graph routes these to the mixer.
const TRACK_BINDS: [&str; 4] = ["@pan", "@mute", "@solo", "@arm"];

/// A bind the page offers beside the params, with the range it brings along.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ExtraBind {
    pub name: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub meta: Value,
}

/// What `param_rows` unions: the file's text, the widgets' binds, and every source of names and ranges.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct RowSources {
    pub content: Option<String>,
    /// The binds of the panel's own widgets.
    #[serde(default)]
    pub bound: Vec<String>,
    pub io: Option<String>,
    pub path: String,
    /// The base's params, and the ranges the project knows for them.
    #[serde(default)]
    pub hint_params: Vec<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub hint_meta: Map<String, Value>,
    #[serde(default)]
    pub plugin_names: Vec<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub plugin_meta: Map<String, Value>,
    #[serde(default)]
    pub scanned_names: Vec<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub scanned_meta: Map<String, Value>,
    /// `@base`, offered only where there ARE bases: the page sorts its settings for people.
    #[serde(default)]
    pub base_binds: Vec<ExtraBind>,
}

/// One row of the params list.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ParamRowOut {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub bound: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub meta: Option<Value>,
}

/// An effect is a module filed under `audio/`, in the workspace or a library: the one family with a dry signal.
fn is_effect_path(path: &str) -> bool {
    in_folder(path, "audio")
}

fn in_folder(path: &str, folder: &str) -> bool {
    let needle = format!("modules/{folder}/");
    path.match_indices(&needle).any(|(i, _)| {
        let before = &path[..i];
        let before = before.strip_suffix('_').unwrap_or(before);
        before.is_empty() || before.ends_with('/')
    })
}

/// The footer an effect or an instrument gets, by its folder: the compile's own test, near enough without its inputs.
pub fn stage_for(path: &str, io: Option<&str>) -> Vec<&'static str> {
    if io.is_some() {
        Vec::new()
    } else if is_effect_path(path) {
        vec![INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM]
    } else if in_folder(path, "instruments") {
        vec![OUTPUT_VOLUME_PARAM]
    } else {
        Vec::new()
    }
}

/// What the FILE says an io setting is: a top-level field, which `read_params` cannot see.
fn authored_io_value(content: Option<&str>, bind: &str) -> Option<String> {
    let name = bind.strip_prefix('@')?;
    let prefix = format!("{name}:");
    for line in content?.split(['\n', '\r', '\u{2028}', '\u{2029}']) {
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let token = rest.trim_start_matches([' ', '\t']).trim_end_matches([' ', '\t']);
        if !token.is_empty() && !token.chars().any(js_space) {
            return Some(token.to_owned());
        }
    }
    None
}

fn plugin_of(doc: &Document) -> Option<(String, String)> {
    let Ok(Some(K7::Map(m))) = doc.get_in(&[PathSeg::Key("plugin".into())]) else {
        return None;
    };
    let get = |k: &str| match m.iter().find(|(n, _)| n == k) {
        Some((_, K7::Str(s))) => Some(s.clone()),
        _ => None,
    };
    Some((get("type")?, get("name")?))
}

fn extends_of(doc: &Document) -> Option<String> {
    match doc.get_in(&[PathSeg::Key("extends".into())]) {
        Ok(Some(K7::Str(s))) if !s.is_empty() => Some(s),
        _ => None,
    }
}

/// The built-in the file itself names, the source that needs NO project.
fn declared(content: Option<&str>) -> Vec<(String, f64, f64, Option<f64>)> {
    let Some(content) = content else { return Vec::new() };
    let doc = Document::new(content);
    if !doc.errors().is_empty() {
        return Vec::new();
    }
    let by_plugin = plugin_of(&doc).and_then(|(t, n)| builtin_plugin(Some(&t), Some(&n)));
    let found = by_plugin.or_else(|| extends_of(&doc).and_then(|r| builtin_plugin_for_ref(&r)));
    found
        .map(|p| p.params.iter().map(|q| (q.name.clone(), q.min, q.max, q.def)).collect())
        .unwrap_or_default()
}

fn io_binds(io: Option<&str>, path: &str) -> Vec<(String, Value)> {
    let volume = || json!({ "min": VOLUME_RANGE.0, "max": VOLUME_RANGE.1, "def": VOLUME_RANGE.2 });
    let mut out: Vec<(String, Value)> = Vec::new();
    if io == Some("in") {
        out.push(("@gain".into(), json!({ "min": -24, "max": 24 })));
        out.push((
            "@monitor".into(),
            json!({ "min": 0, "max": 2, "states": [
                { "label": "off", "value": 0 }, { "label": "auto", "value": 1 }, { "label": "on", "value": 2 }] }),
        ));
    }
    if io.is_none() && is_effect_path(path) {
        out.push((MIX_PARAM.into(), json!({ "min": 0, "max": 1, "def": 1 })));
        out.push((INPUT_VOLUME_PARAM.into(), volume()));
    }
    out.push((OUTPUT_VOLUME_PARAM.into(), volume()));
    out
}

/// Every addressable param, unioned from all the sources, in the order the names are first met.
pub fn param_rows(src: &RowSources) -> Vec<ParamRowOut> {
    let set = src.content.as_deref().map(read_params).unwrap_or_default();
    let declared = declared(src.content.as_deref());
    let io = io_binds(src.io.as_deref(), &src.path);
    let mut names: Vec<String> = Vec::new();
    let mut add = |n: &str| {
        if !names.iter().any(|x| x == n) {
            names.push(n.to_owned());
        }
    };
    set.keys().for_each(|n| add(n));
    src.plugin_names.iter().for_each(|n| add(n));
    src.scanned_names.iter().for_each(|n| add(n));
    src.hint_params.iter().for_each(|n| add(n));
    declared.iter().for_each(|d| add(&d.0));
    TRACK_BINDS.iter().for_each(|n| add(n));
    io.iter().for_each(|b| add(&b.0));
    src.base_binds.iter().for_each(|b| add(&b.name));
    let present = |m: &Map<String, Value>, n: &str| m.get(n).filter(|v| !v.is_null()).cloned();
    names
        .into_iter()
        .map(|name| {
            let declared_meta = declared.iter().find(|d| d.0 == name).map(|d| {
                let mut m = json!({ "min": d.1, "max": d.2 });
                if let Some(def) = d.3 {
                    m["def"] = json!(def);
                }
                m
            });
            let io_meta = src
                .base_binds
                .iter()
                .rev()
                .find(|b| b.name == name)
                .map(|b| b.meta.clone())
                .or_else(|| io.iter().rev().find(|b| b.0 == name).map(|b| b.1.clone()));
            let meta = present(&src.plugin_meta, &name)
                .or_else(|| present(&src.scanned_meta, &name))
                .or(declared_meta)
                .or_else(|| present(&src.hint_meta, &name))
                .or(io_meta);
            let value = match set.get(&name) {
                Some(Value::String(s)) => Some(s.clone()),
                _ => authored_io_value(src.content.as_deref(), &name),
            };
            ParamRowOut {
                bound: src.bound.contains(&name),
                value,
                meta,
                name,
            }
        })
        .collect()
}

/// A string as JavaScript's `Number()` reads it; None for NaN.
fn js_number(s: &str) -> Option<f64> {
    let t = s.trim_matches(js_space);
    if t.is_empty() {
        return Some(0.0);
    }
    let radix = |p: &[&str], r: u32| {
        p.iter()
            .find_map(|p| t.strip_prefix(p))
            .map(|d| u64::from_str_radix(d, r).ok().map(|n| n as f64))
    };
    if let Some(n) = radix(&["0x", "0X"], 16)
        .or_else(|| radix(&["0o", "0O"], 8))
        .or_else(|| radix(&["0b", "0B"], 2))
    {
        return n;
    }
    if t.chars().any(|c| c.is_ascii_alphabetic() && !matches!(c, 'e' | 'E')) {
        return None;
    }
    t.parse::<f64>().ok()
}

/// The same values as NUMBERS for the preview; anything that is not one is left out.
pub fn param_values(content: &str) -> Map<String, Value> {
    read_params(content)
        .into_iter()
        .filter_map(|(name, raw)| {
            let n = js_number(raw.as_str()?)?;
            n.is_finite().then(|| (name, super::loose::jnum(n)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_names_the_stage() {
        assert_eq!(stage_for("modules/audio/x/module.k7x", None).len(), 3);
        assert_eq!(
            stage_for("_libraries/a/_modules/instruments/y/module.k7x", None),
            vec![OUTPUT_VOLUME_PARAM]
        );
        assert!(stage_for("modules/audio/x/module.k7x", Some("in")).is_empty());
        assert!(stage_for("modules/audiox/x.k7x", None).is_empty());
    }

    #[test]
    fn numbers_read_as_javascript_reads_them() {
        assert_eq!(js_number(" 5 "), Some(5.0));
        assert_eq!(js_number(""), Some(0.0));
        assert_eq!(js_number("0x10"), Some(16.0));
        assert_eq!(js_number("1e3"), Some(1000.0));
        assert_eq!(js_number("abc"), None);
        assert_eq!(js_number("inf"), None);
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod cases_tests;
