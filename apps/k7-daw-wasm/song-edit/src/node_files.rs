// node_files.rs — a node's files on disk: `nodes/<node>.k7x` and, beside it, the `<node>.vstpreset` holding its plugin state.
// A reader would get wrong that the preset is optional, but wherever the node file goes it goes too.

use crate::error::Res;
use crate::fs::Fs;
use crate::module_io::ModuleFile;
use crate::paths::{extname, join};
use crate::workspace_file::{ws_copy, ws_remove, ws_rename};

pub const NODES_DIR: &str = "nodes";
const PRESET_EXT: &str = ".vstpreset";

fn preset_rel(node: &str) -> String {
    format!("{NODES_DIR}/{node}{PRESET_EXT}")
}

/// Move a node's file and its preset to `to`, and the moves as `a → b` lines.
///
/// # Errors
/// The file system's.
pub fn move_node_files(fs: &mut dyn Fs, dir: &str, file: &ModuleFile, from: &str, to: &str) -> Res<Vec<String>> {
    let rel = format!("{NODES_DIR}/{to}{}", extname(&file.path));
    let mut moved = vec![format!("{} → {rel}", file.rel)];
    ws_rename(fs, &file.path, &join(&[dir, &rel]))?;
    let preset = join(&[dir, &preset_rel(from)]);
    if fs.exists(&preset) {
        ws_rename(fs, &preset, &join(&[dir, &preset_rel(to)]))?;
        moved.push(format!("{} → {}", preset_rel(from), preset_rel(to)));
    }
    Ok(moved)
}

/// Delete a node's file and its preset, and the rel paths removed.
///
/// # Errors
/// The file system's.
pub fn remove_node_files(fs: &mut dyn Fs, dir: &str, file: &ModuleFile, node: &str) -> Res<Vec<String>> {
    let mut removed = vec![file.rel.clone()];
    ws_remove(fs, &file.path)?;
    let preset = join(&[dir, &preset_rel(node)]);
    if fs.exists(&preset) {
        ws_remove(fs, &preset)?;
        removed.push(preset_rel(node));
    }
    Ok(removed)
}

/// Copy node `from` of `from_dir` (its file at `src`) to node `to` of `dir`, and the rel paths written.
///
/// # Errors
/// The file system's.
pub fn copy_node_files(
    fs: &mut dyn Fs,
    src: &str,
    from_dir: &str,
    from: &str,
    dir: &str,
    to: &str,
) -> Res<Vec<String>> {
    let rel = format!("{NODES_DIR}/{to}.k7x");
    let mut written = vec![rel.clone()];
    ws_copy(fs, src, &join(&[dir, &rel]))?;
    let preset = join(&[from_dir, &preset_rel(from)]);
    if fs.exists(&preset) {
        ws_copy(fs, &preset, &join(&[dir, &preset_rel(to)]))?;
        written.push(preset_rel(to));
    }
    Ok(written)
}
