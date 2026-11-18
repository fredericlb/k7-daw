//! listing.rs — the home page's two listings: a count per workspace, and every workspace with its projects, their
//! tags, surfaces, open one, last edit and demo flag. `listWorkspaces` and `workspaceRow`, read-only.
//! What a reader gets wrong: nothing is remembered here; the 5-second window belongs to the front (1670).

use std::collections::HashSet;
use std::fs;
use std::io::Read;

use super::demos::bundled_names;
use super::names::{child, folder_names_in, is_project_dir, project_dirs_under, workspace_dir};
use super::surfaces::project_surfaces;
use super::{EXAMPLES, Project, Row, Summary, ms};
use crate::filter::project_app;
use crate::libraries::Home;
use crate::paths::{basename, join, resolve, resolve_all};
use crate::space::{Declined, MAX_REL, js_trim};
use crate::tree::Limits;

/// The folder beside the workspaces that holds each one's open-project marker.
const MARKERS_DIR: &str = ".active-projects";
/// The marker a workspace once kept inside itself, which only a migration moves.
const LEGACY_MARKER: &str = ".studio-active";
/// The most of a marker that is read; past it the marker is no path anyone wrote.
const MAX_MARKER: u64 = 1 << 20;
/// A project's own file, where its tags are.
const PROJECT_FILE: &str = "project.k7x";

/// What a listing saw of the disk beside its answer: the root's mtime before the walk, and each workspace folder's.
#[derive(Clone, Debug, PartialEq)]
pub struct Stamps {
    pub root_mtime: f64,
    pub folders: Vec<(String, f64)>,
}

/// `listWorkspaces`: per workspace folder, whether it is one project and how many projects it holds.
pub fn summaries(home: &Home, limits: &Limits) -> Result<Vec<Summary>, Declined> {
    summaries_without(home, limits, &HashSet::new())
}

/// `summaries` without the folders `hidden` names: a saved server's that is not the active one (2650).
pub fn summaries_without(
    home: &Home,
    limits: &Limits,
    hidden: &HashSet<String>,
) -> Result<Vec<Summary>, Declined> {
    let dir = workspace_dir(home);
    let mut seen = 0;
    let mut out = Vec::new();
    for name in folder_names_in(&dir, limits, &mut seen)? {
        if hidden.contains(&name) {
            continue;
        }
        let abs = join(&[&dir, &name]);
        let single = is_project_dir(&abs);
        let count = if single {
            1
        } else {
            project_dirs_under(&abs, limits, &mut seen)?.len()
        };
        out.push(Summary {
            name,
            single,
            count,
        });
    }
    Ok(out)
}

/// The workspace's open project as the marker says (1633), resolved; `None` for nothing open or a library's.
fn active_of(root: &str, markers: &str, name: &str) -> Result<Option<String>, Declined> {
    let mut bytes = Vec::new();
    let read = fs::File::open(join(&[markers, name]))
        .and_then(|f| f.take(MAX_MARKER + 1).read_to_end(&mut bytes));
    if read.is_err() {
        // A legacy marker is moved by a read, a write the native side never makes.
        if fs::metadata(join(&[root, LEGACY_MARKER])).is_ok_and(|m| m.is_file()) {
            return Err(Declined::LegacyMarker);
        }
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let rel = js_trim(&text);
    if bytes.len() as u64 > MAX_MARKER || rel.len() > MAX_REL {
        return Err(Declined::LongPath);
    }
    Ok((!rel.is_empty()).then(|| resolve(root, rel)))
}

/// A project's tags from its `project.k7x`: none for a file that is missing or does not parse.
fn tags_of(dir: &str, limits: &Limits) -> Result<Vec<String>, Declined> {
    let file = join(&[dir, PROJECT_FILE]);
    let Ok(f) = fs::File::open(&file) else {
        return Ok(Vec::new());
    };
    let mut bytes = Vec::new();
    if f.take(limits.max_text + 1).read_to_end(&mut bytes).is_err() {
        return Ok(Vec::new());
    }
    if bytes.len() as u64 > limits.max_text {
        return Err(Declined::TooLarge);
    }
    tags_in(&String::from_utf8_lossy(&bytes))
}

/// `projectTagsOf` over a text: `project.tags`' non-empty strings; `Declined` for a text the TypeScript reads otherwise.
pub fn tags_in(text: &str) -> Result<Vec<String>, Declined> {
    let bump = k7x::Bump::new();
    let Ok(parsed) = k7x::parse_tree_lossless(text, &bump) else {
        return Ok(Vec::new());
    };
    let Some(tree) = parsed else {
        return Err(Declined::LossyText);
    };
    let Some(project) = tree.entries.iter().rev().find(|e| e.key == "project") else {
        return Ok(Vec::new());
    };
    let mut json = String::new();
    k7x::json::value_json(&project.value, &mut json);
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&json)
    else {
        return Ok(Vec::new());
    };
    Ok(match map.get("tags") {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|t| t.as_str().filter(|s| !s.is_empty()).map(String::from))
            .collect(),
        _ => Vec::new(),
    })
}

fn newest_in(dir: &str, left: u32, newest: &mut Option<f64>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = Vec::new();
    for e in read {
        let Ok(e) = e else { return };
        entries.push((
            e.file_name().to_string_lossy().into_owned(),
            e.file_type().is_ok_and(|t| t.is_dir()),
        ));
    }
    for (name, is_dir) in entries {
        // Dot-entries are the replica's own bookkeeping, not an edit anybody made.
        if name.starts_with('.') {
            continue;
        }
        let at = child(dir, &name);
        if is_dir {
            if left > 1 {
                newest_in(&at, left - 1, newest);
            }
            continue;
        }
        if let Ok(m) = fs::metadata(&at) {
            let t = ms(m.modified());
            if newest.is_none_or(|n| t > n) {
                *newest = Some(t);
            }
        }
    }
}

/// `projectEditedAt`: the newest mtime among a project's files and its sub-folders' files, rounded as `Math.round`.
pub fn edited_at(dir: &str) -> Option<f64> {
    let mut newest = None;
    newest_in(dir, 2, &mut newest);
    newest.map(k7_js::number::math_round)
}

/// The names of the bundled demos, for the flag.
pub fn demo_names(demos: Option<&str>) -> HashSet<String> {
    demos
        .map(bundled_names)
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn project_at(
    ws: &str,
    abs: &str,
    path: &str,
    active: Option<&str>,
    demos: &HashSet<String>,
    limits: &Limits,
) -> Result<Option<Project>, Declined> {
    // `.` names the workspace folder itself; either way the NAME decides.
    let named = if path == "." {
        ws.to_string()
    } else {
        basename(path)
    };
    let Some(app) = project_app(&named) else {
        return Ok(None);
    };
    let at = if path == "." {
        abs.to_string()
    } else {
        join(&[abs, path])
    };
    Ok(Some(Project {
        path: path.to_string(),
        app,
        tags: tags_of(&at, limits)?,
        surfaces: project_surfaces(&at)?,
        active: active.is_some_and(|a| a == resolve_all(&[&at])),
        edited_at: edited_at(&at),
        demo: ws == EXAMPLES && !path.contains('/') && demos.contains(path),
    }))
}

/// `listMachineWorkspaces` without its memo: every workspace with its projects, walked and read afresh, and the
/// stamps of the folders it walked (the root's read before the walk).
pub fn rows(
    home: &Home,
    demos: Option<&str>,
    limits: &Limits,
) -> Result<(Vec<Row>, Stamps), Declined> {
    rows_without(home, demos, limits, &HashSet::new())
}

/// `rows` without the folders `hidden` names, which are neither walked nor stamped (2650).
pub fn rows_without(
    home: &Home,
    demos: Option<&str>,
    limits: &Limits,
    hidden: &HashSet<String>,
) -> Result<(Vec<Row>, Stamps), Declined> {
    let dir = workspace_dir(home);
    let markers = join(&[&dir, MARKERS_DIR]);
    let demo_set = demo_names(demos);
    let mut stamps = Stamps {
        root_mtime: fs::metadata(&dir).map_or(0.0, |m| ms(m.modified())),
        folders: Vec::new(),
    };
    let mut seen = 0;
    let mut out = Vec::new();
    for name in folder_names_in(&dir, limits, &mut seen)? {
        if hidden.contains(&name) {
            continue;
        }
        let abs = join(&[&dir, &name]);
        stamps.folders.push((
            name.clone(),
            fs::metadata(&abs).map_or(0.0, |m| ms(m.modified())),
        ));
        let active = active_of(&abs, &markers, &name)?;
        let single = is_project_dir(&abs);
        let paths = if single {
            vec![".".to_string()]
        } else {
            project_dirs_under(&abs, limits, &mut seen)?
        };
        let mut projects = Vec::new();
        for path in &paths {
            if let Some(p) = project_at(&name, &abs, path, active.as_deref(), &demo_set, limits)? {
                projects.push(p);
            }
        }
        out.push(Row {
            name,
            single,
            projects,
        });
    }
    Ok((out, stamps))
}
