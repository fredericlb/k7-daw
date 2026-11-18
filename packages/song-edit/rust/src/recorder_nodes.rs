// recorder_nodes.rs — a project's recorder NODES, taken out at open: the recorder module is gone, so each is deleted as `node delete` does.
// A reader would get wrong that the delete is `graph_edit`'s, whole: a chain project's recorder goes through its chain arm.

use crate::confine::Env;
use crate::error::Res;
use crate::fs::Fs;
use crate::graph_edit::delete_chain_node;
use crate::loader::load_modules;

/// The module ref recorder nodes extended.
pub const RECORDER_REF: &str = "@meta/audio/recorder";

/// Delete every node of `dir` extending the recorder; the names it took out.
///
/// # Errors
/// A read the source refuses, or a delete that fails.
pub fn drop_recorder_nodes(fs: &mut dyn Fs, env: &Env, dir: &str) -> Res<Vec<String>> {
    let names: Vec<String> = load_modules(fs, env, dir)?
        .into_iter()
        .filter(|(_, m)| m.authored.extends.as_deref() == Some(RECORDER_REF))
        .map(|(n, _)| n)
        .collect();
    for n in &names {
        delete_chain_node(fs, env, dir, n)?;
    }
    Ok(names)
}
