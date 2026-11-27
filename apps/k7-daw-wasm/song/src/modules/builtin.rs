// modules/builtin.rs — the built-in plugins' parameters, known without a running engine: the generated table, parsed once.
// A reader would get wrong that the table is read where it is generated (song-schema), embedded here until the facade wires the compiler.

use super::types::{ENGINE_NS, slugify};
use crate::files::scalar::js_space;
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const TABLE: &str = include_str!("builtinParams.generated.json");

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuiltinState {
    pub label: String,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuiltinParam {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub def: Option<f64>,
    pub states: Option<Vec<BuiltinState>>,
    /// False for a control that is plain plugin state: no curve can drive it.
    pub automatable: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuiltinPlugin {
    pub name: String,
    /// `instrument`, `audioEffect` or `midiEffect`.
    pub role: String,
    pub params: Vec<BuiltinParam>,
}

struct Table {
    plugins: IndexMap<String, BuiltinPlugin>,
    by_spelling: HashMap<String, String>,
}

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let plugins: IndexMap<String, BuiltinPlugin> = serde_json::from_str(TABLE).unwrap_or_default();
        let mut by_spelling: HashMap<String, String> = HashMap::new();
        for (kind, info) in &plugins {
            by_spelling.insert(kind.to_lowercase(), kind.clone());
            // Neither derived spelling overwrites a TYPE, which is the addressable one.
            for alias in [info.name.clone(), slugify(&info.name)] {
                if !alias.is_empty() {
                    by_spelling.entry(alias.to_lowercase()).or_insert_with(|| kind.clone());
                }
            }
        }
        Table { plugins, by_spelling }
    })
}

/// Every built-in, keyed by xmlTypeName, in the table's order.
pub fn builtin_plugins() -> &'static IndexMap<String, BuiltinPlugin> {
    &table().plugins
}

fn by_spelling(spelling: &str) -> Option<&'static BuiltinPlugin> {
    let t = table();
    let key = spelling.trim_matches(js_space).to_lowercase();
    t.by_spelling.get(&key).and_then(|k| t.plugins.get(k))
}

/// The built-in a module declares (`type: internal`, any of the three spellings), or None.
pub fn builtin_plugin(plugin_type: Option<&str>, name: Option<&str>) -> Option<&'static BuiltinPlugin> {
    (plugin_type == Some("internal"))
        .then_some(name)
        .flatten()
        .and_then(by_spelling)
}

/// The built-in behind an `extends:` ref, the engine namespace only.
pub fn builtin_plugin_for_ref(r: &str) -> Option<&'static BuiltinPlugin> {
    let rest = r.strip_prefix(ENGINE_NS)?.strip_prefix('/')?;
    by_spelling(rest)
}

/// Whether a curve can drive `param` of the built-in spelled `kind`: false only for a control the table marks as state.
pub fn builtin_param_automatable(kind: &str, param: &str) -> bool {
    by_spelling(kind)
        .and_then(|i| i.params.iter().find(|p| p.name == param))
        .is_none_or(|p| p.automatable != Some(false))
}
