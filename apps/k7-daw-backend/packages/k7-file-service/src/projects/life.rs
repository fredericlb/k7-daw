//! life.rs — a project renamed or recycled: both judged in the TypeScript's order, then the disk's part.
//! What a reader gets wrong: `was_active` differs on purpose: a rename's is ancestry, a recycle's is equality (2042).

use std::time::Duration;

use serde_json::json;

use super::ProjError;
use super::marker::active_strict;
use super::open::{SUFFIXES, is_project, not_a_project, own_spelling};
use crate::entries::Probe;
use crate::entries::apply::{Renamer, make_folder, recycle_target_in, rename_with_retry};
use crate::entries::guards::{RECYCLE_DIR, guard_shared, is_recycled};
use crate::entries::plan::clean_name;
use crate::filter::project_app;
use crate::libraries::library_of;
use crate::paths::{basename, dirname, is_within, join, relative};
use crate::rel::to_rel;
use crate::space::Space;

/// A move judged: where it goes, whether the open project moves with it, and the route's body.
/// `active_to` is a rename's open project at its new path: the folder itself, or the inner project below it (F3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovePlan {
    pub abs: String,
    pub dest: String,
    pub was_active: bool,
    pub active_to: Option<String>,
    pub answer: String,
}

/// `renameProject`'s guards (2041); `None` is the name it has (answer with `kept_answer`).
pub fn plan_rename(
    space: &Space<'_>,
    rel: &str,
    name: &str,
    probe: &dyn Probe,
) -> Result<Option<MovePlan>, ProjError> {
    let abs = own_spelling(space, &space.safe_path(rel, probe)?, probe)?;
    if abs == space.root() {
        return Err(ProjError::Refused("can't rename the workspace root".into()));
    }
    guard_shared(space, &abs, "rename")?;
    if is_recycled(space, &abs) {
        return Err(ProjError::Refused(format!(
            "'{}' is already in the recycle bin",
            to_rel(space, &abs)
        )));
    }
    if !is_project(&abs, probe) {
        return Err(not_a_project(rel));
    }
    let clean = clean_name(name);
    // The SUFFIX is the app: renaming a `.k7` into anything else would stop it being a project.
    if project_app(&clean) != project_app(&basename(&abs)) {
        return Err(ProjError::Refused(format!(
            "a project keeps the suffix that says which app opens it — '{clean}' should end in '{SUFFIXES}'"
        )));
    }
    let dest = space.safe_path(&to_rel(space, &join(&[&dirname(&abs), &clean])), probe)?;
    if dest == abs {
        return Ok(None);
    }
    if probe.exists(&dest) {
        return Err(ProjError::Exists(to_rel(space, &dest)));
    }
    let active = active_strict(space)?.filter(|a| is_within(&abs, a));
    Ok(Some(MovePlan {
        was_active: active.is_some(),
        active_to: active.map(|a| join(&[&dest, &relative(&abs, &a)])),
        answer: json!({ "path": to_rel(space, &dest) }).to_string(),
        abs,
        dest,
    }))
}

/// The body of a rename to the name it has: `{"path": <rel>}`.
pub fn kept_answer(space: &Space<'_>, rel: &str, probe: &dyn Probe) -> Result<String, ProjError> {
    let abs = own_spelling(space, &space.safe_path(rel, probe)?, probe)?;
    Ok(json!({ "path": to_rel(space, &abs) }).to_string())
}

/// `deleteProject`'s guards (2042, 2094); `dest` is the bin's first free name.
pub fn plan_recycle(
    space: &Space<'_>,
    rel: &str,
    probe: &dyn Probe,
) -> Result<MovePlan, ProjError> {
    let abs = own_spelling(space, &space.safe_path(rel, probe)?, probe)?;
    if abs == space.root() {
        return Err(ProjError::Refused("can't delete the workspace root".into()));
    }
    if is_recycled(space, &abs) {
        return Err(ProjError::Refused(format!(
            "already in the recycle bin: {rel}"
        )));
    }
    guard_shared(space, &abs, "delete")?;
    if !is_project(&abs, probe) {
        return Err(not_a_project(rel));
    }
    // A library keeps its own bin at its root, as 1b's delete does (W7).
    let base = library_of(space.home(), &abs).map_or_else(|| space.root().to_string(), |l| l.root);
    let bin = join(&[&base, RECYCLE_DIR]);
    let dest = recycle_target_in(&bin, &basename(&abs), &|p| probe.exists(p));
    let active = active_strict(space)?;
    Ok(MovePlan {
        was_active: active.as_deref() == Some(abs.as_str()),
        active_to: None,
        answer: json!({ "recycled": to_rel(space, &dest) }).to_string(),
        abs,
        dest,
    })
}

/// The rename's disk part: `abs` to `dest`, retried as 1b's `rename_with_retry`.
pub fn apply_rename(
    plan: &MovePlan,
    r: &dyn Renamer,
    tries: u32,
    delay: Duration,
) -> Result<(), ProjError> {
    Ok(rename_with_retry(r, &plan.abs, &plan.dest, tries, delay)?)
}

/// The recycle's disk part: the bin made, then `abs` to `dest`, retried.
pub fn apply_recycle(
    _space: &Space<'_>,
    plan: &MovePlan,
    r: &dyn Renamer,
    tries: u32,
    delay: Duration,
) -> Result<(), ProjError> {
    make_folder(&dirname(&plan.dest))?;
    apply_rename(plan, r, tries, delay)
}
