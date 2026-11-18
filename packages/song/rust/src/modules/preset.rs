// modules/preset.rs — the state file a module loads: its chosen `base`, a node's own `.vstpreset`, then the module's.
// A reader would get wrong that a preset is only STAT'ed: it is a binary for the engine, so a bundle holds it by its length.

use super::assets::Files;
use crate::files::scalar::js_space;
use crate::source::SourceError;
use crate::source::path::{join, join_within};

/// The folder a module keeps its named bases in, and the extensions a base file carries.
pub const MODULE_BASES_DIR: &str = "base";
const BASE_EXT: &str = ".vstpreset";
const BASE_EXTS: [&str; 2] = [BASE_EXT, ".fxp"];

/// The file a `base:` names, probed by stat, never one outside `base/`; None sends the caller to the module's default.
pub fn base_preset_in(files: &mut Files, folder: &str, base: &str) -> Result<Option<String>, SourceError> {
    let name = base.trim_matches(js_space);
    if name.is_empty() || name.starts_with('/') {
        return Ok(None);
    }
    let root = join(folder, MODULE_BASES_DIR);
    for ext in BASE_EXTS {
        let Some(file) = join_within(&root, &format!("{name}{ext}")) else {
            continue;
        };
        if file != root && files.is_file(&file)? {
            return Ok(Some(file));
        }
    }
    Ok(None)
}

/// Whether a resolved base is a Surge XT patch, which the engine loads as one rather than as a `.vstpreset`.
pub fn is_surge_patch(file: Option<&str>) -> bool {
    file.is_some_and(|f| f.to_lowercase().ends_with(".fxp"))
}

/// The state file a module loads: the chosen `base:`, then `<node or leaf>.vstpreset`, then `<leaf>.vstpreset`.
pub fn preset_for(
    files: &mut Files,
    folder: &str,
    leaf: &str,
    node: Option<&str>,
    base: Option<&str>,
) -> Result<Option<String>, SourceError> {
    if let Some(base) = base
        && let Some(hit) = base_preset_in(files, folder, base)?
    {
        return Ok(Some(hit));
    }
    for stem in [node.unwrap_or(leaf), leaf] {
        let file = join(folder, &format!("{stem}{BASE_EXT}"));
        if files.is_file(&file)? {
            return Ok(Some(file));
        }
    }
    Ok(None)
}
