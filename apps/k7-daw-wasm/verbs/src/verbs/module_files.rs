// module_files.rs — the files `module new` and `node new` write, and how a typed module name is read back, after projectFiles.ts, nodeOnRef.ts and moduleCase.ts.
// A reader would get wrong case: it never picks the module, a typed name matching nothing keeps its spelling for the new one.

use super::shared::join;
use crate::host::{Host, Thrown};
use indexmap::IndexMap;
use k7x::edit::{StringifyOptions, Value, stringify};
use song::modules::LoadedModule;
use song::modules::types::{IO_MODULES, MODULE_TYPES};
use song_edit::Fs;
use song_edit::chain_create::create_chain_node;
use song_edit::chain_item_edit::is_chain_project;
use song_edit::loader::{NODES_DIR, load_module_refs};
use song_edit::module_io::{is_project_folder, module_file_for, read_module_file};
use song_edit::paths::resolve;
use song_edit::shared_folders::SHARED_MODULES_PATH;
use song_edit::workspace_doc::ws_write_text;
use song_edit::workspace_file::ws_exists;

fn valid(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// `localModuleFolder`: the workspace's `_modules/<category>/<name>`, or a template's own `modules/`.
pub(super) fn local_module_folder(fs: &dyn Fs, dir: &str, category: &str, name: &str) -> String {
    if is_project_folder(fs, dir) {
        resolve(&join(dir, &format!("{SHARED_MODULES_PATH}/{category}/{name}")))
    } else {
        join(dir, &format!("modules/{category}/{name}"))
    }
}

/// The project's module refs, which `module new`'s extends-ref must be one of.
pub(super) fn vocabulary(h: &Host) -> Result<IndexMap<String, LoadedModule>, Thrown> {
    Ok(load_module_refs(&*h.fs, &h.env, &h.song_path)?.modules.clone())
}

/// `existingExtends`: what a module already on disk extends, read from its own file, or "".
pub(super) fn existing_extends(fs: &dyn Fs, dir: &str, category: &str, name: &str) -> String {
    let folder = local_module_folder(fs, dir, category, name);
    let Some(file) = module_file_for(fs, &folder, &format!("{category}/{name}"), "module") else {
        return String::new();
    };
    match read_module_file(fs, &file).ok().and_then(|r| r.value) {
        Some(v) => match v.get("extends") {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        },
        None => String::new(),
    }
}

/// `createModuleFile`: `<category>/<module>/module.k7x`, bare or extending a known ref; its project-relative path.
pub(super) fn create_module_file(
    h: &mut Host,
    category: &str,
    module: &str,
    extends: Option<&str>,
) -> Result<String, String> {
    if !MODULE_TYPES.iter().any(|t| t.name == category) {
        let all: Vec<&str> = MODULE_TYPES.iter().map(|t| t.name).collect();
        return Err(format!(
            "unknown module category '{category}' — use one of: {}",
            all.join(", ")
        ));
    }
    if category == "io" {
        let roles: Vec<&str> = IO_MODULES.iter().map(|(n, _)| *n).collect();
        return Err(format!(
            "an io module is made with its instance — `node new io/<{}> <name>`",
            roles.join("|")
        ));
    }
    if !valid(module) {
        return Err(format!(
            "invalid module name '{module}' — letters, digits, '_' and '-' only"
        ));
    }
    if let Some(r) = extends {
        let known = vocabulary(h).map_err(|e| e.0)?;
        if !known.contains_key(r) {
            let mut refs: Vec<&String> = known.keys().collect();
            refs.sort();
            let has = if refs.is_empty() {
                " — this project has none".to_owned()
            } else {
                format!(
                    " — this project has: {}",
                    refs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                )
            };
            return Err(format!("unknown module '{r}'{has}"));
        }
    }
    let dir = h.song_path.clone();
    let is_project = is_project_folder(&*h.fs, &dir);
    let folder = local_module_folder(&*h.fs, &dir, category, module);
    let rel = if is_project {
        format!("{SHARED_MODULES_PATH}/{category}/{module}/module.k7x")
    } else {
        format!("modules/{category}/{module}/module.k7x")
    };
    if ws_exists(&*h.fs, &folder) {
        let place = if is_project {
            "in the workspace's _modules"
        } else {
            "here"
        };
        return Err(format!("{category}/{module} already exists {place}"));
    }
    let body = match extends {
        Some(r) => {
            let header = [format!("{module}: extends module \"{r}\".")];
            let v = Value::Map(vec![("extends".to_owned(), Value::Str(r.to_owned()))]);
            stringify(
                &v,
                &StringifyOptions {
                    header: &header,
                    inline: None,
                },
            )
            .map_err(|e| e.to_string())?
        }
        None => format!(
            "// {module}: a bare module. Give it a plugin inline, or extend another module, e.g.\n//   extends(\"@k7-core/instruments/fourosc\")\n"
        ),
    };
    h.fs.mkdir_all(&folder).map_err(|e| e.to_string())?;
    ws_write_text(h.fs, &join(&folder, "module.k7x"), &body).map_err(|e| e.to_string())?;
    Ok(rel)
}

/// `spelled`: the entry of `names` that is `typed` but for case, else `typed`.
fn spelled<'a>(typed: &str, names: impl IntoIterator<Item = &'a str>) -> String {
    names
        .into_iter()
        .find(|n| n.eq_ignore_ascii_case(typed))
        .map_or_else(|| typed.to_owned(), str::to_owned)
}

/// `canonicalTarget`: `<category>/<name>` or a ref in the project's own spelling, case never picking the module.
pub(super) fn canonical_target(h: &Host, target: &str, known: &IndexMap<String, LoadedModule>) -> String {
    if target.starts_with('@') {
        return spelled(target, known.keys().map(String::as_str));
    }
    let Some((raw_cat, raw_name)) = target
        .split_once('/')
        .filter(|(c, n)| !c.is_empty() && !n.is_empty() && !n.contains('/'))
    else {
        return target.to_owned();
    };
    let category = spelled(raw_cat, MODULE_TYPES.iter().map(|t| t.name));
    if category == "io" {
        return format!("io/{}", spelled(raw_name, IO_MODULES.iter().map(|(n, _)| *n)));
    }
    let r = spelled(&format!("{category}/{raw_name}"), known.keys().map(String::as_str));
    if r != format!("{category}/{raw_name}") {
        return r;
    }
    let folder = local_module_folder(&*h.fs, &h.song_path, &category, "_");
    let parent = folder.rsplit_once('/').map_or(folder.as_str(), |(p, _)| p).to_owned();
    let on_disk: Vec<String> =
        h.fs.list(&parent)
            .map(|l| l.into_iter().map(|e| e.name).collect())
            .unwrap_or_default();
    format!("{category}/{}", spelled(raw_name, on_disk.iter().map(String::as_str)))
}

/// `createNodeOn`: a node straight over a module ref, with no workspace module between; its file, or its id in a chain project.
pub(super) fn create_node_on(
    h: &mut Host,
    r: &str,
    node: &str,
    known: &IndexMap<String, LoadedModule>,
) -> Result<String, String> {
    if !valid(node) {
        return Err(format!(
            "invalid node name '{node}' — letters, digits, '_' and '-' only"
        ));
    }
    let m = known.get(r).filter(|m| m.node.is_none() && m.engine_of.is_none());
    let Some(m) = m else {
        return Err(format!("'{r}' is not a module this project can see"));
    };
    if m.kind == Some(song::modules::ModuleKind::Io) {
        return Err(format!("'{r}' is an io module — use node new io/<role>"));
    }
    let header = [format!(
        "{node}: one use of module \"{r}\" — state only what THIS use changes."
    )];
    let v = Value::Map(vec![("extends".to_owned(), Value::Str(r.to_owned()))]);
    let body = stringify(
        &v,
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let dir = h.song_path.clone();
    if is_chain_project(&*h.fs, &dir) {
        return create_chain_node(h.fs, &dir, node, &body, h.new_id)
            .map(|(id, _)| id)
            .map_err(|e| e.to_string());
    }
    let rel = format!("{NODES_DIR}/{node}.k7x");
    if h.fs.exists(&join(&dir, &rel)) {
        return Err(format!("{rel} already exists — refusing to overwrite"));
    }
    h.fs.mkdir_all(&join(&dir, NODES_DIR)).map_err(|e| e.to_string())?;
    ws_write_text(h.fs, &join(&dir, &rel), &body).map_err(|e| e.to_string())?;
    Ok(rel)
}
