// base_preset.rs — a module's named BASES: the whole-state files in its `base/` folder, and which one a module or node chose.
// A reader would get wrong that a `base:` never leaves `base/`: a path climbing out of it names nothing, not another file.

use crate::fs::Fs;
use crate::js_text::is_js_space;
use crate::list_files::{ListOpts, list_files_under};
use crate::paths::{is_within, join, resolve};
use k7x::edit::Value;

pub const MODULE_BASES_DIR: &str = "base";
const BASE_EXT: &str = ".vstpreset";
const BASE_EXTS: [&str; 2] = [BASE_EXT, ".fxp"];
const MAX_DEPTH: usize = 3;
const MAX_ANCESTRY: usize = 16;

fn base_name(file: &str) -> Option<&str> {
    BASE_EXTS.iter().find_map(|e| {
        let cut = file.len().checked_sub(e.len())?;
        file.get(cut..)
            .is_some_and(|t| t.eq_ignore_ascii_case(e))
            .then(|| &file[..cut])
    })
}

/// The bases in ONE folder, without the extension, subfolders as `<dir>/<name>`, by code point.
#[must_use]
pub fn bases_in(fs: &dyn Fs, folder: Option<&str>) -> Vec<String> {
    let Some(folder) = folder.filter(|f| !f.is_empty()) else {
        return vec![];
    };
    let opts = ListOpts {
        max_depth: MAX_DEPTH,
        ..ListOpts::default()
    };
    let mut names: Vec<String> = list_files_under(fs, &join(&[folder, MODULE_BASES_DIR]), &opts)
        .iter()
        .filter_map(|f| base_name(f).map(str::to_owned))
        .collect();
    // UTF-8 byte order is code point order.
    names.sort();
    names
}

/// The file a `base:` names, or None so the caller falls back to the module's default.
#[must_use]
pub fn base_preset_in(fs: &dyn Fs, folder: &str, base: &Value) -> Option<String> {
    let Value::Str(b) = base else { return None };
    let b = b.trim_matches(is_js_space);
    if b.is_empty() {
        return None;
    }
    let root = resolve(&join(&[folder, MODULE_BASES_DIR]));
    for ext in BASE_EXTS {
        let file = resolve(&format!("{root}/{b}{ext}"));
        if is_within(&root, &file) && !is_within(&file, &root) && fs.exists(&file) {
            return Some(file);
        }
    }
    None
}

/// Whether a resolved base is a Surge XT patch.
#[must_use]
pub fn is_surge_patch(file: Option<&str>) -> bool {
    file.is_some_and(|f| f.len() >= 4 && f.get(f.len() - 4..).is_some_and(|t| t.eq_ignore_ascii_case(".fxp")))
}

/// The state file a module loads: the chosen `base:`, then the node's `.vstpreset`, then the module's own.
#[must_use]
pub fn preset_for(fs: &dyn Fs, folder: &str, leaf: &str, node: Option<&str>, base: &Value) -> Option<String> {
    base_preset_in(fs, folder, base).or_else(|| {
        [
            join(&[folder, &format!("{}{BASE_EXT}", node.unwrap_or(leaf))]),
            join(&[folder, &format!("{leaf}{BASE_EXT}")]),
        ]
        .into_iter()
        .find(|p| fs.exists(p))
    })
}

/// Just enough of a module to find its folder and its parent.
#[derive(Clone, Debug, Default)]
pub struct BaseOwner {
    pub extends: Option<String>,
    pub folder_abs: Option<String>,
}

/// Every base this module may be set to, its ancestors' included, each name once.
#[must_use]
pub fn module_base_choices(
    fs: &dyn Fs,
    module: Option<&BaseOwner>,
    refs: &dyn Fn(&str) -> Option<BaseOwner>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |folder: Option<&str>| {
        for b in bases_in(fs, folder) {
            if !out.contains(&b) {
                out.push(b);
            }
        }
    };
    add(module.and_then(|m| m.folder_abs.as_deref()));
    let mut r = module.and_then(|m| m.extends.clone());
    for _ in 0..MAX_ANCESTRY {
        let Some(name) = r.filter(|n| !n.is_empty()) else { break };
        let parent = refs(&name);
        add(parent.as_ref().and_then(|p| p.folder_abs.as_deref()));
        r = parent.and_then(|p| p.extends);
    }
    out
}
