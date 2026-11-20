//! projects — a project opened, renamed and recycled natively: the marker's write, `plan_open`, `plan_rename`,
//! `plan_recycle` and the info the home page polls (docs/file-projects.md, 1g). Nothing here sets a Workspace's memory.
//! What a reader gets wrong: a plan reads the disk BEFORE the act, so the answer counts only if the act runs under the lock.

pub mod info;
pub mod life;
pub mod marker;
pub mod open;

use std::fmt;

use crate::entries::{DESCRIPTIONS, EntryError};
use crate::space::{Declined, PathError};

/// Why an open, a rename or a recycle is refused or fails, each as its route words it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjError {
    /// 400 with the TypeScript's sentence, whole.
    Refused(String),
    /// 400 `already exists: <rel>` (EEXIST in the TypeScript).
    Exists(String),
    /// An `fs` failure, worded through `entryFailure` by the route (2095).
    Io {
        code: &'static str,
        syscall: &'static str,
        path: String,
    },
    Declined(Declined),
}

impl ProjError {
    /// The errno code of an I/O failure, which a route maps to its status.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            ProjError::Io { code, .. } => Some(code),
            ProjError::Exists(_) => Some("EEXIST"),
            _ => None,
        }
    }
}

impl fmt::Display for ProjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjError::Refused(s) => f.write_str(s),
            ProjError::Exists(rel) => write!(f, "already exists: {rel}"),
            ProjError::Io {
                code,
                syscall,
                path,
            } => {
                let what = DESCRIPTIONS
                    .iter()
                    .find(|(c, _)| c == code)
                    .map_or("unknown error", |(_, d)| d);
                write!(f, "{code}: {what}, {syscall} '{path}'")
            }
            ProjError::Declined(d) => write!(f, "declined: {}", d.as_str()),
        }
    }
}

impl From<Declined> for ProjError {
    fn from(d: Declined) -> ProjError {
        ProjError::Declined(d)
    }
}

impl From<PathError> for ProjError {
    fn from(e: PathError) -> ProjError {
        match e {
            PathError::Declined(d) => ProjError::Declined(d),
            e => ProjError::Refused(e.to_string()),
        }
    }
}

impl From<EntryError> for ProjError {
    fn from(e: EntryError) -> ProjError {
        match e {
            EntryError::Io {
                code,
                syscall,
                path,
            } => ProjError::Io {
                code,
                syscall,
                path,
            },
            EntryError::Exists { rel } => ProjError::Exists(rel),
            EntryError::Declined(d) => ProjError::Declined(d),
            e => ProjError::Refused(e.to_string()),
        }
    }
}

/// `workspaceInfo()`: what a workspace holds and which project is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub root: String,
    pub root_is_project: bool,
    pub active: Option<String>,
    pub projects: Vec<String>,
    pub active_is_demo: bool,
}

fn quoted(out: &mut String, s: &str) {
    // A `&str` serializes without failing.
    out.push_str(&serde_json::to_string(s).unwrap_or_default());
}

/// `workspaceInfo`'s literal in its key order, then `tail` (spelled members without braces, `""` for none).
pub fn info_json(info: &Info, tail: &str) -> String {
    let mut out = String::from("{\"root\":");
    quoted(&mut out, &info.root);
    out.push_str(&format!(
        ",\"rootIsProject\":{},\"activeProject\":",
        info.root_is_project
    ));
    match &info.active {
        Some(a) => quoted(&mut out, a),
        None => out.push_str("null"),
    }
    out.push_str(",\"projects\":[");
    for (i, p) in info.projects.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quoted(&mut out, p);
    }
    out.push(']');
    if info.active_is_demo {
        out.push_str(",\"activeIsDemo\":true");
    }
    if !tail.is_empty() {
        out.push(',');
        out.push_str(tail);
    }
    out.push('}');
    out
}
