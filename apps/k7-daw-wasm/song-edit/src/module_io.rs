// module_io.rs — locate and read the .k7x files a project is made of: module files, per-file entities and the root files.
// A reader would get wrong the listing order: the TypeScript sorts `[base, name]` pairs as strings, so `a+` comes before `a`.

use crate::error::Res;
use crate::fs::Fs;
use crate::paths::{basename_without, extname, join};
use crate::workspace_file::{K7X_EXT, k7x_data, ws_exists, ws_read};
use k7x::edit::Value;
use song::Kind;
use std::collections::BTreeMap;

/// A file that exists: its absolute path, and its path relative to the project, which an error message names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleFile {
    pub path: String,
    pub rel: String,
}

/// The module folder's OWN basenames: anything else there is a node.
pub const UI_BASE: &str = "ui";
pub const PRESETS_BASE: &str = "presets";
pub const CONTROLLER_BASE: &str = "controller";
pub const DSP_BASE: &str = "dsp";
pub const MODULE_OWN_BASES: [&str; 5] = ["module", UI_BASE, PRESETS_BASE, CONTROLLER_BASE, DSP_BASE];

/// The module file for `base` ("module", or a node's name) in `folder`, or None; `rel_dir` builds the rel.
#[must_use]
pub fn module_file_for(fs: &dyn Fs, folder: &str, rel_dir: &str, base: &str) -> Option<ModuleFile> {
    let path = join(&[folder, &format!("{base}{K7X_EXT}")]);
    ws_exists(fs, &path).then(|| ModuleFile {
        path,
        rel: format!("{rel_dir}/{base}{K7X_EXT}"),
    })
}

/// The .k7x files a folder holds, by basename, as the TypeScript sorts them.
fn doc_files_in(fs: &dyn Fs, folder: &str) -> Vec<(String, String)> {
    // Not there, or unreadable: every caller treats that as empty.
    let Ok(entries) = fs.list(folder) else { return vec![] };
    let mut by_base: BTreeMap<String, String> = BTreeMap::new();
    for e in entries {
        if !extname(&e.name).eq_ignore_ascii_case(K7X_EXT) || e.kind == Kind::Dir {
            continue;
        }
        by_base.insert(basename_without(&e.name, extname(&e.name)).to_owned(), e.name);
    }
    let mut out: Vec<(String, String)> = by_base.into_iter().collect();
    // `[...map].sort()` compares the pairs as `base,name` in UTF-16 order.
    out.sort_by_key(|(b, n)| format!("{b},{n}").encode_utf16().collect::<Vec<u16>>());
    out
}

/// The NODE files beside a module's generic: every `<node>.k7x` that is not one of `MODULE_OWN_BASES`.
#[must_use]
pub fn node_files(fs: &dyn Fs, folder: &str, rel_dir: &str) -> Vec<(String, ModuleFile)> {
    doc_files_in(fs, folder)
        .into_iter()
        .filter(|(base, _)| !MODULE_OWN_BASES.contains(&base.as_str()))
        .map(|(node, name)| {
            (
                node,
                ModuleFile {
                    path: join(&[folder, &name]),
                    rel: format!("{rel_dir}/{name}"),
                },
            )
        })
        .collect()
}

/// The file for entity `base`: the flat per-file kinds and the root files.
#[must_use]
pub fn entity_file_for(fs: &dyn Fs, folder: &str, rel_dir: &str, base: &str) -> Option<ModuleFile> {
    module_file_for(fs, folder, rel_dir, base)
}

/// Every entity id in `folder`: the ids ARE the filenames.
#[must_use]
pub fn entity_files(fs: &dyn Fs, folder: &str, rel_dir: &str) -> Vec<(String, ModuleFile)> {
    doc_files_in(fs, folder)
        .into_iter()
        .map(|(id, name)| {
            (
                id,
                ModuleFile {
                    path: join(&[folder, &name]),
                    rel: format!("{rel_dir}/{name}"),
                },
            )
        })
        .collect()
}

/// One module file read: the data as authored, or why not.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadModule {
    pub value: Option<Value>,
    pub issues: Vec<String>,
}

/// Read one module file; the issues are `song`'s findings spelled `<rel>:<line>:<col>: <message>`.
///
/// # Errors
/// The read's own failure: the file is not there.
pub fn read_module_file(fs: &dyn Fs, file: &ModuleFile) -> Res<ReadModule> {
    let text = ws_read(fs, &file.path)?;
    if let Err(f) = song::modules::decode_module(&file.rel, &text) {
        return Ok(ReadModule {
            value: None,
            issues: vec![format!(
                "{}:{}:{}: {}",
                file.rel,
                f.line,
                f.col,
                zod_wording(&f.message)
            )],
        });
    }
    let mut value = k7x_data(&file.path, &text)?;
    normalize(&mut value);
    // The default is applied on the way OUT and never over an inherited type.
    if let Value::Map(m) = &mut value
        && !["type", "io", "extends"].iter().any(|k| m.iter().any(|(x, _)| x == k))
    {
        m.push(("type".to_owned(), Value::Str("plugin".to_owned())));
    }
    Ok(ReadModule {
        value: Some(value),
        issues: vec![],
    })
}

/// An unknown key spelled as the schema's issue: `<key>: Unrecognized key: "<key>"`.
fn zod_wording(message: &str) -> String {
    match message.strip_prefix("unknown key `").and_then(|r| r.split_once('`')) {
        Some((key, _)) => format!("{key}: Unrecognized key: \"{key}\""),
        None => message.to_owned(),
    }
}

/// The schema's own defaults and spellings: a rack's flavour and pads, and a bare-number fade as `{ over }`.
fn normalize(value: &mut Value) {
    let Value::Map(m) = value else { return };
    for (k, v) in m.iter_mut() {
        match (k.as_str(), v) {
            ("rack", Value::Map(r)) => {
                for (key, default) in [
                    ("rack_type", Value::Str("samples".to_owned())),
                    ("symbols", Value::Map(vec![])),
                ] {
                    if !r.iter().any(|(x, _)| x == key) {
                        r.push((key.to_owned(), default));
                    }
                }
            }
            ("events", Value::List(events)) => {
                for e in events
                    .iter_mut()
                    .filter_map(|e| if let Value::Map(f) = e { Some(f) } else { None })
                {
                    for (_, fv) in e.iter_mut().filter(|(fk, _)| fk == "fadeIn" || fk == "fadeOut") {
                        if let Value::Num(n) = fv {
                            *fv = Value::Map(vec![("over".to_owned(), Value::Num(*n))]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Is `dir` a k7 project: does it hold a `project.k7x`?
#[must_use]
pub fn is_project_folder(fs: &dyn Fs, dir: &str) -> bool {
    entity_file_for(fs, dir, "", "project").is_some()
}
