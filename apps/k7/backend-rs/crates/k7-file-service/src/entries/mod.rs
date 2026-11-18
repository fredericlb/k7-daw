//! entries — the workspace's entries written natively: a folder made, an entry renamed, moved or recycled, an
//! upload's two halves. `plan` judges a write with every guard and writes nothing; `apply` does the disk's part.
//! What a reader gets wrong: a plan reads the disk BEFORE the write, so it is the answer only if nothing moved since.

pub mod apply;
pub mod guards;
pub mod plan;

use std::fmt;

use crate::confine::{self, Disk};
use crate::space::{Declined, PathError};
use crate::tree::Limits;
use crate::workspaces::names::spelled_in;

/// What the plan asks of the disk beyond the gate's: whether a path exists (as `existsSync`, links followed) and is a folder.
pub trait Probe: confine::Probe {
    fn exists(&self, p: &str) -> bool;
    fn is_dir(&self, p: &str) -> bool;
    /// The folder's own spelling of `name` in `dir`, `None` when absent (1632); the disk's, bounded, its dirents added to `seen` (F5).
    fn spelled(&self, dir: &str, name: &str, seen: &mut u64) -> Result<Option<String>, Declined> {
        spelled_in(dir, name, &Limits::default(), seen)
    }
}

impl Probe for Disk {
    fn exists(&self, p: &str) -> bool {
        std::fs::metadata(p).is_ok()
    }

    fn is_dir(&self, p: &str) -> bool {
        std::fs::metadata(p).is_ok_and(|m| m.is_dir())
    }
}

/// One of the five writes, with the names as the request carried them.
#[derive(Clone, Copy, Debug)]
pub enum Op<'a> {
    Folder { dir: &'a str, name: &'a str },
    Rename { path: &'a str, name: &'a str },
    Move { path: &'a str, to_dir: &'a str },
    Delete { path: &'a str },
    Upload { dir: &'a str, name: &'a str },
}

/// What a write came to, in absolute paths; `answer_of` spells it for the route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Folder {
        abs: String,
    },
    /// `refs` is the library ref before and after, and whether refs below it follow (`under`), for a library module.
    Rename {
        from: String,
        to: String,
        refs: Option<(String, String, bool)>,
    },
    Move {
        from: String,
        to: String,
    },
    Recycle {
        from: String,
        to: String,
    },
    /// A rename to its own name, a move to its own folder.
    Unchanged {
        abs: String,
    },
    Upload {
        abs: String,
    },
}

/// Why a write is refused or fails, as each route words it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryError {
    NotFound {
        rel: String,
    },
    Exists {
        rel: String,
    },
    Refused(String),
    Path(PathError),
    Io {
        code: &'static str,
        syscall: &'static str,
        path: String,
    },
    Declined(Declined),
}

impl From<PathError> for EntryError {
    fn from(e: PathError) -> EntryError {
        match e {
            PathError::Declined(d) => EntryError::Declined(d),
            e => EntryError::Path(e),
        }
    }
}

impl From<Declined> for EntryError {
    fn from(d: Declined) -> EntryError {
        EntryError::Declined(d)
    }
}

/// The shim's description of an errno, as `js/node/fs.ts`' `DESCRIPTIONS` holds it; a test holds the two equal (1233).
pub const DESCRIPTIONS: [(&str, &str); 10] = [
    ("ENOENT", "no such file or directory"),
    ("EEXIST", "file already exists"),
    ("ENOTDIR", "not a directory"),
    ("EISDIR", "illegal operation on a directory"),
    ("ENOTEMPTY", "directory not empty"),
    ("EACCES", "permission denied"),
    ("EPERM", "operation not permitted"),
    ("EBUSY", "resource busy or locked"),
    ("ENOSYS", "function not implemented"),
    ("EIO", "i/o error"),
];

impl EntryError {
    /// The errno code a route maps to its status: `ENOENT` is a 404, `EEXIST` an upload's 409.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            EntryError::NotFound { .. } => Some("ENOENT"),
            EntryError::Exists { .. } => Some("EEXIST"),
            EntryError::Io { code, .. } => Some(code),
            _ => None,
        }
    }

    /// An I/O failure that carries a `syscall`, which the routes word apart from every other message (1233).
    pub fn is_io(&self) -> bool {
        matches!(self, EntryError::Io { .. })
    }
}

impl fmt::Display for EntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryError::NotFound { rel } => write!(f, "no such file or folder: {rel}"),
            EntryError::Exists { rel } => write!(f, "already exists: {rel}"),
            EntryError::Refused(s) => f.write_str(s),
            EntryError::Path(p) => write!(f, "{p}"),
            EntryError::Io {
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
            EntryError::Declined(d) => write!(f, "declined: {}", d.as_str()),
        }
    }
}
