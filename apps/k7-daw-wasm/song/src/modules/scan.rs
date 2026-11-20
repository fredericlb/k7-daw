// modules/scan.rs — the engine's plugin scan read through the Source, and the modules it offers by ref.
// A reader would get wrong that EMPTY means "cannot say", not "none installed": the built-in table then stands in for the scan.

use super::schema::{IoKind, Module, Plugin};
use super::types::{IO_MODULES, category_for_role, engine_ref, meta_ref, ports, slugify, type_meta};
use super::{LoadedModule, ModuleKind};
use crate::source::{Kind, Source, SourceError, Span};
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashSet;

/// Past this many plugins a scan reads as "cannot say": a hostile engine reply must not be held.
pub const SCAN_CAP: usize = 20_000;

/// A scan file larger than this is not read.
const MAX_SCAN_BYTES: u64 = 32 * 1024 * 1024;

/// One plugin as the engine's scan describes it; `format` is as scanned (`VST3`, `internal`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedPlugin {
    pub name: String,
    pub format: String,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    /// `instrument`, `audioEffect` or `midiEffect`.
    pub role: Option<String>,
    pub instrument: bool,
    pub category: Option<String>,
}

/// What the engine's scan said, sorted; no plugins means the scan cannot say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub plugins: Vec<ScannedPlugin>,
    keys: HashSet<String>,
}

/// `<format>|<name>`, both lowercased: the engine's case-insensitive match.
fn scan_key(format: &str, name: &str) -> String {
    format!("{}|{}", format.to_lowercase(), name.to_lowercase())
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// JavaScript's `String(v)` for what a scan holds.
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_owned(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|e| if e.is_null() { String::new() } else { js_string(e) })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
        other => other.to_string(),
    }
}

/// What `for (const x of v ?? [])` walks: None when `v` is not iterable (the TypeScript's throw).
fn iterate(v: Option<&Value>) -> Option<&[Value]> {
    match v {
        None | Some(Value::Null | Value::String(_)) => Some(&[]),
        Some(Value::Array(a)) => Some(a),
        Some(_) => None,
    }
}

fn plugins_of(root: &Value) -> Option<Vec<ScannedPlugin>> {
    let mut out: Vec<ScannedPlugin> = Vec::new();
    for e in iterate(root.get("entries"))? {
        if e.is_null() {
            return None;
        }
        for p in iterate(e.get("plugins"))? {
            let (Some(name), Some(format)) = (p.get("name"), p.get("format")) else {
                continue;
            };
            if !truthy(name) || !truthy(format) {
                continue;
            }
            if out.len() >= SCAN_CAP {
                return None;
            }
            let opt = |k: &str| p.get(k).filter(|v| truthy(v)).map(js_string);
            out.push(ScannedPlugin {
                name: js_string(name),
                format: js_string(format),
                id: opt("id"),
                manufacturer: opt("manufacturer"),
                role: opt("role"),
                instrument: p.get("instrument") == Some(&Value::Bool(true)),
                category: opt("category"),
            });
        }
    }
    Some(out)
}

impl Scan {
    /// The plugins in a scan, sorted by name then format by code point.
    pub fn from_plugins(mut plugins: Vec<ScannedPlugin>) -> Scan {
        plugins.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.format.cmp(&b.format)));
        let keys = plugins.iter().map(|p| scan_key(&p.format, &p.name)).collect();
        Scan { plugins, keys }
    }

    /// Reads the scan file: absent, not JSON or naming no plugin is a scan that cannot say.
    pub fn read(source: &dyn Source, path: Option<&str>) -> Result<Scan, SourceError> {
        let Some(path) = path else { return Ok(Scan::default()) };
        match source.stat(path) {
            Ok(st) if st.kind == Kind::File && st.len <= MAX_SCAN_BYTES => {}
            Ok(_) | Err(SourceError::NotFound | SourceError::Outside | SourceError::Io(_)) => {
                return Ok(Scan::default());
            }
            Err(e) => return Err(e),
        }
        let bytes = match source.read(path, Span::All) {
            Ok(b) => b,
            Err(SourceError::NotFound | SourceError::Outside | SourceError::Io(_)) => return Ok(Scan::default()),
            Err(e) => return Err(e),
        };
        // The file may have grown between the stat and the read.
        if bytes.len() as u64 > MAX_SCAN_BYTES {
            return Ok(Scan::default());
        }
        let plugins = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|root| plugins_of(&root))
            .unwrap_or_default();
        Ok(Scan::from_plugins(plugins))
    }

    /// Whether the scan can say anything at all.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Does the engine know this external plugin? False only against a NON-EMPTY scan.
    pub fn is_plugin_scanned(&self, format: &str, name: &str) -> bool {
        self.keys.is_empty() || self.keys.contains(&scan_key(format, name))
    }

    /// Does the scan name EVERY one of `plugins`? False against an empty scan, which cannot say.
    pub fn hosts_all(&self, plugins: &[(&str, &str)]) -> bool {
        !self.keys.is_empty() && plugins.iter().all(|(f, n)| self.keys.contains(&scan_key(f, n)))
    }

    pub fn has_key(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    /// Scanned plugin names grouped by lowercased format.
    pub fn names(&self) -> IndexMap<String, Vec<String>> {
        let mut out: IndexMap<String, Vec<String>> = IndexMap::new();
        for p in &self.plugins {
            out.entry(p.format.to_lowercase()).or_default().push(p.name.clone());
        }
        out
    }
}

/// The built-ins as the scan would list them: every engine has them, scanned or not.
pub fn builtin_scan() -> Vec<ScannedPlugin> {
    super::builtin::builtin_plugins()
        .values()
        .map(|b| ScannedPlugin {
            name: b.name.clone(),
            format: "internal".to_owned(),
            id: None,
            manufacturer: None,
            role: Some(b.role.clone()),
            instrument: b.role == "instrument",
            category: None,
        })
        .collect()
}

/// How a scanned plugin becomes a module: its ref and plugin block, None when the name leaves no slug.
pub fn engine_module_of(name: &str, format: &str) -> Option<(String, Plugin)> {
    let slug = slugify(name);
    if slug.is_empty() {
        return None;
    }
    // The FORMAT lowercased, never the role: the role mislabels a VST3 as whatever it does.
    let plugin = Plugin {
        plugin_type: Some(format.to_lowercase()),
        name: Some(name.to_owned()),
        ..Plugin::default()
    };
    Some((engine_ref(&slug), plugin))
}

/// Every module the engine's scan offers, keyed by ref; the built-ins stand in for a scan that cannot say.
pub fn engine_modules(scan: &Scan) -> IndexMap<String, LoadedModule> {
    let mut out = IndexMap::new();
    let fallback;
    let plugins = if scan.is_empty() {
        fallback = builtin_scan();
        &fallback
    } else {
        &scan.plugins
    };
    for p in plugins {
        let Some((r, plugin)) = engine_module_of(&p.name, &p.format) else {
            continue;
        };
        if out.contains_key(&r) {
            continue;
        }
        let category = category_for_role(p.role.as_deref(), p.instrument);
        let Some(meta) = type_meta(category) else { continue };
        let mut m = LoadedModule::bare(meta.kind, category, &slugify(&p.name));
        m.authored.plugin = Some(plugin);
        m.authored.inputs = Some(ports(meta.inputs));
        m.authored.outputs = Some(ports(meta.outputs));
        out.insert(r, m);
    }
    out
}

fn io_kind(io: &str) -> Option<IoKind> {
    Some(match io {
        "in" => IoKind::In,
        "out" => IoKind::Out,
        "file" => IoKind::File,
        "camera" => IoKind::Camera,
        "midi" => IoKind::Midi,
        _ => return None,
    })
}

/// The io roles the app provides: no scan behind them, so they need no machine to answer.
pub fn io_modules() -> IndexMap<String, LoadedModule> {
    let mut out = IndexMap::new();
    for (name, io) in IO_MODULES {
        let mut m = LoadedModule::bare(ModuleKind::Io, "io", name);
        m.authored = Module {
            io: io_kind(io),
            inputs: Some(Vec::new()),
            ..Module::default()
        };
        out.insert(meta_ref(name), m);
    }
    out
}
