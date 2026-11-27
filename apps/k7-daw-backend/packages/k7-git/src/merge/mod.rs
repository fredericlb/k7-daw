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

pub use pending::{Pending, Summary};

pub use git_rules::{Choice, Conflict, Kind};

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
