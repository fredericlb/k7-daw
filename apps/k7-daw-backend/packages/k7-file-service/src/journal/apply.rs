//! apply.rs — one side of a step written over the project: every path confined before the first write, texts and blobs
//! landing whole, a missing side removing the file (1750 to 1755). What a reader gets wrong: the side is idempotent, so a
//! step that failed part-way is finished by the next undo, and the cursor moves only after the files did.

use std::fs::File;
use std::io;
use std::time::Duration;

use super::blobs::Side;
use super::format::Entry;
use super::store::Store;
use crate::atomic::{Fs, write_atomic_on};
use crate::filter::code_of;
use crate::paths::dirname;

/// Why a step was not applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyError {
    /// `history: unsafe path <rel>`: absolute, or climbing.
    Unsafe(String),
    /// The workspace's gates refused the path (1795): their sentence.
    Refused(String),
    Io {
        code: &'static str,
        path: String,
    },
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::Unsafe(rel) => write!(f, "history: unsafe path {rel}"),
            ApplyError::Refused(s) => f.write_str(s),
            ApplyError::Io { code, path } => {
                write!(f, "the file could not be changed ({code}): {path}")
            }
        }
    }
}

/// How a write lands: the disk, whether a rename is retried (Windows), and the wait.
pub struct Writer<'a> {
    pub fs: &'a dyn Fs,
    pub retry: bool,
    pub sleep: &'a (dyn Fn(Duration) + Sync),
}

fn unsafe_rel(rel: &str) -> bool {
    rel.starts_with('/') || rel.split(['\\', '/']).any(|s| s == "..")
}

fn io(e: &io::Error, path: &str) -> ApplyError {
    ApplyError::Io {
        code: code_of(e),
        path: path.to_string(),
    }
}

fn remove(abs: &str) -> Result<(), ApplyError> {
    match std::fs::remove_file(abs) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(io(&e, abs)),
        _ => Ok(()),
    }
}

/// `side` of `e` written; `confine` is the workspace's two gates (1795), a refusal being its sentence.
pub fn apply_side(
    store: &Store,
    e: &Entry,
    side: Side,
    confine: &dyn Fn(&str) -> Result<String, String>,
    w: &Writer<'_>,
) -> Result<(), ApplyError> {
    let paths: Vec<&str> = e
        .changes
        .iter()
        .chain(&e.blobs)
        .map(|c| c.path.as_str())
        .collect();
    let mut abs = Vec::with_capacity(paths.len());
    for rel in &paths {
        if unsafe_rel(rel) {
            return Err(ApplyError::Unsafe((*rel).into()));
        }
    }
    for rel in &paths {
        abs.push(confine(rel).map_err(ApplyError::Refused)?);
    }
    let mut at = abs.iter();
    for c in &e.changes {
        let target = at.next().map_or("", String::as_str);
        let content = if side == Side::Before {
            &c.before
        } else {
            &c.after
        };
        match content {
            None => remove(target)?,
            Some(text) => {
                std::fs::create_dir_all(dirname(target)).map_err(|x| io(&x, target))?;
                write_atomic_on(w.fs, target, &mut text.as_bytes(), w.retry, w.sleep)
                    .map_err(|x| io(&x, target))?;
            }
        }
    }
    for b in &e.blobs {
        let target = at.next().map_or("", String::as_str);
        let name = if side == Side::Before {
            &b.before
        } else {
            &b.after
        };
        match name {
            None => remove(target)?,
            Some(n) => {
                std::fs::create_dir_all(dirname(target)).map_err(|x| io(&x, target))?;
                // The copy lands through a temp and a rename, so a full disk leaves the song's file whole (1753).
                let mut src = File::open(store.blob_path(n)).map_err(|x| io(&x, target))?;
                write_atomic_on(w.fs, target, &mut src, w.retry, w.sleep)
                    .map_err(|x| io(&x, target))?;
            }
        }
    }
    Ok(())
}
