//! changes — one change set of the browser's verbs, judged and landed whole: `request` reads the body, `plan` judges every
//! path and write without touching the disk, `fresh` compares the set with the disk, `commit` lands it or leaves nothing.
//! What a reader gets wrong: the judging order IS the answer (shape, limits, gates, parse, then freshness), see docs/file-changes.md.

pub mod commit;
pub mod fresh;
pub mod jobs;
pub mod landing;
pub mod plan;
pub mod request;
pub mod target;

use std::fmt;

/// The most ops one set carries (3-B8).
pub const MAX_OPS: usize = 2_000;
/// The most text bytes one set carries in all (3-B8), as `PUT /file`'s body limit.
pub const MAX_TEXT: usize = 8 << 20;
/// The longest label, in characters.
pub const MAX_LABEL: usize = 4_000;

/// One op of the set, with the names as the request spelled them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Text {
        file: String,
        text: String,
        was: Option<String>,
    },
    Remove {
        file: String,
        was: Option<String>,
    },
    /// A check only: the file a verb read, never written; the set is stale when it moved.
    Read {
        file: String,
        was: Option<String>,
    },
    Copy {
        from: String,
        file: String,
        replace: bool,
    },
    Move {
        from: String,
        file: String,
        replace: bool,
    },
    Adopt {
        job: String,
        out: String,
        file: String,
        replace: bool,
    },
}

/// A request body, shaped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub at: String,
    pub base: String,
    pub label: String,
    /// How the journal takes the set: a step of its own, joined to the one on top, or left out (`graph pin`).
    pub journal: crate::journal::Mode,
    pub ops: Vec<Op>,
    /// A new project folder below the workspace, whose files the set writes (`landing`); only with an empty `at`.
    pub create: Option<String>,
    pub reload: bool,
    pub want_graph: bool,
}

/// Why a set is refused, each with its status and its sentence; nothing is written for any of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// 400: a malformed body or an op that cannot mean anything.
    Bad(String),
    /// 403: a path past the gates, a hidden name, a library this person may not write.
    Forbidden(String),
    /// 413: past a limit.
    TooMany(String),
    /// 422: a written `.k7x` that does not parse.
    Unparsable { file: String, error: String },
    /// 409: the files whose `was` moved, and whether the project's token did.
    Stale { files: Vec<String>, project: bool },
    /// 409: the folder a set creates holds a project already; nothing is removed for it.
    Taken(String),
    /// 409: the open project is another than the set's `at`; `open` is the one open now below the root, none when it is not.
    Elsewhere { open: Option<String> },
    /// 503: the native side does not answer this one.
    Declined(String),
}

impl Refusal {
    pub fn status(&self) -> u16 {
        match self {
            Refusal::Bad(_) => 400,
            Refusal::Forbidden(_) => 403,
            Refusal::TooMany(_) => 413,
            Refusal::Unparsable { .. } => 422,
            Refusal::Stale { .. } | Refusal::Elsewhere { .. } | Refusal::Taken(_) => 409,
            Refusal::Declined(_) => 503,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Bad(s)
            | Refusal::Forbidden(s)
            | Refusal::TooMany(s)
            | Refusal::Taken(s)
            | Refusal::Declined(s) => f.write_str(s),
            Refusal::Unparsable { error, .. } => f.write_str(error),
            Refusal::Elsewhere { .. } => f.write_str("another project is open"),
            Refusal::Stale { files, .. } if files.is_empty() => {
                f.write_str("the project moved since it was read")
            }
            Refusal::Stale { files, .. } => {
                write!(f, "changed since it was read: {}", files.join(", "))
            }
        }
    }
}
