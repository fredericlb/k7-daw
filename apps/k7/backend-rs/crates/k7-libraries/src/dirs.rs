//! dirs.rs — what the hub reads off a library's folder: its version numbers, each version's manifest, whether a seed
//! is already one of them, and the husks (version folders holding no file but dot-files). `libraryDirs.ts`.
//! What a reader gets wrong: a husk is no version, and nothing is lost removing it (docs/libraries.md).

use std::fs;
use std::path::Path;


use crate::manifest::{Manifest, parse_manifest, stringify_manifest};

/// A manifest in its k7x spelling.
pub const MANIFEST: &str = "library.k7x";
/// A manifest before it was k7x.
/// The folder an admin edits a library in.
pub const DRAFT: &str = "draft";
/// A workspace's recycle bin, never published.
const RECYCLE_DIR: &str = "_recycle_bin";
/// The version a library written before versions is.
pub const UNPINNED_VERSION: u64 = 1;

/// A folder name as a published version: a positive integer without leading zeros, six digits at most.
pub fn version_of_dir(name: &str) -> Option<u64> {
    let b = name.as_bytes();
    let ok = (1..=6).contains(&b.len())
        && (b'1'..=b'9').contains(&b[0])
        && b.iter().all(u8::is_ascii_digit);
    ok.then(|| name.parse().unwrap_or(0))
}

/// What a published version carries: never a `.git`, never a recycle bin, anywhere in the source's path.
pub fn publishable(src: &Path) -> bool {
    !src.to_string_lossy()
        .split(['/', '\\'])
        .any(|s| s == ".git" || s == RECYCLE_DIR)
}

/// The published version numbers under a library's folder, ascending.
pub fn version_numbers(at: &Path) -> Vec<u64> {
    let Ok(rd) = fs::read_dir(at) else {
        return Vec::new();
    };
    let mut out: Vec<u64> = rd
        .flatten()
        .filter(|d| d.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|d| version_of_dir(&d.file_name().to_string_lossy()))
        .collect();
    out.sort_unstable();
    out
}

/// Does `dir` hold no file but dot-named ones, at any depth?
pub fn is_husk(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    for e in rd.flatten() {
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if !e.file_type().is_ok_and(|t| t.is_dir()) || !is_husk(&e.path()) {
            return false;
        }
    }
    true
}

fn read_text(p: &Path) -> Option<String> {
    fs::read(p)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// A version's manifest, `library.k7x`, read where it is.
pub fn read_manifest(dir: &Path) -> Option<Manifest> {
    parse_manifest(&read_text(&dir.join(MANIFEST))?)
}

/// Is the version whose manifest is `held` this seed, installed under its own number or, renumbered, under another?
pub fn is_copy_of_seed(held: Option<&Manifest>, seed: &Manifest) -> bool {
    let Some(h) = held else { return false };
    match h.seed {
        Some(s) => s == seed.version,
        None => h.version == seed.version && h.requires == seed.requires && h.notes == seed.notes,
    }
}

/// Version 1 of a library written before versions: it asks for nothing past the first level.
pub fn write_first_manifest(name: &str, dir: &Path) -> std::io::Result<()> {
    let file = dir.join(MANIFEST);
    if file.exists() {
        return Ok(());
    }
    fs::write(
        file,
        stringify_manifest(&Manifest::plain(name, UNPINNED_VERSION, 1)),
    )
}
