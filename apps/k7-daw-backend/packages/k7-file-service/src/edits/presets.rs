//! presets.rs — `GET /presets`: a node's presets listed from every scope, read through song-edit over the mounts the project sees.
//! What a reader gets wrong: nothing is written, so the project is mounted for reading only and no lock is held.

use serde_json::{Value, json};
use song_edit::presets::list_node_presets;

use super::{StagedFs, env_for};
use crate::libraries::Home;

/// The answer's JSON for `node` of `project`, or the sentence a 400 carries.
///
/// # Errors
/// A mount that is no absolute path, a node the project lacks, or a scope with no library.
pub fn node_presets(
    home: &Home,
    code_root: &str,
    project: &str,
    node: &str,
) -> Result<Value, String> {
    let env = env_for(home, code_root, project);
    let mut reads = vec![
        env.meta_root.clone(),
        home.libraries.clone(),
        home.account_home(),
    ];
    reads.extend(env.overlays.iter().map(|(_, root)| root.clone()));
    // The workspace holds the project, its `_presets/` and its pins.
    let workspace = std::path::Path::new(project)
        .parent()
        .map(|p| p.display().to_string())
        .ok_or_else(|| String::from("no workspace"))?;
    reads.push(workspace);
    let fs = StagedFs::reading::<&str, _>(&[], &reads).map_err(|e| e.to_string())?;
    let p =
        list_node_presets(&fs, &env, &super::posix(project), node).map_err(|e| e.to_string())?;
    Ok(json!({
        "ref": p.reference,
        "presets": p.presets.iter().map(|e| json!({ "scope": e.scope, "name": e.name })).collect::<Vec<_>>(),
        "scopes": p.scopes,
    }))
}
