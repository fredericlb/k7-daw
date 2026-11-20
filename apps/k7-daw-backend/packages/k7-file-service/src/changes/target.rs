//! target.rs — the folder a change set or a bundle is FOR: the open project, a library chain the person writes, or the workspace alone.
//! What a reader gets wrong: a chain is named by its `_libraries/…` path and gated like any write there; an empty `at` is no project at all.

use super::Refusal;
use super::plan::{Act, Planned};
use crate::confine::Disk;
use crate::entries::guards::assert_library_writable;
use crate::paths::{is_within, join, resolve, resolve_all};
use crate::space::Space;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The open project, absolute.
    Project(String),
    /// A library chain folder the person may write, absolute.
    Chain(String),
    /// No project: the shared texts only.
    Workspace,
}

impl Target {
    /// The folder whose token a base is checked against; `None` for the workspace alone.
    pub fn project_dir(&self) -> Option<&str> {
        match self {
            Target::Project(d) | Target::Chain(d) => Some(d),
            Target::Workspace => None,
        }
    }

    /// The folder the journal images: the workspace alone images nothing (a folder that is never there).
    pub fn journal_dir(&self, root: &str) -> String {
        match self {
            Target::Project(d) | Target::Chain(d) => d.clone(),
            Target::Workspace => crate::paths::join(&[root, ".no-project"]),
        }
    }

    /// The folder a compile and a walk treat as the project; the workspace alone has the workspace's own root.
    pub fn dir(&self, root: &str) -> String {
        match self {
            Target::Project(d) | Target::Chain(d) => d.clone(),
            Target::Workspace => resolve_all(&[root]),
        }
    }
}

/// Where `at` points. A project must be the open one (`Elsewhere` names the open one); a chain must be writable by this person.
pub fn target(space: &Space<'_>, at: &str) -> Result<Target, Refusal> {
    if at.is_empty() {
        return Ok(Target::Workspace);
    }
    let rel = at.replace('\\', "/");
    if rel.starts_with("_libraries/") {
        let abs = space
            .safe_path(&rel, &Disk)
            .map_err(|e| Refusal::Forbidden(e.to_string()))?;
        assert_library_writable(space, &abs, "run a command on")
            .map_err(|e| Refusal::Forbidden(e.to_string()))?;
        if !std::fs::metadata(&abs).is_ok_and(|m| m.is_dir()) {
            return Err(Refusal::Bad(format!("'{at}' is not a folder")));
        }
        return Ok(Target::Chain(abs));
    }
    let active = match space.active() {
        Ok(a) => a,
        Err(d) => return Err(Refusal::Declined(format!("declined: {}", d.as_str()))),
    };
    let Some(open) = active else {
        return Err(Refusal::Bad("no project is open".into()));
    };
    if resolve(space.root(), at) == open {
        return Ok(Target::Project(open));
    }
    // A project outside the workspace is never shown by its place.
    let shown = open
        .strip_prefix(&format!(
            "{}/",
            space.root().replace('\\', "/").trim_end_matches('/')
        ))
        .map(str::to_string);
    Err(Refusal::Elsewhere { open: shown })
}

/// Every folder or file a set that is not for the open project may write: judged on the RESOLVED paths, `..` already folded by the gates.
pub fn confined(target: &Target, root: &str, planned: &Planned) -> Result<(), Refusal> {
    confined_in(target, root, planned, None)
}

/// …and, for a set that creates a project folder, that folder too (`landing`).
pub fn confined_in(
    target: &Target,
    root: &str,
    planned: &Planned,
    create: Option<&str>,
) -> Result<(), Refusal> {
    if matches!(target, Target::Project(_)) {
        return Ok(());
    }
    let root = resolve_all(&[root]);
    let shared = |abs: &str| {
        is_within(&join(&[&root, "_modules"]), abs)
            || is_within(&join(&[&root, "_playlists"]), abs)
            || resolve_all(&[abs]) == join(&[&root, "_library-pins.k7x"])
    };
    if let (Target::Workspace, Some(name)) = (target, create) {
        return super::landing::inside(&root, name, planned, &shared);
    }
    for a in &planned.acts {
        if matches!(a, Act::Read { .. }) {
            continue;
        }
        let mut written = vec![a.target().1.to_string()];
        if let Act::Move { from, .. } = a {
            written.push(from.clone());
        }
        for abs in written {
            let ok = match target {
                Target::Chain(dir) => is_within(dir, &abs) || shared(&abs),
                _ => shared(&abs),
            };
            if !ok {
                let what = if matches!(target, Target::Chain(_)) {
                    "a set for a library chain writes the chain and the workspace's shared texts only"
                } else {
                    "a set with no project writes the workspace's shared texts only"
                };
                return Err(Refusal::Forbidden(format!("'{}': {what}", a.target().0)));
            }
        }
    }
    Ok(())
}
