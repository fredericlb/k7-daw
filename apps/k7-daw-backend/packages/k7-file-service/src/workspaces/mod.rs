//! workspaces — the account's workspaces as a whole, natively: their names, the home page's listings, the demos in
//! Examples, a project copied between two, a workspace made and removed (docs/file-workspaces.md).
//! What a reader gets wrong: a plan reads the disk BEFORE the write, so it is the answer only if nothing moved since.

pub mod copy;
pub mod demos;
pub mod life;
pub mod listing;
pub mod names;
pub mod refs;
pub mod surfaces;

use std::fmt;

use crate::entries::{DESCRIPTIONS, EntryError};
use crate::space::{Declined, PathError};

/// The workspace the demos are mirrored into; a test holds it equal to web-protocol's `EXAMPLES_WORKSPACE`.
pub const EXAMPLES: &str = "Examples";
/// The bench's workspace, opened by its address and never listed; a test holds it equal to `BENCH_WORKSPACE`.
pub const BENCH: &str = "Banc";

/// Why an act on a workspace is refused or fails, each as its route words it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WsError {
    /// 400 `invalid workspace name: <name>`.
    Invalid {
        name: String,
    },
    /// 404 `unknown workspace: <name>` (a copy's ends).
    Unknown {
        name: String,
    },
    /// 400 `no such workspace: <name>` (a removal).
    NoSuch {
        name: String,
    },
    /// 400, the TypeScript's sentence whole.
    Refused(String),
    /// An `fs` failure, worded through `entryFailure` by the route (1691).
    Io {
        code: &'static str,
        syscall: &'static str,
        path: String,
    },
    Declined(Declined),
}

impl WsError {
    /// The errno code of an I/O failure, which a route maps to its status.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            WsError::Io { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl fmt::Display for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WsError::Invalid { name } => write!(f, "invalid workspace name: {name}"),
            WsError::Unknown { name } => write!(f, "unknown workspace: {name}"),
            WsError::NoSuch { name } => write!(f, "no such workspace: {name}"),
            WsError::Refused(s) => f.write_str(s),
            WsError::Io {
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
            WsError::Declined(d) => write!(f, "declined: {}", d.as_str()),
        }
    }
}

impl From<Declined> for WsError {
    fn from(d: Declined) -> WsError {
        WsError::Declined(d)
    }
}

impl From<PathError> for WsError {
    fn from(e: PathError) -> WsError {
        match e {
            PathError::Declined(d) => WsError::Declined(d),
            e => WsError::Refused(e.to_string()),
        }
    }
}

impl From<EntryError> for WsError {
    fn from(e: EntryError) -> WsError {
        match e {
            EntryError::Io {
                code,
                syscall,
                path,
            } => WsError::Io {
                code,
                syscall,
                path,
            },
            EntryError::Declined(d) => WsError::Declined(d),
            e => WsError::Refused(e.to_string()),
        }
    }
}

/// One workspace of `GET /workspaces`: `{name, isSingleProject, projectCount}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub name: String,
    pub single: bool,
    pub count: usize,
}

/// One surface a project declares; `kind` is `type` on the wire, `playlist` what a copy carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surfaced {
    pub id: String,
    pub kind: String,
    pub playlist: Option<String>,
}

/// One project of a workspace, as the home page draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub path: String,
    pub app: &'static str,
    pub tags: Vec<String>,
    pub surfaces: Vec<Surfaced>,
    pub active: bool,
    pub edited_at: Option<f64>,
    pub demo: bool,
}

/// One workspace with its projects.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub single: bool,
    pub projects: Vec<Project>,
}

pub use crate::revision::ms;

fn quoted(out: &mut String, s: &str) {
    // A `&str` serializes without failing.
    out.push_str(&serde_json::to_string(s).unwrap_or_default());
}

/// `{"workspaces":[{name, isSingleProject, projectCount}]}`, the literal's key order.
pub fn summaries_json(rows: &[Summary]) -> String {
    let mut out = String::from("{\"workspaces\":[");
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        quoted(&mut out, &r.name);
        out.push_str(&format!(
            ",\"isSingleProject\":{},\"projectCount\":{}}}",
            r.single, r.count
        ));
    }
    out.push_str("]}");
    out
}

fn project_json(out: &mut String, p: &Project) {
    out.push_str("{\"path\":");
    quoted(out, &p.path);
    out.push_str(",\"app\":");
    quoted(out, p.app);
    if !p.tags.is_empty() {
        out.push_str(",\"tags\":[");
        for (i, t) in p.tags.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            quoted(out, t);
        }
        out.push(']');
    }
    out.push_str(",\"surfaces\":[");
    for (i, s) in p.surfaces.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quoted(out, &s.kind);
    }
    out.push_str(&format!("],\"active\":{}", p.active));
    if let Some(t) = p.edited_at {
        out.push_str(",\"editedAt\":");
        out.push_str(&k7_js::number::to_string(t));
    }
    if p.demo {
        out.push_str(",\"demo\":true");
    }
    out.push('}');
}

/// `{"workspaces":[{name, isSingleProject, projects:[{path, app, tags?, surfaces, active, editedAt?, demo?}]}]}`.
pub fn rows_json(rows: &[Row]) -> String {
    let mut out = String::from("{\"workspaces\":[");
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        quoted(&mut out, &r.name);
        out.push_str(&format!(",\"isSingleProject\":{},\"projects\":[", r.single));
        for (j, p) in r.projects.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            project_json(&mut out, p);
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// `{"projects":[{workspace, path, app, active, surfaces:[{id, type}]}]}`, the projects that declare a surface.
pub fn surfaced_json(rows: &[Row]) -> String {
    let mut out = String::from("{\"projects\":[");
    let mut first = true;
    for r in rows {
        for p in r.projects.iter().filter(|p| !p.surfaces.is_empty()) {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("{\"workspace\":");
            quoted(&mut out, &r.name);
            out.push_str(",\"path\":");
            quoted(&mut out, &p.path);
            out.push_str(",\"app\":");
            quoted(&mut out, p.app);
            out.push_str(&format!(",\"active\":{},\"surfaces\":[", p.active));
            for (i, s) in p.surfaces.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str("{\"id\":");
                quoted(&mut out, &s.id);
                out.push_str(",\"type\":");
                quoted(&mut out, &s.kind);
                out.push('}');
            }
            out.push_str("]}");
        }
    }
    out.push_str("]}");
    out
}
