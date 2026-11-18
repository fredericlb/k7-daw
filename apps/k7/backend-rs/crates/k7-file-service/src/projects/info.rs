//! info.rs — what a workspace holds and which project is open (`workspaceInfo`), and the poll's `{name, root}`.
//! What a reader gets wrong: the list is walked afresh here, never from a memo (2032).

use std::fs;
use std::io::Read;

use serde_json::{Value, json};

use super::Info;
use crate::paths::{basename, join};
use crate::rel::to_rel;
use crate::space::{Declined, Space};
use crate::tree::Limits;
use crate::workspaces::EXAMPLES;
use crate::workspaces::listing::demo_names;
use crate::workspaces::names::{is_project_dir, project_dirs_under};

/// A project's own file, where its name is.
const PROJECT_FILE: &str = "project.k7x";

/// `workspaceInfo` with `projects` walked now and the demo flag by 1f's rule; `demos` is the bundled demos' folder.
pub fn workspace_info(
    space: &Space<'_>,
    demos: Option<&str>,
    limits: &Limits,
) -> Result<Info, Declined> {
    let active = space.active()?.map(|a| to_rel(space, &a));
    info_with(space, demos, limits, active)
}

/// `workspace_info` as node's open answered a library project: a marker naming `_libraries/` resolves to no project in the workspace.
pub fn workspace_info_opened(
    space: &Space<'_>,
    demos: Option<&str>,
    limits: &Limits,
) -> Result<Info, Declined> {
    match workspace_info(space, demos, limits) {
        Err(Declined::LibraryActive) => info_with(space, demos, limits, None),
        other => other,
    }
}

fn info_with(
    space: &Space<'_>,
    demos: Option<&str>,
    limits: &Limits,
    active: Option<String>,
) -> Result<Info, Declined> {
    let root = space.root().to_string();
    let root_is_project = is_project_dir(&root);
    let mut projects = project_dirs_under(&root, limits, &mut 0)?;
    if root_is_project {
        projects.insert(0, ".".into());
    }
    let active_is_demo = basename(&root) == EXAMPLES
        && active
            .as_deref()
            .is_some_and(|a| !a.contains('/') && demo_names(demos).contains(a));
    Ok(Info {
        root,
        root_is_project,
        active,
        projects,
        active_is_demo,
    })
}

/// `project.name` of a `project.k7x` text when it is a string; `Declined` for a text the TypeScript reads otherwise.
fn name_in(text: &str) -> Result<Option<String>, Declined> {
    let bump = k7x::Bump::new();
    let Ok(parsed) = k7x::parse_tree_lossless(text, &bump) else {
        return Ok(None);
    };
    let Some(tree) = parsed else {
        return Err(Declined::LossyText);
    };
    let Some(project) = tree.entries.iter().rev().find(|e| e.key == "project") else {
        return Ok(None);
    };
    let mut json = String::new();
    k7x::json::value_json(&project.value, &mut json);
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&json) else {
        return Ok(None);
    };
    Ok(map.get("name").and_then(Value::as_str).map(String::from))
}

/// `projectInfoOf(active ?? root)`: `{"name":…,"root":…}` (2070).
pub fn project_info(space: &Space<'_>) -> Result<String, Declined> {
    let dir = space.active()?.unwrap_or_else(|| space.root().to_string());
    let mut bytes = Vec::new();
    let max = crate::tree::Limits::default().max_text;
    let read = fs::File::open(join(&[&dir, PROJECT_FILE]))
        .and_then(|f| f.take(max + 1).read_to_end(&mut bytes));
    let named = if read.is_err() {
        None
    } else if bytes.len() as u64 > max {
        return Err(Declined::TooLarge);
    } else {
        name_in(&String::from_utf8_lossy(&bytes))?
    };
    let name = named.unwrap_or_else(|| basename(&dir));
    Ok(json!({ "name": name, "root": dir }).to_string())
}
