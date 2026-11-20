//! blobs.rs — a step's audio copies: where they are, which an undo lacks, and the three sentences it is stopped with
//! (1740 to 1744). What a reader gets wrong: fetching is the caller's, asynchronous and before the gate (1743); this
//! module only says what is missing and what the failure reads, as a 409 (1790).

use std::fmt;

use super::format::{EngineRef, Entry};
use super::store::Store;

/// Which image of a step an undo (`Before`) or a redo (`After`) writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

/// Why a step's audio cannot be had.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlobError {
    NoLongerKept { path: String },
    KeptElsewhere { path: String, kept: String },
    Gone { path: String, kept: String },
}

impl fmt::Display for BlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobError::NoLongerKept { path } => {
                write!(f, "The audio of {path} for this step is no longer kept.")
            }
            BlobError::KeptElsewhere { path, kept } => write!(
                f,
                "The audio of {path} for this step is kept by the engine \"{kept}\", not by the one in use. Switch back to \"{kept}\" to go past this step."
            ),
            BlobError::Gone { path, kept } => {
                write!(
                    f,
                    "The engine \"{kept}\" no longer has the audio of {path} for this step: it keeps it for a month."
                )
            }
        }
    }
}

/// A blob an undo needs that is not on disk here, and the engine that should hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    /// The project file it restores.
    pub path: String,
    pub name: String,
    /// Where the fetched copy goes.
    pub file: String,
    pub kept_on: EngineRef,
}

impl Missing {
    /// The failure when the engine did not bring it.
    pub fn gone(&self) -> BlobError {
        BlobError::Gone {
            path: self.path.clone(),
            kept: self.kept_on.name.clone(),
        }
    }
}

/// Each blob `side` of `e` needs, in order, until the first that cannot be had: `Ok` is one to fetch, `Err` the sentence.
pub fn steps(
    store: &Store,
    e: &Entry,
    side: Side,
    bound: Option<&EngineRef>,
) -> Vec<Result<Missing, BlobError>> {
    let mut out = Vec::new();
    for b in &e.blobs {
        let name = match side {
            Side::Before => &b.before,
            Side::After => &b.after,
        };
        let Some(name) = name else { continue };
        let file = store.blob_path(name);
        if std::path::Path::new(&file).exists() {
            continue;
        }
        let Some(Some(kept)) = &e.kept_on else {
            out.push(Err(BlobError::NoLongerKept {
                path: b.path.clone(),
            }));
            break;
        };
        if bound.map(|n| n.id.as_str()) != Some(kept.id.as_str()) {
            out.push(Err(BlobError::KeptElsewhere {
                path: b.path.clone(),
                kept: kept.name.clone(),
            }));
            break;
        }
        out.push(Ok(Missing {
            path: b.path.clone(),
            name: name.clone(),
            file,
            kept_on: kept.clone(),
        }));
    }
    out
}

/// Remove the copies here: an evicted step's, or a fetched copy once written over the project.
pub fn delete(store: &Store, names: &[String]) {
    for n in names {
        let _ = std::fs::remove_file(store.blob_path(n));
    }
}

/// Whether every blob `side` of `e` needs is on disk here.
pub fn all_here(store: &Store, e: &Entry, side: Side) -> bool {
    e.blobs.iter().all(|b| {
        let name = if side == Side::Before {
            &b.before
        } else {
            &b.after
        };
        name.as_ref()
            .is_none_or(|n| std::path::Path::new(&store.blob_path(n)).exists())
    })
}

/// A step's blobs stored: each staged file renamed to its blob name, the folder made only for a blob (1751).
pub fn store_staged(store: &Store, from_to: &[(String, String)]) -> std::io::Result<()> {
    if !from_to.is_empty() {
        std::fs::create_dir_all(store.blobs_dir())?;
    }
    for (staged, name) in from_to {
        std::fs::rename(staged, store.blob_path(name))?;
    }
    Ok(())
}
