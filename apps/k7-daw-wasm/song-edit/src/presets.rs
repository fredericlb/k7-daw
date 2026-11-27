// presets.rs — a module's named settings, one FILE each, in a `presets/` tree beside the modules: schema, text, paths and the listing.
// A reader would get wrong that the scope is WHERE a preset lives and the path is WHICH module it is for: a workspace may keep presets for a library module.

use crate::confine::{Env, is_library_name};
use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::loader::{load_modules, roots_for};
use crate::paths::{is_within, join, resolve};
use crate::sugar::strip_doc_ext;
use crate::workspace_file::{is_doc_name, ws_exists};
use k7x::edit::{StringifyOptions, Value, stringify};
use song::Kind;
use song::Library;
use song::resolve::swapped_module;

pub const SHARED_PRESETS_DIR: &str = "_presets";
pub const LIBRARY_PRESETS_DIR: &str = "presets";
pub const WORKSPACE_SCOPE: &str = "workspace";
pub const ACCOUNT_LIBRARY: &str = "account";
/// The level a project's own samples answer to: a sample a preset may not keep, being one project's.
pub const PROJECT_SAMPLE_PREFIX: &str = "@project/";

/// One pad of a rack's kit as a preset keeps it: a note and a LIBRARY sample.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetTap {
    pub note: i64,
    pub sample: Option<String>,
}

/// One preset file: the values it sets, the engine they were taken on, the base they sit over, and a rack's kit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PresetFile {
    pub engine: Option<String>,
    pub base: Option<String>,
    pub params: Option<Vec<(String, f64)>>,
    pub rack: Option<Vec<(String, PresetTap)>>,
}

pub use song::wire::{NodePresets, PresetEntry};

/// `^[A-Za-z0-9][A-Za-z0-9 _-]{0,63}$`.
#[must_use]
pub fn is_preset_name(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_alphanumeric()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b' ' | b'_' | b'-'))
}

fn segment_ok(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && (b[0].is_ascii_alphanumeric() || b[0] == b'_')
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'+' | b'-'))
}

/// `@k7-core/instruments/synth` → `k7-core/instruments/synth`, the folder its presets sit in; None for a ref with no such path.
#[must_use]
pub fn preset_module_path(reference: &str) -> Option<String> {
    let rest = reference.strip_prefix('@')?;
    // `.` stops at a line break, and the pattern ends the string.
    if rest.is_empty() || rest.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    let segs: Vec<&str> = rest.split('/').collect();
    (segs.len() >= 2 && segs.iter().all(|s| segment_ok(s))).then(|| segs.join("/"))
}

/// A preset as k7x text, headed by the module it is for; a rack's pad stays on one line.
///
/// # Errors
/// A number the text cannot spell.
pub fn preset_file_text(reference: &str, preset: &PresetFile) -> Res<String> {
    let mut fields: Vec<(String, Value)> = vec![];
    if let Some(e) = &preset.engine {
        fields.push(("engine".to_owned(), Value::Str(e.clone())));
    }
    if let Some(b) = &preset.base {
        fields.push(("base".to_owned(), Value::Str(b.clone())));
    }
    if let Some(p) = &preset.params {
        let m = p.iter().map(|(k, v)| (k.clone(), Value::Num(*v))).collect();
        fields.push(("params".to_owned(), Value::object(m)));
    }
    if let Some(r) = &preset.rack {
        let m = r
            .iter()
            .map(|(k, t)| {
                let mut f = vec![("note".to_owned(), Value::Num(t.note as f64))];
                if let Some(s) = &t.sample {
                    f.push(("sample".to_owned(), Value::Str(s.clone())));
                }
                (k.clone(), Value::Map(f))
            })
            .collect();
        fields.push(("rack".to_owned(), Value::object(m)));
    }
    let header = [format!("preset for {reference}")];
    let inline = |path: &[k7x::edit::PathSeg], _: &Value| {
        path.len() == 2 && path[0] == k7x::edit::PathSeg::Key("rack".to_owned())
    };
    Ok(stringify(
        &Value::Map(fields),
        &StringifyOptions {
            header: &header,
            inline: Some(&inline),
        },
    )?)
}

fn received(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Num(_) => "number",
        Value::Str(_) => "string",
        Value::List(_) => "array",
        Value::Map(_) => "object",
    }
}

fn expected(what: &str, v: &Value) -> String {
    format!("Invalid input: expected {what}, received {}", received(v))
}

fn non_empty(v: &Value) -> Result<String, String> {
    match v {
        Value::Str(s) if s.is_empty() => Err("Too small: expected string to have >=1 characters".to_owned()),
        Value::Str(s) => Ok(s.clone()),
        o => Err(expected("string", o)),
    }
}

fn unrecognized(v: &Value, allowed: &[&str]) -> Result<(), String> {
    let Value::Map(m) = v else {
        return Err(expected("object", v));
    };
    let extra: Vec<String> = m
        .iter()
        .filter(|(k, _)| !allowed.contains(&k.as_str()))
        .map(|(k, _)| format!("\"{k}\""))
        .collect();
    match extra.len() {
        0 => Ok(()),
        1 => Err(format!("Unrecognized key: {}", extra[0])),
        _ => Err(format!("Unrecognized keys: {}", extra.join(", "))),
    }
}

fn tap(v: &Value) -> Result<PresetTap, String> {
    if !matches!(v, Value::Map(_)) {
        return Err(expected("object", v));
    }
    let note = match v.get("note") {
        Some(Value::Num(n)) if n.fract() != 0.0 => {
            return Err("Invalid input: expected int, received number".to_owned());
        }
        Some(Value::Num(n)) if *n < 0.0 => return Err("Too small: expected number to be >=0".to_owned()),
        Some(Value::Num(n)) if *n > 127.0 => return Err("Too big: expected number to be <=127".to_owned()),
        Some(Value::Num(n)) => *n as i64,
        Some(o) => return Err(expected("number", o)),
        None => return Err("Invalid input: expected number, received undefined".to_owned()),
    };
    let sample = v.get("sample").map(non_empty).transpose()?;
    unrecognized(v, &["note", "sample"])?;
    Ok(PresetTap { note, sample })
}

/// `PresetFileSchema.safeParse`: the preset, or the first finding's message.
///
/// # Errors
/// The wording of the first finding, as Zod 4 gives it for the ones a preset file can hold.
pub fn read_preset_data(data: &Value) -> Result<PresetFile, String> {
    if !matches!(data, Value::Map(_)) {
        return Err(expected("object", data));
    }
    let mut out = PresetFile {
        engine: data.get("engine").map(non_empty).transpose()?,
        base: data.get("base").map(non_empty).transpose()?,
        ..PresetFile::default()
    };
    if let Some(p) = data.get("params") {
        let Value::Map(m) = p else {
            return Err(expected("object", p));
        };
        let mut params = vec![];
        for (k, v) in m {
            if k.is_empty() {
                return Err("Invalid key in record".to_owned());
            }
            match v {
                Value::Num(n) => params.push((k.clone(), *n)),
                o => return Err(expected("number", o)),
            }
        }
        out.params = Some(params);
    }
    if let Some(r) = data.get("rack") {
        let Value::Map(m) = r else {
            return Err(expected("object", r));
        };
        let mut rack = vec![];
        for (k, v) in m {
            if k.is_empty() {
                return Err("Invalid key in record".to_owned());
            }
            rack.push((k.clone(), tap(v)?));
        }
        out.rack = Some(rack);
    }
    unrecognized(data, &["engine", "base", "params", "rack"])?;
    Ok(out)
}

/// Every library on this machine but the account's, by name order, as the project reads them.
#[must_use]
pub fn account_libraries(fs: &dyn Fs, env: &Env, dir: &str) -> Vec<Library> {
    let mut libs: Vec<Library> = roots_for(fs, env, dir)
        .libraries
        .into_iter()
        .filter(|l| l.name != ACCOUNT_LIBRARY)
        .collect();
    libs.sort_by(|a, b| a.name.encode_utf16().cmp(b.name.encode_utf16()));
    libs
}

/// Every scope this project can keep a preset in: its workspace, the account's library, then each library by name.
#[must_use]
pub fn preset_scopes(fs: &dyn Fs, env: &Env, dir: &str) -> Vec<String> {
    let mut out = vec![WORKSPACE_SCOPE.to_owned(), ACCOUNT_LIBRARY.to_owned()];
    out.extend(account_libraries(fs, env, dir).into_iter().map(|l| l.name));
    out
}

/// The module ref a node's presets are keyed by (what the node, as it plays, `extends`), or None for one with none.
///
/// # Errors
/// A node the project lacks, or a read the source refuses.
pub fn preset_ref_of(fs: &dyn Fs, env: &Env, dir: &str, node: &str) -> Res<Option<String>> {
    let nodes = load_modules(fs, env, dir)?;
    let Some(m) = nodes.get(node) else {
        return fail(format!("no node named '{node}'"));
    };
    let played = swapped_module(m);
    let reference = played
        .as_ref()
        .unwrap_or(m)
        .authored
        .extends
        .clone()
        .unwrap_or_default();
    Ok(preset_module_path(&reference).map(|_| reference))
}

/// The preset tree a scope keeps: `_presets/` beside the project, or a library's `presets/`.
fn scope_root(fs: &dyn Fs, env: &Env, dir: &str, scope: &str) -> Res<String> {
    if scope == WORKSPACE_SCOPE {
        return Ok(resolve(&join(&[dir, "..", SHARED_PRESETS_DIR])));
    }
    if scope == ACCOUNT_LIBRARY {
        return Ok(join(&[&env.studio_home, "library", LIBRARY_PRESETS_DIR]));
    }
    let lib = is_library_name(scope)
        .then(|| account_libraries(fs, env, dir).into_iter().find(|l| l.name == scope))
        .flatten();
    match lib {
        Some(l) => Ok(join(&[&l.root, LIBRARY_PRESETS_DIR])),
        None => fail(format!("no library '{scope}' to keep presets in")),
    }
}

/// The folder one module's presets sit in, inside a scope.
fn preset_folder(fs: &dyn Fs, env: &Env, dir: &str, scope: &str, reference: &str) -> Res<String> {
    let Some(rel) = preset_module_path(reference) else {
        return fail(format!("'{reference}' is not a module ref presets can be kept for"));
    };
    let root = scope_root(fs, env, dir, scope)?;
    let mut parts: Vec<&str> = vec![&root];
    parts.extend(rel.split('/'));
    Ok(join(&parts))
}

/// The file a preset is, refused when the name would leave its module's folder.
///
/// # Errors
/// A name or ref presets cannot be kept under, or a scope with no library.
pub fn preset_file(fs: &dyn Fs, env: &Env, dir: &str, scope: &str, reference: &str, name: &str) -> Res<String> {
    if !is_preset_name(name) {
        return fail(format!(
            "invalid preset name '{name}' — letters, digits, space, '_' and '-' only"
        ));
    }
    let folder = preset_folder(fs, env, dir, scope, reference)?;
    let path = resolve(&join(&[&folder, &format!("{name}.k7x")]));
    if !is_within(&folder, &path) || path == folder {
        return fail(format!("invalid preset name '{name}'"));
    }
    Ok(path)
}

/// What a node offers: its presets from every scope, workspace first, each scope by name.
///
/// # Errors
/// A node the project lacks, or a scope with no library.
pub fn list_node_presets(fs: &dyn Fs, env: &Env, dir: &str, node: &str) -> Res<NodePresets> {
    let scopes = preset_scopes(fs, env, dir);
    let Some(reference) = preset_ref_of(fs, env, dir, node)? else {
        return Ok(NodePresets {
            reference: None,
            presets: vec![],
            scopes,
        });
    };
    let mut presets = vec![];
    for scope in &scopes {
        let folder = preset_folder(fs, env, dir, scope, &reference)?;
        if !ws_exists(fs, &folder) {
            continue;
        }
        let entries = fs.list(&folder).map_err(|e| io_error(&e, "scandir", &folder))?;
        let mut names: Vec<String> = entries
            .into_iter()
            .filter(|e| e.kind == Kind::File && is_doc_name(&e.name))
            .map(|e| strip_doc_ext(&e.name).to_owned())
            .filter(|n| is_preset_name(n))
            .collect();
        // UTF-8 byte order is code point order.
        names.sort();
        presets.extend(names.into_iter().map(|name| PresetEntry {
            scope: scope.clone(),
            name,
        }));
    }
    Ok(NodePresets {
        reference: Some(reference),
        presets,
        scopes,
    })
}
