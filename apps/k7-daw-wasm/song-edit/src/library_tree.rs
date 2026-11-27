// library_tree.rs — the LIBRARIES tab's rows: one row per module level, each account library as a row with its families, and the path rules over them.
// A reader would get wrong that nothing here sorts: the page hands every list in the order people read (a collator), and rows keep it.
// The levels come from `GET /module-libraries`, outside the active project; why the tab exists: apps/k7-daw-frontend/docs/sharedModules.md.

use crate::sugar::is_doc_file;
use serde::Deserialize;
use serde_json::{Value, json};
use song::modules::names::{FIXED_NAMESPACES, LIBRARIES_DIR, LIBRARY_FILE_FAMILIES, SHARED_MODULES_DIR};
use song::modules::types::is_library_name;

/// One module level the file service answers, as the tree reads it: `nodes` and the file folders are file rows as the page holds them.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct LibraryLevel {
    pub ns: String,
    pub path: String,
    pub read_only: Option<bool>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "Array<unknown>"))]
    pub nodes: Vec<Value>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[cfg_attr(feature = "ts", ts(type = "Array<unknown>"))]
    pub samples: Option<Vec<Value>>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[cfg_attr(feature = "ts", ts(type = "Array<unknown>"))]
    pub irs: Option<Vec<Value>>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[cfg_attr(feature = "ts", ts(type = "Array<unknown>"))]
    pub wavetables: Option<Vec<Value>>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[cfg_attr(feature = "ts", ts(type = "Array<unknown>"))]
    pub models: Option<Vec<Value>>,
}

/// A library chain, as the tab lists it: `ref` is `@<library>/<path under chains/>`, `path` its file or a folder chain's folder.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct LibraryChainRow {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub path: String,
    /// A FOLDER chain's node files; a chain file holds its nodes, so none.
    pub nodes: Option<Vec<String>>,
}

/// An assembly template of the library: `path` its folder, `file` the `assembly.k7x` the builder opens.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LibraryAssemblyRow {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub path: String,
    pub file: String,
}

/// How a level is written wherever a person reads one: in a ref, and on a tree row.
fn ns_label(ns: &str) -> String {
    format!("@{ns}")
}

fn dir_row(name: &str, path: &str, children: Vec<Value>) -> Value {
    json!({"name": name, "path": path, "dir": true, "children": children})
}

fn file_row(name: &str, path: &str) -> Value {
    json!({"name": name, "path": path, "dir": false})
}

/// A ref as its library's row shows it: the `@<library>/` is the row it sits under.
fn within_library<'a>(ns: &str, r: &'a str) -> &'a str {
    r.strip_prefix(&format!("{}/", ns_label(ns))).unwrap_or(r)
}

/// The repository's own folder: a library's module level is always its `modules/`, so its parent.
fn repository_path(l: &LibraryLevel) -> String {
    match l.path.rfind('/') {
        Some(at) if at > 0 => l.path[..at].to_owned(),
        _ => format!("{LIBRARIES_DIR}/{}", l.ns),
    }
}

fn repository_row(l: &LibraryLevel, chains: &[&LibraryChainRow], assemblies: &[&LibraryAssemblyRow]) -> Value {
    let root = repository_path(l);
    let list = |files: &Option<Vec<Value>>| files.clone().unwrap_or_default();
    let chain_rows = chains
        .iter()
        .map(|c| {
            let name = within_library(&l.ns, &c.ref_);
            // A chain file is a leaf; a folder chain OPENS, its node files under it, even with none yet.
            if is_doc_file(&c.path) {
                return file_row(name, &c.path);
            }
            let nodes = c
                .nodes
                .iter()
                .flatten()
                .map(|n| file_row(n, &format!("{}/nodes/{n}", c.path)));
            dir_row(name, &c.path, nodes.collect())
        })
        .collect();
    let assembly_rows = assemblies
        .iter()
        .map(|a| file_row(within_library(&l.ns, &a.ref_), &a.path))
        .collect();
    let family = |name: &str, children: Vec<Value>| dir_row(name, &format!("{root}/{name}"), children);
    dir_row(
        &ns_label(&l.ns),
        &root,
        vec![
            // The module level's own path, untouched: it is what `root_of` resolves 'New module…' against.
            dir_row("modules", &l.path, l.nodes.clone()),
            family("chains", chain_rows),
            family("assemblies", assembly_rows),
            // The library's shared SOUNDS, then the tables a wavetable instrument reads, then its amp MODELS.
            family("samples", list(&l.samples)),
            family("irs", list(&l.irs)),
            family("wavetables", list(&l.wavetables)),
            family("models", list(&l.models)),
        ],
    )
}

/// The libraries as tree rows: `@workspace` and `@engine` are module LEVELS, every other level an account library REPOSITORY.
pub fn library_tree(
    levels: &[LibraryLevel],
    chains: &[LibraryChainRow],
    assemblies: &[LibraryAssemblyRow],
) -> Vec<Value> {
    levels
        .iter()
        .map(|l| {
            if FIXED_NAMESPACES.contains(&l.ns.as_str()) {
                // The real path, workspace-relative; `@engine` has none, its label being all that identifies its row.
                let path = if l.path.is_empty() {
                    ns_label(&l.ns)
                } else {
                    l.path.clone()
                };
                return dir_row(&ns_label(&l.ns), &path, l.nodes.clone());
            }
            let prefix = format!("{}/", ns_label(&l.ns));
            let mine_chains: Vec<_> = chains.iter().filter(|c| c.ref_.starts_with(&prefix)).collect();
            let mine_assemblies: Vec<_> = assemblies.iter().filter(|a| a.ref_.starts_with(&prefix)).collect();
            repository_row(l, &mine_chains, &mine_assemblies)
        })
        .collect()
}

/// Which of a library's FILE folders this row is or lies in (`samples/`, `irs/`, `wavetables/`, `models/`): the only places a drop goes.
pub fn file_folder_of(path: &str) -> Option<&'static str> {
    let rest = path.strip_prefix(LIBRARIES_DIR)?.strip_prefix('/')?;
    let (library, rest) = rest.split_once('/')?;
    if library.is_empty() {
        return None;
    }
    let family = rest.split('/').next().unwrap_or("");
    LIBRARY_FILE_FAMILIES.iter().map(|(f, _)| *f).find(|f| *f == family)
}

/// Every path under a READ-ONLY level, plus the level's own row: `@engine` and only it, a scanned plugin having no folder.
pub fn read_only_paths(levels: &[LibraryLevel]) -> Vec<String> {
    fn walk(nodes: &[Value], out: &mut Vec<String>) {
        for n in nodes {
            if let Some(p) = n.get("path").and_then(Value::as_str) {
                out.push(p.to_owned());
            }
            if let Some(children) = n.get("children").and_then(Value::as_array) {
                walk(children, out);
            }
        }
    }
    let mut out = Vec::new();
    for l in levels.iter().filter(|l| l.read_only == Some(true)) {
        out.push(if l.path.is_empty() {
            ns_label(&l.ns)
        } else {
            l.path.clone()
        });
        walk(&l.nodes, &mut out);
    }
    out
}

/// The index of the library ROOT a path belongs to, or none under a read-only level: what 'New folder…' and 'New module…' resolve against.
pub fn root_of(levels: &[LibraryLevel], path: &str) -> Option<usize> {
    levels.iter().position(|l| {
        l.read_only != Some(true)
            && !l.path.is_empty()
            && (path == l.path || path.strip_prefix(&l.path).is_some_and(|r| r.starts_with('/')))
    })
}

/// `<type>/<sub>` under a level, or none when there is no module there to name.
fn ref_of(ns: &str, segs: &[&str]) -> Option<String> {
    (segs.len() >= 2).then(|| format!("{}/{}", ns_label(ns), segs.join("/")))
}

/// The ref a chain writes for one of a SHARED module's files: a FILE path always, so the last segment is the one to drop.
pub fn library_module_ref(file: &str) -> Option<String> {
    let parts: Vec<&str> = file.split('/').filter(|s| !s.is_empty()).collect();
    // The segments from `from` up to the last one, which is the file's own name.
    let folders = |from: usize| parts.get(from..parts.len().saturating_sub(1)).unwrap_or(&[]).to_vec();
    if let Some(at) = parts.iter().position(|p| *p == LIBRARIES_DIR) {
        let name = parts.get(at + 1).copied().unwrap_or("");
        return if is_library_name(name) && parts.get(at + 2) == Some(&"modules") {
            ref_of(name, &folders(at + 3))
        } else {
            None
        };
    }
    let shared = parts.iter().position(|p| *p == SHARED_MODULES_DIR)?;
    ref_of("workspace", &folders(shared + 1))
}

/// Is this workspace path inside one of the ACCOUNT's libraries? Only `_libraries/…`; a `@workspace` module forks instead.
pub fn is_library_path(path: &str) -> bool {
    path.split('/').any(|p| p == LIBRARIES_DIR)
}

/// WHICH library a `_libraries/<name>/…` path is in, the one a Save publishes, or none.
pub fn library_of_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let at = parts.iter().position(|p| *p == LIBRARIES_DIR)?;
    let name = parts.get(at + 1).copied().unwrap_or("");
    is_library_name(name).then(|| name.to_owned())
}

#[cfg(test)]
mod tests;
