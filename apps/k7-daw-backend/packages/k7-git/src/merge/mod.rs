//! merge — a pull that diverged: a three-way merge computed in memory, kept in the git folder until every conflict
//! is chosen, then landed as one merge commit. What a reader gets wrong: nothing in the work tree moves before
//! `resolve`, so aborting deletes one file and leaves `HEAD` and every file as they were.

pub(crate) mod assemble;
mod build;
mod casefold;
mod pending;
pub(crate) mod plan;
mod resolve;
mod text;

use serde::{Deserialize, Serialize};

pub use pending::{Pending, Summary};

/// What the person picks for one conflict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Mine,
    Theirs,
    /// Both texts (a block of lines, mine then theirs) or both files (theirs written beside under a free name).
    Both,
    /// One side removed a file the other changed: keep it.
    Keep,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    K7x,
    Text,
    Binary,
    Presence,
}

/// One thing the merge could not settle by itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub id: String,
    pub path: String,
    pub kind: Kind,
    /// The key path of a `.k7x` change, dotted.
    #[serde(rename = "keyPath", skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    /// The first line of a text block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    pub base: Option<String>,
    pub mine: Option<String>,
    pub theirs: Option<String>,
    /// Whether `Both` is offered.
    pub both: bool,
    pub choice: Option<Choice>,
}

/// What `begin` found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Begin {
    /// Nothing to bring: the remote's commit is already in this branch.
    UpToDate,
    /// This branch is behind in a straight line: the files move, no commit is made.
    /// `head` is where the branch stood when it was planned (none: no commit yet); the move re-checks it.
    FastForward {
        theirs: String,
        head: Option<String>,
    },
    /// A merge is pending (written to the git folder), with whatever needs choosing.
    Merge(Summary),
}
