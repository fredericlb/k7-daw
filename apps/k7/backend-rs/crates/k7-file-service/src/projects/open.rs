//! open.rs — which project a bare address opens and the open judged: `defaultProject` and `setActiveProject`'s guards.
//! What a reader gets wrong: the guards' ORDER is the answer (a stale link reads "no project", not "not a project").

use super::ProjError;
use super::marker::read_raw;
use crate::entries::Probe;
use crate::entries::guards::is_recycled;
use crate::filter::project_app;
use crate::libraries::library_of;
use crate::paths::{basename, is_within, join, relative};
use crate::rel::to_rel;
use crate::space::{Declined, Space};
use crate::tree::Limits;
use crate::workspaces::names::{is_project_dir, project_dirs_under};

/// `PROJECT_SUFFIXES`' values in their order, as the sentences print them.
pub(super) const SUFFIXES: &str = ".k7";

/// `defaultProject`: `.` for a workspace that is a project; else the marker's project; else the first on disk (2030).
pub fn default_project(space: &Space<'_>, limits: &Limits) -> Result<Option<String>, Declined> {
    if is_project_dir(space.root()) {
        return Ok(Some(".".into()));
    }
    if let Some(marked) = read_raw(space.root())?
        && is_project_dir(&marked)
    {
        return Ok(Some(to_rel(space, &marked)));
    }
    let found = project_dirs_under(space.root(), limits, &mut 0)?;
    Ok(found.into_iter().next())
}

/// An open judged: the project's absolute path and its rel, spelled as the folder spells itself (2092).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenPlan {
    pub abs: String,
    pub rel: String,
}

/// `abs` with each segment below its root spelled as the disk spells it (2092); a segment the disk lacks stays as typed.
pub(super) fn own_spelling(
    space: &Space<'_>,
    abs: &str,
    probe: &dyn Probe,
) -> Result<String, Declined> {
    let base = library_of(space.home(), abs).map_or_else(|| space.root().to_string(), |l| l.root);
    if abs == base || !is_within(&base, abs) {
        return Ok(abs.to_string());
    }
    let (mut at, mut seen) = (base.clone(), 0);
    let below = relative(&base, abs);
    let mut segs = below.split('/');
    for seg in segs.by_ref() {
        match probe.spelled(&at, seg, &mut seen)? {
            Some(spelled) => at = join(&[&at, &spelled]),
            None => {
                at = join(&[&at, seg]);
                break;
            }
        }
    }
    for seg in segs {
        at = join(&[&at, seg]);
    }
    Ok(at)
}

/// The sentence for a folder that is no project.
pub(super) fn not_a_project(rel: &str) -> ProjError {
    ProjError::Refused(format!(
        "not a project (the folder must end in {SUFFIXES}): {rel}"
    ))
}

/// Whether `abs` is a project folder, as the probe's disk answers.
pub(super) fn is_project(abs: &str, probe: &dyn Probe) -> bool {
    project_app(&basename(abs)).is_some() && probe.is_dir(abs)
}

/// `setActiveProject`'s guards in the TypeScript's order (2031); nothing is written.
pub fn plan_open(space: &Space<'_>, rel: &str, probe: &dyn Probe) -> Result<OpenPlan, ProjError> {
    let abs = own_spelling(space, &space.safe_path(rel, probe)?, probe)?;
    if is_recycled(space, &abs) {
        return Err(ProjError::Refused(format!(
            "project is in the recycle bin: {rel}"
        )));
    }
    if !probe.exists(&abs) {
        return Err(ProjError::Refused(format!(
            "no project '{rel}' in this workspace — it may have been renamed, removed, or never have been on this machine; go back to the projects page and open it from there"
        )));
    }
    if !is_project(&abs, probe) {
        return Err(not_a_project(rel));
    }
    Ok(OpenPlan {
        rel: to_rel(space, &abs),
        abs,
    })
}
