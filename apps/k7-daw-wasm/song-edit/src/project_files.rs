// project_files.rs — the writers that CREATE a node or clone a module in a project, after projectFiles.ts.
// A reader would get wrong that a node in a chain project is a chain file, not `nodes/`: the name asked for names the chain.

use crate::chain_create::{chain_taken, create_chain_node};
use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::ids::NewId;
use crate::instance_name::instance_name;
use crate::loader::{NODES_DIR, load_modules};
use crate::module_io::{is_project_folder, module_file_for};
use crate::paths::join;
use crate::shared_folders::SHARED_MODULES_PATH;
use crate::workspace_doc::{EditOpts, ws_edit, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{PathSeg, StringifyOptions, Value, WriteOptions, stringify};
use song::Kind;
use song::modules::LoadedModule;
use song::modules::panels::module_ref_of;
use song::modules::types::{IO_MODULES, MODULE_TYPES, engine_ref, is_io_source};

fn valid(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn unknown_category(category: &str) -> String {
    let all: Vec<&str> = MODULE_TYPES.iter().map(|t| t.name).collect();
    format!("unknown module category '{category}' — use one of: {}", all.join(", "))
}

fn io_names() -> String {
    IO_MODULES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
}

fn node_text(header: &[String], body: Value) -> Res<String> {
    Ok(stringify(&body, &StringifyOptions { header, inline: None })?)
}

fn mkdirs(fs: &mut dyn Fs, dir: &str) -> Res<()> {
    fs.mkdir_all(dir).map_err(|e| io_error(&e, "mkdir", dir))
}

/// What an io module's `module.k7x` says: the role line is what the loader reads, the rest is prose for the reader.
fn io_module_body(module: &str, role: &str) -> String {
    match role {
        "in" => format!(
            "// {module}: this machine's audio input, the live rig's IN.\n//\n// `monitor(\"auto\")` is audible once its track is armed; \"on\" always, \"off\" never.\nio(\"in\")\nmonitor(\"auto\")\n// …and a trim in dB, applied at the interface ahead of everything: 0 is unity.\ngain(0)\n"
        ),
        "camera" => format!(
            "// {module}: a live picture — the one io source whose signal is video, not audio (docs/camera.md).\n//\n// It names NO lens: which camera you are on is a fact about your rig. Empty means whichever\n// one this machine calls default, so it draws a picture before you have chosen anything.\n// Pick it on the node's card, or write the LABEL here — never a deviceId, which is salted\n// per browser and rotates:\n//\n//     camera(\"Logitech BRIO\")\nio(\"camera\")\n"
        ),
        "midi" => format!(
            "// {module}: a MIDI device — the notes a lane records, when this node is wired to it (docs/midiIn.md).\n//\n// Pick the device on the node's card, or write its LABEL here — never an id, which is per\n// browser. Empty records from any device:\n//\n//     device(\"Arturia KeyStep 37\")\nio(\"midi\")\n"
        ),
        "file" => format!(
            "// {module}: audio off disk rather than off an interface (docs/fileInput.md).\n//\n// `source` is optional and absent is the useful case — pick the file on the node's\n// card, or name it here, project-relative:\n//\n//     source(\"clips/original.wav\")\nio(\"file\")\n"
        ),
        _ => format!(
            "// {module}: an output sink — OUT the interface, CUE the headphone bus. A module\n// reaches it by naming it as a destination in the graph.\nio(\"out\")\n"
        ),
    }
}

/// Create an io NODE directly: the role is the vocabulary's, the name the graph node's; a chain project answers the node's id.
///
/// # Errors
/// A name that is not a name, a file already there, or the file system's.
pub fn create_io_module_file(fs: &mut dyn Fs, dir: &str, name: &str, role: &str, new_id: NewId) -> Res<String> {
    if !valid(name) {
        return fail(format!(
            "invalid io module name '{name}' — letters, digits, '_' and '-' only"
        ));
    }
    let body = io_module_body(name, role);
    if crate::chain_item_edit::is_chain_project(fs, dir) {
        return Ok(create_chain_node(fs, dir, name, &body, new_id)?.0);
    }
    let rel = format!("{NODES_DIR}/{name}.k7x");
    if fs.exists(&join(&[dir, &rel])) {
        return fail(format!("{rel} already exists — refusing to overwrite"));
    }
    mkdirs(fs, &join(&[dir, NODES_DIR]))?;
    ws_write_text(fs, &join(&[dir, &rel]), &body)?;
    Ok(rel)
}

/// Is this name spoken for, as a module or as a file in `nodes/` (a chain file too, in a chain project).
#[must_use]
pub fn node_name_taken(fs: &dyn Fs, env: &Env, dir: &str, name: &str) -> bool {
    let loaded = load_modules(fs, env, dir).unwrap_or_default();
    loaded.contains_key(name)
        || fs.exists(&join(&[dir, NODES_DIR, &format!("{name}.k7x")]))
        || (crate::chain_item_edit::is_chain_project(fs, dir) && chain_taken(fs, dir, name))
}

/// What to call a new use of `module` when nobody said: `<lane>_<module>_<n>`, free across the whole project.
#[must_use]
pub fn free_node_name(fs: &dyn Fs, env: &Env, dir: &str, module: &str, lane: Option<&str>) -> String {
    instance_name(module, lane, &|n| node_name_taken(fs, env, dir, n))
}

/// Create one USE of a module: `nodes/<node>.k7x`, flat, or a loose chain item whose id comes back.
///
/// # Errors
/// An unknown category, io role or module, a taken or invalid name, or the file system's.
#[allow(clippy::too_many_arguments)]
pub fn create_node(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    category: &str,
    module: &str,
    node: &str,
    catalogue: &IndexMap<String, LoadedModule>,
    new_id: NewId,
) -> Res<String> {
    if !MODULE_TYPES.iter().any(|t| t.name == category) {
        return fail(unknown_category(category));
    }
    if !valid(node) {
        return fail(format!(
            "invalid node name '{node}' — letters, digits, '_' and '-' only"
        ));
    }
    if category == "io" {
        let Some((_, role)) = IO_MODULES.iter().find(|(n, _)| *n == module) else {
            return fail(format!(
                "unknown io module '{module}' — the app provides: {}",
                io_names()
            ));
        };
        if !is_io_source(role) {
            return fail(format!(
                "'{module}' is an OUTPUT — MIX, OUT, CUE, RENDER and VIDEO_OUT are built into every graph and take no node"
            ));
        }
        if load_modules(fs, env, dir).unwrap_or_default().contains_key(node) {
            return fail(format!("the name '{node}' is already a module or node in this project"));
        }
        return create_io_module_file(fs, dir, node, role, new_id);
    }
    let is_project = is_project_folder(fs, dir);
    let local_rel = if is_project {
        format!("{SHARED_MODULES_PATH}/{category}/{module}")
    } else {
        format!("modules/{category}/{module}")
    };
    let local_dir = crate::paths::resolve(&join(&[dir, &local_rel]));
    let local = module_file_for(fs, &local_dir, &local_rel, "module");
    let from_engine = if local.is_some() {
        None
    } else {
        catalogue.get(&engine_ref(module))
    };
    if local.is_none() && from_engine.is_none() {
        return fail(format!(
            "module '{category}/{module}' is neither in {} nor one the engine offers — create it first, or name one of the engine's",
            if is_project { "the workspace" } else { "this template" }
        ));
    }
    let rel = format!("{NODES_DIR}/{node}.k7x");
    let chains = crate::chain_item_edit::is_chain_project(fs, dir);
    if !chains && fs.exists(&join(&[dir, &rel])) {
        return fail(format!("{rel} already exists — refusing to overwrite"));
    }
    let prefix = if is_project { "@workspace" } else { "@project" };
    let reference = format!("{prefix}/{}", module_ref_of(category, module));
    let body = if local.is_some() {
        let header = [
            format!("{node}: one use of module \"{category}/{module}\"."),
            "It inherits everything that module says — state only what THIS use changes, e.g.".to_owned(),
            "  params({ \"Filter Freq\": 69 })".to_owned(),
        ];
        node_text(
            &header,
            Value::object(vec![("extends".to_owned(), Value::Str(reference))]),
        )?
    } else {
        let plugin = from_engine.and_then(|m| m.authored.plugin.as_ref());
        let header = [
            format!("{node}: one use of \"{module}\", a module the engine provides."),
            "The plugin is named here so the project says what it uses, whatever this machine".to_owned(),
            "has scanned. Add params to set it.".to_owned(),
        ];
        let field = |v: Option<&String>| Value::Str(v.cloned().unwrap_or_default());
        let plugin = Value::object(vec![
            ("type".to_owned(), field(plugin.and_then(|p| p.plugin_type.as_ref()))),
            ("name".to_owned(), field(plugin.and_then(|p| p.name.as_ref()))),
        ]);
        node_text(&header, Value::object(vec![("plugin".to_owned(), plugin)]))?
    };
    if chains {
        return Ok(create_chain_node(fs, dir, node, &body, new_id)?.0);
    }
    mkdirs(fs, &join(&[dir, NODES_DIR]))?;
    ws_write_text(fs, &join(&[dir, &rel]), &body)?;
    Ok(rel)
}

/// Copy a folder's files and sub folders through the `Fs`.
fn copy_tree(fs: &mut dyn Fs, from: &str, to: &str) -> Res<()> {
    // A link would be copied as the file or folder it leads to, out of the module, and a link to `..` never ends.
    if fs.is_link(from) {
        return fail(format!("'{from}' is a link: a clone copies no links"));
    }
    mkdirs(fs, to)?;
    for e in fs.list(from).map_err(|e| io_error(&e, "scandir", from))? {
        let (src, dst) = (join(&[from, &e.name]), join(&[to, &e.name]));
        if fs.is_link(&src) {
            return fail(format!("'{src}' is a link: a clone copies no links"));
        }
        if e.kind == Kind::Dir {
            copy_tree(fs, &src, &dst)?;
        } else {
            fs.copy(&src, &dst).map_err(|e| io_error(&e, "copyfile", &src))?;
        }
    }
    Ok(())
}

/// Clone a module folder under a new name, rewriting `name:` when the module states one.
///
/// # Errors
/// An unknown category, an invalid or equal name, a missing source, a taken destination, or the file system's.
pub fn clone_module_file(fs: &mut dyn Fs, dir: &str, category: &str, name: &str, new_name: &str) -> Res<String> {
    if !MODULE_TYPES.iter().any(|t| t.name == category) {
        return fail(unknown_category(category));
    }
    if !valid(name) {
        return fail(format!(
            "invalid module name '{name}' — letters, digits, '_' and '-' only"
        ));
    }
    if !valid(new_name) {
        return fail(format!(
            "invalid new name '{new_name}' — letters, digits, '_' and '-' only"
        ));
    }
    if name == new_name {
        return fail("source and destination names are the same");
    }
    let is_project = is_project_folder(fs, dir);
    let base = if is_project {
        format!("{SHARED_MODULES_PATH}/{category}")
    } else {
        format!("modules/{category}")
    };
    let local = |n: &str| crate::paths::resolve(&join(&[dir, &base, n]));
    let src = local(name);
    if !fs.exists(&src) || !fs.exists(&join(&[&src, "module.k7x"])) {
        return fail(format!("module '{category}/{name}' not found"));
    }
    let dest = local(new_name);
    let dest_rel = format!("{base}/{new_name}");
    if fs.exists(&dest) {
        return fail(format!("folder {dest_rel}/ already exists"));
    }
    copy_tree(fs, &src, &dest)?;
    // A `name` it states follows the folder; a failure to edit leaves the copy as it is.
    let _ = ws_edit(fs, &join(&[&dest, "module.k7x"]), EditOpts::default(), |doc| {
        if matches!(doc.get_in(&[PathSeg::Key("name".to_owned())])?, Some(Value::Str(_))) {
            doc.set_in(
                &[PathSeg::Key("name".to_owned())],
                &Value::Str(new_name.to_owned()),
                WriteOptions::default(),
            )?;
        }
        Ok(())
    });
    Ok(format!("{dest_rel}/module.k7x"))
}
