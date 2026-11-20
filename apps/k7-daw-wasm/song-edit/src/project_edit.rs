// project_edit.rs — a project changed in its own vocabulary ("set `Filter Freq` on module `synth`"): find the file, hand it to its writer.
// A reader would get wrong that a chain project's node is an item of its chain file, and a shared module is a file of a library.

use crate::alternative_file::{add_alternative, set_alternative_param};
use crate::chain_item_edit::{chain_file_path, chain_item_text, edit_chain_item};
use crate::confine::Env;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::loader::{load_module_refs, load_modules, resolve_node, roots_for};
use crate::module_file::{
    alternative_names, current_alternative, set_module_automation, set_module_list, set_module_value,
};
use crate::module_io::module_file_for;
use crate::paths::{join, relative};
use crate::workspace_doc::{EditOpts, ws_data, ws_edit};
use k7x::edit::{PathSeg, Scope, Value, parse};
use song::modules::ModuleKind;
use song::modules::types::split_module_key;
use song::resolve::{current_engine, keeps_its_module};

/// Where a module's text is: a file of its own, or an item of a chain file.
pub enum ModuleText {
    File(String),
    Item { dir: String, item: String },
}

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn path_of(at: &ModuleText) -> String {
    match at {
        ModuleText::File(p) => p.clone(),
        ModuleText::Item { dir, item } => chain_file_path(dir, item),
    }
}

/// Edit a module's document through `edit`, writing only when it changed; the file's path.
///
/// # Errors
/// What the read, the edit or the write fails with.
pub fn rewrite_module(fs: &mut dyn Fs, at: &ModuleText, edit: impl FnOnce(&mut Scope) -> Res<()>) -> Res<String> {
    if let ModuleText::Item { dir, item } = at {
        return edit_chain_item(fs, dir, item, edit);
    }
    let path = path_of(at);
    ws_edit(fs, &path, EditOpts::default(), |doc| edit(&mut doc.at(&[])))?;
    Ok(path)
}

fn read_module_data(fs: &dyn Fs, at: &ModuleText) -> Res<Value> {
    match at {
        ModuleText::Item { dir, item } => Ok(parse(&chain_item_text(fs, dir, item)?)?),
        ModuleText::File(p) => ws_data(fs, p),
    }
}

/// Where a module's file is: a node's own file, or a shared one at `@workspace`, `@k7-core`.
///
/// # Errors
/// A module the project does not have.
pub fn module_file_of(fs: &dyn Fs, env: &Env, dir: &str, module: &str) -> Res<ModuleText> {
    let modules = load_modules(fs, env, dir)?;
    if let Some(b) = modules.get(module) {
        if b.home.as_deref() == Some("chains") && b.chain.is_some() {
            return Ok(ModuleText::Item {
                dir: dir.to_owned(),
                item: module.to_owned(),
            });
        }
        if let Some(home) = &b.home {
            let folder = join(&[dir, home]);
            return match module_file_for(fs, &folder, home, module) {
                Some(f) => Ok(ModuleText::File(f.path)),
                None => fail(format!("module \"{module}\" has no file in {home}")),
            };
        }
    }
    if let Some(shared) = shared_module_file_of(fs, env, dir, module) {
        return Ok(shared);
    }
    fail(format!("no module \"{module}\" in this project"))
}

fn shared_module_file_of(fs: &dyn Fs, env: &Env, dir: &str, k: &str) -> Option<ModuleText> {
    if !k.starts_with('@') {
        return None;
    }
    let (module, node) = split_module_key(k);
    let mut parts = module[1..].split('/');
    let ns = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let root = roots_for(fs, env, dir)
        .module_roots()
        .ok()?
        .into_iter()
        .find(|r| r.namespace == ns)?
        .dir;
    if rest.is_empty() {
        return None;
    }
    let folder = join(&std::iter::once(root.as_str()).chain(rest).collect::<Vec<_>>());
    module_file_for(fs, &folder, "", node.unwrap_or("module")).map(|f| ModuleText::File(f.path))
}

/// Edit a module's node file in place, resolved through the loader.
///
/// # Errors
/// A module with no file, or what the edit fails with.
pub fn edit_module_file(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
) -> Res<String> {
    let at = module_file_of(fs, env, dir, module)?;
    rewrite_module(fs, &at, edit)
}

/// Bind a curve to a module's param; `None` unbinds, and `trigger` names whose note-ons restart an envelope.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_automation_binding(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    param: &str,
    curve: Option<&str>,
    trigger: Option<&str>,
) -> Res<String> {
    edit_module_file(fs, env, dir, module, |doc| {
        set_module_automation(doc, param, curve, trigger)
    })
}

fn edit_module_value(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    path: &[&str],
    value: Option<Value>,
) -> Res<String> {
    let p: Vec<PathSeg> = path.iter().map(|k| key(k)).collect();
    edit_module_file(fs, env, dir, module, |doc| set_module_value(doc, &p, value.as_ref()))
}

/// The engine a module runs as, resolved like the compile resolves it.
fn engine_on(fs: &dyn Fs, env: &Env, dir: &str, module: &str) -> Res<Option<String>> {
    Ok(resolve_node(fs, env, dir, module)?.and_then(|r| current_engine(&r)))
}

fn set_param_at(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, name: &str, value: &Value) -> Res<String> {
    let engine = engine_on(fs, env, dir, module)?;
    let at = module_file_of(fs, env, dir, module)?;
    rewrite_module(fs, &at, |doc| {
        set_alternative_param(doc, name, Some(value), engine.as_deref())
    })
}

/// Set one param on a module node, on the attribute set that is on.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_param(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, name: &str, value: f64) -> Res<String> {
    set_param_at(fs, env, dir, module, name, &Value::Num(value))
}

/// Point a file-valued param at one of the module's own assets, or clear it.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_asset(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    param: &str,
    file: Option<&str>,
) -> Res<String> {
    match file {
        Some(f) => set_param_at(fs, env, dir, module, param, &Value::Str(f.to_owned())),
        None => {
            let engine = engine_on(fs, env, dir, module)?;
            let at = module_file_of(fs, env, dir, module)?;
            rewrite_module(fs, &at, |doc| {
                set_alternative_param(doc, param, None, engine.as_deref())
            })
        }
    }
}

/// What a module's file says its attribute sets are called, and which is on.
///
/// # Errors
/// A module with no file, or one that does not read.
pub fn module_alternatives(fs: &dyn Fs, env: &Env, dir: &str, module: &str) -> Res<(Vec<String>, String)> {
    let raw = read_module_data(fs, &module_file_of(fs, env, dir, module)?)?;
    let names = alternative_names(&raw);
    let current = names[current_alternative(&raw)].clone();
    Ok((names, current))
}

/// Add one and switch to it: empty, or a copy of the set that is on; a lane's empty one drops its placements.
///
/// # Errors
/// As `edit_module_file`, or a name the module already has.
pub fn new_module_alternative(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    name: Option<&str>,
    copy: bool,
) -> Res<(String, String)> {
    let blank = resolve_node(fs, env, dir, module)?.is_some_and(|r| r.kind == Some(ModuleKind::Track));
    let at = module_file_of(fs, env, dir, module)?;
    let mut chosen = name.unwrap_or("").to_owned();
    let path = rewrite_module(fs, &at, |doc| {
        chosen = add_alternative(doc, name, copy, blank)?;
        Ok(())
    })?;
    Ok((path, chosen))
}

/// Point a module at its render, or clear it; refused on a shared one.
///
/// # Errors
/// A shared module, or as `edit_module_file`.
pub fn set_module_frozen(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, render: Option<&str>) -> Res<String> {
    if module.starts_with('@') {
        return fail(format!(
            "'{module}' comes from a shared catalogue — freeze the node that uses it, not the module itself"
        ));
    }
    edit_module_value(
        fs,
        env,
        dir,
        module,
        &["frozen"],
        render.map(|r| Value::Str(r.to_owned())),
    )
}

/// Set a live input's `monitor` (`off`, `auto` or `on`).
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_monitor(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, value: &str) -> Res<String> {
    edit_module_value(fs, env, dir, module, &["monitor"], Some(Value::Str(value.to_owned())))
}

fn set_trimmed(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, field: &str, text: &str) -> Res<String> {
    let clean = text.trim();
    let v = (!clean.is_empty()).then(|| Value::Str(clean.to_owned()));
    edit_module_value(fs, env, dir, module, &[field], v)
}

/// Choose which shader a node runs; empty clears it.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_shader(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, name: &str) -> Res<String> {
    set_trimmed(fs, env, dir, module, "shader", name)
}

/// Which camera a live picture node opens, by its label; empty is the default.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_camera(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, label: &str) -> Res<String> {
    set_trimmed(fs, env, dir, module, "camera", label)
}

/// Which MIDI device a midi-in node records from, by its label; empty clears it.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_device(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, label: &str) -> Res<String> {
    set_trimmed(fs, env, dir, module, "device", label)
}

/// Choose which named base a module starts from; empty clears it.
///
/// # Errors
/// As `edit_module_file`.
pub fn set_module_base(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, name: &str) -> Res<String> {
    set_trimmed(fs, env, dir, module, "base", name)
}

/// Set a live input's `gain` in dB, deliberately unclamped.
///
/// # Errors
/// A gain that is not a number.
pub fn set_module_input_gain(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, db: f64) -> Res<String> {
    if !db.is_finite() {
        return fail("a gain is a number of decibels");
    }
    edit_module_value(fs, env, dir, module, &["gain"], Some(Value::Num(db)))
}

/// Set an effect's `mix`; 1 (all wet) is no line at all.
///
/// # Errors
/// A mix outside 0 to 1.
pub fn set_module_mix(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, mix: f64) -> Res<String> {
    if !mix.is_finite() || !(0.0..=1.0).contains(&mix) {
        return fail("a mix is a number from 0 (all dry) to 1 (all wet)");
    }
    edit_module_value(fs, env, dir, module, &["mix"], (mix < 1.0).then_some(Value::Num(mix)))
}

/// Set a module's level into (`input`) or out of it (`output`), in dB; 0 is no line at all.
///
/// # Errors
/// A level outside -60 to +12.
pub fn set_module_volume(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, side: &str, db: f64) -> Res<String> {
    if !db.is_finite() || !(-60.0..=12.0).contains(&db) {
        return fail("a volume is a number of dB from -60 to +12");
    }
    let field = format!("{side}_volume");
    edit_module_value(fs, env, dir, module, &[&field], (db != 0.0).then_some(Value::Num(db)))
}

/// Set a live input's `channel`: one for mono, two for a stereo pair.
///
/// # Errors
/// An empty list, more than two, or a channel that is not a whole number from 1.
pub fn set_module_channel(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, channels: &[f64]) -> Res<String> {
    if channels.is_empty() {
        return fail("an input needs at least one channel");
    }
    if channels.len() > 2 {
        return fail("an input is one channel or a stereo pair, not more");
    }
    if channels.iter().any(|n| n.fract() != 0.0 || *n < 1.0 || !n.is_finite()) {
        return fail("channels are whole numbers counted from 1");
    }
    edit_module_file(fs, env, dir, module, |doc| {
        set_module_list(doc, &[key("channel")], channels)
    })
}

/// Repoint a node at another module: rewrite its `extends`, keep everything else.
///
/// # Errors
/// A module that extends itself, an unknown ref, a lane's set, or as `edit_module_file`.
pub fn set_module_extends(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    to: &str,
) -> Res<(String, Vec<String>, Option<String>)> {
    if module == to {
        return fail(format!("'{module}' cannot extend itself"));
    }
    let known = load_module_refs(fs, env, dir)?;
    if !known.modules.contains_key(to) {
        let refs = base_module_refs(&known.modules);
        let list = if refs.is_empty() {
            " — this project has none".to_owned()
        } else {
            format!(" — this project has: {}", refs.join(", "))
        };
        return fail(format!("unknown module '{to}'{list}"));
    }
    let at = module_file_of(fs, env, dir, module)?;
    let raw = read_module_data(fs, &at)?;
    let on = current_alternative(&raw);
    if on > 0 {
        refuse_swap(fs, env, dir, module)?;
    }
    let mut dropped: Vec<String> = Vec::new();
    let mut set = None;
    let path = rewrite_module(fs, &at, |doc| {
        let raw = doc.to_js()?.unwrap_or(Value::Map(vec![]));
        let on = current_alternative(&raw);
        let base: Vec<PathSeg> = if on > 0 {
            set = Some(alternative_names(&raw)[on].clone());
            vec![key("alternatives"), PathSeg::Index(on - 1)]
        } else {
            vec![]
        };
        let at = |k: &str| [base.clone(), vec![key(k)]].concat();
        doc.set_in(
            &at("extends"),
            &Value::Str(to.to_owned()),
            k7x::edit::WriteOptions::default(),
        )?;
        for k in ["plugin", "params", "rack"] {
            if doc.has_in(&at(k))? {
                doc.delete_in(&at(k), false)?;
                dropped.push(k.to_owned());
            }
        }
        Ok(())
    })?;
    Ok((relative(dir, &path), dropped, set))
}

/// A lane or its fader keeps its module whatever set is on, so repointing one of its sets is refused.
fn refuse_swap(fs: &dyn Fs, env: &Env, dir: &str, module: &str) -> Res<()> {
    let Some(own) = crate::loader::resolve_node_with(fs, env, dir, module, |m| m.authored.alternative = None)? else {
        return Ok(());
    };
    if keeps_its_module(&own) {
        let what = if own.kind == Some(ModuleKind::Track) {
            "lane"
        } else {
            "lane's fader"
        };
        return fail(format!(
            "'{module}' is a {what} — its alternatives can change its settings, not its module"
        ));
    }
    Ok(())
}

/// The plugin refs a new module can `extends`: not an io module and not a node.
#[must_use]
pub fn base_module_refs(by_ref: &indexmap::IndexMap<String, song::LoadedModule>) -> Vec<String> {
    let mut out: Vec<String> = by_ref
        .iter()
        .filter(|(_, m)| {
            m.kind == Some(ModuleKind::Plugin)
                && m.node.is_none()
                && m.engine_of.is_none()
                && !matches!(m.category.as_deref(), Some("console" | "cold"))
        })
        .map(|(r, _)| r.clone())
        .collect();
    out.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    out
}
