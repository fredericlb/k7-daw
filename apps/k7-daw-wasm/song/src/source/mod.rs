// source/mod.rs — the one door the compile reads through: three calls over absolute paths, bytes out.
// A reader would get wrong what Missing is: a memory source that does not hold a path, which is not a path that is not there.
// Confinement belongs to every implementation, so no caller can forget it (docs/rustCompiler.md, "The Source").

mod bundle;
#[cfg(feature = "disk")]
mod disk;
mod mem;
pub mod path;

pub use bundle::{
    BundleFile, BundleRequest, MAX_BUNDLE, MAX_ENTRIES, MAX_MOUNTS, MAX_PATH_DEPTH, MAX_PATH_LEN, SourceBundle,
};
#[cfg(feature = "disk")]
pub use disk::DiskSource;
pub use mem::MemSource;

use serde::{Deserialize, Serialize};
use std::fmt;

/// What a path is; a link is already followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Dir,
}

/// What a file looked like when it was stat'ed; only a settled stamp may key a cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stamp {
    /// The disk's mtime in nanoseconds, or the memory source's FNV-1a of the text mixed with the path's generation.
    pub mtime_ns: u64,
    pub len: u64,
    /// Whether no later write could reproduce this stamp (mtime older than two seconds); always true in memory.
    pub settled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stat {
    pub kind: Kind,
    pub len: u64,
    pub stamp: Stamp,
}

/// One name in a listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
}

/// Which bytes of a file to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    All,
    /// `offset`, then at most `len` bytes.
    Range {
        offset: u64,
        len: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    NotFound,
    /// The path lies outside every mount, lexically or through a link.
    Outside,
    /// The memory source does not hold this path or listing: fetch a larger bundle.
    Missing,
    Io(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::NotFound => f.write_str("not found"),
            SourceError::Outside => f.write_str("outside the project's folders"),
            SourceError::Missing => f.write_str("not in the bundle"),
            SourceError::Io(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for SourceError {}

pub trait Source {
    /// Follows links.
    fn stat(&self, path: &str) -> Result<Stat, SourceError>;
    /// Sorted by name, bytewise.
    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError>;
    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError>;

    /// A text the caller already holds for `path`, which a recording Source keeps as if it had been read.
    fn note_text(&self, _path: &str, _text: &std::sync::Arc<str>) {}

    /// Whether a stamp's `mtime_ns` is a time two stamps can be ordered by; a memory stamp is a hash (decision 352).
    fn stamps_are_times(&self) -> bool {
        false
    }

    /// What names a folder whatever path leads to it (its real path on a disk), or None when this source cannot say.
    fn folder_id(&self, _path: &str) -> Option<String> {
        None
    }

    /// A digest the caller computed for `path`, which a recording Source keeps for a file it holds no text of.
    fn note_digest(&self, _path: &str, _hex: &str) {}

    /// The digest a bundle recorded for `path`, so a replay need not read bytes it does not hold.
    fn known_digest(&self, _path: &str) -> Option<String> {
        None
    }

    /// Whether the jsdsp source's graph was stale when a bundle was made, or None when this source cannot say.
    fn known_stale(&self, _source: &str) -> Option<bool> {
        None
    }

    /// The seconds a bundle measured for the audio at `path`, so a source holding no bytes of it still answers a length.
    fn known_seconds(&self, _path: &str) -> Option<f64> {
        None
    }

    /// Whether a path with no seconds in `known_seconds` is one the bundle could not measure, not one it forgot: a replay holds a table.
    fn measured_by_table(&self) -> bool {
        false
    }

    /// The whole file as text; bytes that are not UTF-8 are refused.
    fn read_text(&self, path: &str) -> Result<String, SourceError> {
        let bytes = self.read(path, Span::All)?;
        String::from_utf8(bytes).map_err(|_| SourceError::Io(format!("{path}: not valid UTF-8")))
    }
}

/// A listing's order, which is bytes and not the file system's.
pub(crate) fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
}
