//! listings — what a workspace and its libraries list without compiling: playlists, reference tracks, the libraries'
//! chains and assemblies, and the module levels (1e). Each is the route's body written once as JSON text.
//! What a reader gets wrong: the orders are JavaScript's (`.sort()` is UTF-16, `byCodePoint` is `str`'s), and a
//! failed listing is empty, never an error, except where the TypeScript threw, which is declined (1418).

pub mod below;
pub mod engine;
pub mod families;
pub mod levels;
pub mod versions;
pub mod workspace;

use std::cmp::Ordering;

pub use versions::{LIBRARY_LEVEL, Remote};

use crate::paths::relative;
use crate::rel::to_rel;
use crate::space::{Declined, Space};
use crate::tree::Limits;

/// The five listings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listing {
    Playlists,
    References,
    LibraryChains,
    LibraryAssemblies,
    ModuleLibraries,
}

/// What the excuse of 1405 reads about one row: the file it came from, if any, and the folder holding it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub file: Option<String>,
    pub folder: String,
}

/// A listing: the body, per row its stamp, and what the walks met.
#[derive(Debug)]
pub struct Listed {
    /// The route's body, `{"<key>": [...]}`, in the literal's key order.
    pub json: String,
    pub rows: Vec<Stamp>,
    /// The newest mtime of a folder listed.
    pub newest_folder_ns: u64,
    /// For the parse bound (1055).
    pub depth: u32,
    /// The first folders a level could not list, as the walk spelt them (1423).
    pub unlisted: Vec<String>,
}

/// The listing `which`; a `Declined` where a limit is passed or the route is declined.
pub fn list(
    space: &Space<'_>,
    which: Listing,
    remote: &Remote,
    limits: &Limits,
) -> Result<Listed, Declined> {
    list_gated(space, which, remote, limits, &|_| true)
}

/// `list`, asking `admit` the size of each playlist before it is read; no is `Busy` (1442).
pub fn list_gated(
    space: &Space<'_>,
    which: Listing,
    remote: &Remote,
    limits: &Limits,
    admit: &dyn Fn(u64) -> bool,
) -> Result<Listed, Declined> {
    match which {
        Listing::Playlists => workspace::playlists(space, limits, admit),
        Listing::References => workspace::references(space, limits),
        Listing::LibraryChains => families::chains(space, limits),
        Listing::LibraryAssemblies => families::assemblies(space, limits),
        Listing::ModuleLibraries => levels::module_libraries(space, remote, limits),
    }
}

/// `.sort()`'s order: UTF-16 code units, which differs from `str`'s above U+FFFF.
pub(crate) fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `isDocName`: the name ends in `.k7x`, in any case.
pub(crate) fn is_doc_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 4 && b[b.len() - 4..].eq_ignore_ascii_case(b".k7x")
}

/// `stripDocExt` of a name that `is_doc_name`; any other name unchanged.
pub(crate) fn strip_doc_ext(name: &str) -> &str {
    if is_doc_name(name) {
        &name[..name.len() - 4]
    } else {
        name
    }
}

/// A string as a JSON literal, appended.
pub(crate) fn quoted(out: &mut Vec<u8>, s: &str) {
    // A `&str` serializes to a `Vec` without failing.
    let _ = serde_json::to_writer(&mut *out, s);
}

/// The entries of a folder, each a name and whether it is a folder or a file by its own type (a link is neither),
/// in byte order as node's `readdir` listed them; `None` when the folder or one entry cannot be read.
pub(crate) fn entries_of(abs: &str) -> Option<Vec<(String, Kind)>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(abs).ok()? {
        let e = e.ok()?;
        let kind = e.file_type().map_or(Kind::Other, |t| {
            if t.is_symlink() {
                Kind::Other
            } else if t.is_dir() {
                Kind::Dir
            } else if t.is_file() {
                Kind::File
            } else {
                Kind::Other
            }
        });
        out.push((e.file_name().to_string_lossy().into_owned(), kind));
    }
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Dir,
    File,
    Other,
}

/// `toRel` for the paths below one folder: the folder is spelt once, a path under it is that prefix and the rest (1422).
pub(crate) struct Spell<'a> {
    space: &'a Space<'a>,
    base: String,
    prefix: String,
}

impl<'a> Spell<'a> {
    pub fn new(space: &'a Space<'a>, base: &str) -> Spell<'a> {
        Spell {
            space,
            base: base.to_string(),
            prefix: to_rel(space, base),
        }
    }

    pub fn of(&self, abs: &str) -> String {
        if abs == self.base {
            return self.prefix.clone();
        }
        // A path the folder holds shares its prefix; one that leaves it is spelt by what it lands in.
        match abs
            .strip_prefix(self.base.as_str())
            .and_then(|t| t.strip_prefix('/'))
        {
            Some(t) if !t.is_empty() && !t.split('/').any(|p| matches!(p, "" | "." | "..")) => {
                if self.prefix == "." {
                    t.to_string()
                } else {
                    format!("{}/{t}", self.prefix)
                }
            }
            _ => {
                let rel = relative(&self.base, abs);
                if rel.is_empty() || crate::paths::climbs_out(&rel) {
                    to_rel(self.space, abs)
                } else if self.prefix == "." {
                    rel
                } else {
                    format!("{}/{rel}", self.prefix)
                }
            }
        }
    }
}
