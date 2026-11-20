// node_presets.rs — a node's presets applied and saved: what the node resolves to written as a preset, a preset loaded onto the node.
// A reader would get wrong the order of an apply: engine, then base, then the values in ONE write, then the kit.

use crate::alternative_file::{clear_alternative_params, set_alternative_field, set_alternative_param};
use crate::base_preset::{BaseOwner, module_base_choices};
use crate::confine::Env;
use crate::engine_switch::select_module_engine;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::loader::{load_module_refs, load_modules, resolve_node};
use crate::paths::dirname;
use crate::presets::{
    ACCOUNT_LIBRARY, PROJECT_SAMPLE_PREFIX, PresetFile, PresetTap, WORKSPACE_SCOPE, account_libraries, preset_file,
    preset_file_text, preset_ref_of, read_preset_data,
};
use crate::project_edit::edit_module_file;
use crate::rack_taps::set_kit;
use crate::workspace_doc::{ws_data, ws_write_text};
use crate::workspace_file::ws_exists;
use indexmap::IndexMap;
use k7x::edit::{Value, js_trim};
use song::LoadedModule;
use song::modules::{ModuleType, ParamValue, RackType};
use song::resolve::{current_engine, engine_names, resolved_rack, swapped_module};

/// The node as it PLAYS: the module its attribute set names, when the set on swaps it for another.
fn played_as(fs: &dyn Fs, env: &Env, dir: &str, node: &str) -> Res<LoadedModule> {
    let nodes = load_modules(fs, env, dir)?;
    let Some(m) = nodes.get(node) else {
        return fail(format!("no node named '{node}'"));
    };
    Ok(swapped_module(m).unwrap_or_else(|| m.clone()))
}

/// The base a node is on: its own `base:`, else the nearest one up what it extends.
fn chosen_base(node: &LoadedModule, refs: &IndexMap<String, LoadedModule>) -> String {
    let mut cur = Some(node);
    for _ in 0..16 {
        let Some(c) = cur else { break };
        if let Some(b) = c.authored.base.as_deref() {
            let t = js_trim(b);
            if !t.is_empty() {
                return t.to_owned();
            }
        }
        cur = c.authored.extends.as_ref().and_then(|e| refs.get(e));
    }
    String::new()
}

/// A rack node's kit as a preset keeps it: refused when a pad plays a sound only this project has.
fn kit_snapshot(node: &str, r: &song::Resolved) -> Res<Vec<(String, PresetTap)>> {
    let rack = resolved_rack(Some(r));
    if rack.rack_type == RackType::Slice {
        return fail(format!(
            "'{node}' slices one source — only a kit of library samples can be kept as a preset"
        ));
    }
    let mut kit = vec![];
    for (sym, tap) in &rack.symbols {
        let sample = tap.sample.as_ref().map(|s| s.0.clone());
        if let Some(s) = sample.as_deref().filter(|s| s.starts_with(PROJECT_SAMPLE_PREFIX)) {
            return fail(format!(
                "pad '{sym}' plays {s}, a sample only this project has — move it into a library first"
            ));
        }
        if sample.is_none() && tap.clip.is_some() {
            return fail(format!(
                "pad '{sym}' plays a file of the module's own folder — give it a library sample first"
            ));
        }
        kit.push((
            sym.clone(),
            PresetTap {
                note: i64::from(tap.note.0),
                sample,
            },
        ));
    }
    Ok(kit)
}

/// What a node plays as right now: its resolved numeric params, the engine and base they belong to, and a rack's kit.
fn node_snapshot(fs: &dyn Fs, env: &Env, dir: &str, node: &str) -> Res<PresetFile> {
    let Some(resolved) = resolve_node(fs, env, dir, node)? else {
        return fail(format!("no node named '{node}'"));
    };
    let params: Vec<(String, f64)> = resolved
        .authored
        .params
        .iter()
        .flatten()
        .filter_map(|(k, v)| match v {
            ParamValue::Number(n) if n.is_finite() => Some((k.clone(), *n)),
            _ => None,
        })
        .collect();
    let engine = if engine_names(&resolved).len() > 1 {
        current_engine(&resolved)
    } else {
        None
    };
    let rack = if resolved.authored.module_type == Some(ModuleType::Rack) {
        Some(kit_snapshot(node, &resolved)?)
    } else {
        None
    };
    let refs = load_module_refs(fs, env, dir)?;
    let base = chosen_base(&played_as(fs, env, dir, node)?, &refs.modules);
    Ok(PresetFile {
        engine,
        base: (!base.is_empty()).then_some(base),
        params: (!params.is_empty()).then_some(params),
        rack: rack.filter(|r| !r.is_empty()),
    })
}

/// Whether a library the preset goes to is one this account writes: the account's own, or an admin's overlay.
fn scope_editable(fs: &dyn Fs, env: &Env, dir: &str, scope: &str) -> bool {
    scope == ACCOUNT_LIBRARY
        || account_libraries(fs, env, dir)
            .iter()
            .any(|l| l.name == scope && l.editable)
}

/// Which write a preset makes: a new one refuses a taken name, an update needs it to exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetMode {
    New,
    Update,
}

/// Write a node's state as a preset; the file, the module ref and the preset.
///
/// # Errors
/// A node with no module to key by, a scope the account may not write, a name taken or missing, or nothing to keep.
pub fn write_node_preset(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    scope: &str,
    name: &str,
    mode: PresetMode,
) -> Res<(String, String, PresetFile)> {
    let Some(reference) = preset_ref_of(fs, env, dir, node)? else {
        return fail(format!("'{node}' extends no module, so it has no presets"));
    };
    if scope != WORKSPACE_SCOPE && !scope_editable(fs, env, dir, scope) {
        return fail(format!(
            "a preset is kept in '{scope}' by the origin's admins only — a library is theirs to edit"
        ));
    }
    let path = preset_file(fs, env, dir, scope, &reference, js_trim(name))?;
    let exists = ws_exists(fs, &path);
    if mode == PresetMode::New && exists {
        return fail(format!("preset '{name}' already exists in {scope} — update it instead"));
    }
    if mode == PresetMode::Update && !exists {
        return fail(format!("no preset '{name}' in {scope} to update"));
    }
    let preset = node_snapshot(fs, env, dir, node)?;
    // A base alone is worth keeping: it is the whole sound of a patch.
    if preset.params.is_none() && preset.rack.is_none() && preset.base.is_none() {
        return fail(format!("'{node}' has no params, base or pads to keep — set some first"));
    }
    let folder = dirname(&path);
    fs.mkdir_all(&folder)
        .map_err(|e| crate::error::io_error(&e, "mkdir", &folder))?;
    ws_write_text(fs, &path, &preset_file_text(&reference, &preset)?)?;
    Ok((path, reference, preset))
}

/// One preset's content; a file the schema refuses fails with the reason, since applying half of one would be worse.
fn read_preset(fs: &dyn Fs, env: &Env, dir: &str, scope: &str, reference: &str, name: &str) -> Res<PresetFile> {
    let path = preset_file(fs, env, dir, scope, reference, name)?;
    if !ws_exists(fs, &path) {
        return fail(format!("no preset '{name}' in {scope} for {reference}"));
    }
    match read_preset_data(&ws_data(fs, &path)?) {
        Ok(p) => Ok(p),
        Err(m) => fail(format!("preset '{name}' is malformed: {m}")),
    }
}

/// What applying a preset did.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub params: Vec<(String, f64)>,
    pub switched: bool,
    pub rebased: bool,
    pub kit: bool,
    pub missing_engine: Option<String>,
    pub missing_base: Option<String>,
}

fn owner(m: &LoadedModule) -> BaseOwner {
    BaseOwner {
        extends: m.authored.extends.clone(),
        folder_abs: m.folder_abs.clone(),
    }
}

/// Load a preset onto a node: its engine first, then its values in ONE write, then its kit in place of the node's.
///
/// # Errors
/// A node with no module to key by, a preset that is missing or malformed, or a kit the node cannot take.
pub fn apply_preset_to_node(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    scope: &str,
    name: &str,
) -> Res<Applied> {
    let Some(reference) = preset_ref_of(fs, env, dir, node)? else {
        return fail(format!("'{node}' extends no module, so it has no presets"));
    };
    let preset = read_preset(fs, env, dir, scope, &reference, name)?;
    let params = preset.params.clone().unwrap_or_default();
    let resolved = |fs: &dyn Fs| -> Res<song::Resolved> {
        match resolve_node(fs, env, dir, node)? {
            Some(r) => Ok(r),
            None => fail(format!("no node named '{node}'")),
        }
    };
    let mut switched = false;
    let mut missing_engine = None;
    // Checked before anything is written, so a kit the node cannot take leaves it untouched.
    if preset.rack.is_some() && resolved(fs)?.authored.module_type != Some(ModuleType::Rack) {
        return fail(format!("preset '{name}' holds a kit and '{node}' is not a rack"));
    }
    if let Some(engine) = &preset.engine {
        let r = resolved(fs)?;
        if !engine_names(&r).contains(engine) {
            missing_engine = Some(engine.clone());
        } else if current_engine(&r).as_ref() != Some(engine) {
            select_module_engine(fs, env, dir, node, engine)?;
            switched = true;
        }
    }
    // The base next, since the values are what goes on top of it.
    let mut rebased = false;
    let mut missing_base = None;
    if let Some(base) = &preset.base {
        let m = played_as(fs, env, dir, node)?;
        let refs = load_module_refs(fs, env, dir)?;
        let current = chosen_base(&m, &refs.modules);
        let lookup = |n: &str| refs.modules.get(n).map(owner);
        if !module_base_choices(fs, Some(&owner(&m)), &lookup).contains(base) {
            missing_base = Some(base.clone());
        } else if current != *base {
            // Into the set that is on, as the values go: a node whose set swaps its module reads its base there.
            edit_module_file(fs, env, dir, node, |doc| set_alternative_field(doc, "base", base))?;
            rebased = true;
        }
    }
    // The engine that is on AFTER the switch, since a module with engines keeps its knobs per engine.
    let engine = current_engine(&*resolved(fs)?);
    // A new base drops the node's own values: they were set over the old state and mean nothing over this one.
    if rebased || !params.is_empty() {
        edit_module_file(fs, env, dir, node, |doc| {
            if rebased {
                clear_alternative_params(doc, engine.as_deref())?;
            }
            for (k, v) in &params {
                set_alternative_param(doc, k, Some(&Value::Num(*v)), engine.as_deref())?;
            }
            Ok(())
        })?;
    }
    if let Some(rack) = &preset.rack {
        let taps: Vec<(String, f64, Option<String>)> = rack
            .iter()
            .map(|(k, t)| (k.clone(), t.note as f64, t.sample.clone()))
            .collect();
        set_kit(fs, env, dir, node, &taps)?;
    }
    Ok(Applied {
        params,
        switched,
        rebased,
        kit: preset.rack.is_some(),
        missing_engine,
        missing_base,
    })
}
