//! changes.rs — what differs between `HEAD` and the files: each path added, modified, removed or renamed,
//! with its size and whether it is binary. What a reader gets wrong: a rename is an exact match only (a file
//! moved and edited reads as a removal and an addition), and a symlink is never a change.

use std::path::Path;

use git2::{Delta, DiffFindOptions, DiffOptions};
use serde::Serialize;

use crate::lfs::Attributes;
use crate::lfs::pointer::Pointer;
use crate::lfs::store::StatCache;
use crate::{Repo, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Modified,
    Removed,
    Renamed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub path: String,
    pub change: Change,
    /// The old path of a rename.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub size: u64,
    pub binary: bool,
    /// The file goes through LFS: git holds a pointer, the store the bytes.
    pub lfs: bool,
    /// Over the size a git host takes: a commit leaves it out and `status` keeps naming it.
    #[serde(rename = "tooLarge")]
    pub too_large: bool,
}

/// A NUL in the first 8000 bytes, as git reads it.
pub(crate) fn looks_binary(head: &[u8]) -> bool {
    head.iter().take(8000).any(|b| *b == 0)
}

pub(crate) fn file_head(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut buf = vec![0u8; 8000];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .unwrap_or(0);
    buf.truncate(n);
    buf
}

impl Repo {
    pub(crate) fn head_tree(&self) -> Option<git2::Tree<'_>> {
        self.git.head().ok()?.peel_to_tree().ok()
    }

    /// Every changed path of the work tree against `HEAD`, sorted by path.
    pub fn status(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        let attrs = Attributes::read(&self.workdir);
        for c in self.deltas()? {
            let rel = c.new_path.clone();
            let abs = self.workdir.join(&rel);
            let lfs = attrs.tracks(&rel);
            let (size, head) = match c.change {
                Change::Removed => match c.old_blob_head {
                    Some((_, head)) if lfs && Pointer::parse(&head).is_some() => {
                        (Pointer::parse(&head).map_or(0, |p| p.size), vec![0])
                    }
                    Some((size, head)) => (size, head),
                    None => (0, Vec::new()),
                },
                _ => (
                    std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0),
                    file_head(&abs),
                ),
            };
            out.push(Entry {
                path: rel,
                change: c.change,
                from: c.old_path,
                size,
                binary: looks_binary(&head),
                lfs,
                too_large: c.change != Change::Removed && size > self.max_file,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// The deltas the status and the commit share, with the device's not-here files and symlinks left out.
    pub(crate) fn deltas(&self) -> Result<Vec<Raw>> {
        let tree = self.head_tree();
        let mut o = DiffOptions::new();
        o.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_typechange(true);
        let mut diff = self
            .git
            .diff_tree_to_workdir_with_index(tree.as_ref(), Some(&mut o))?;
        diff.find_similar(Some(
            DiffFindOptions::new()
                .renames(true)
                .for_untracked(true)
                .exact_match_only(true),
        ))?;
        let mut out = Vec::new();
        let attrs = Attributes::read(&self.workdir);
        let mut cache = StatCache::load(&self.gitdir);
        for d in diff.deltas() {
            let new = d.new_file();
            let old = d.old_file();
            let (Some(np), Some(op)) = (new.path(), old.path()) else {
                continue;
            };
            let (np, op) = (to_rel(np), to_rel(op));
            let change = match d.status() {
                Delta::Added | Delta::Untracked => Change::Added,
                Delta::Deleted => Change::Removed,
                Delta::Modified | Delta::Typechange => Change::Modified,
                Delta::Renamed => Change::Renamed,
                _ => continue,
            };
            let probe = if change == Change::Removed { &op } else { &np };
            if !self.present.present(probe) || (change != Change::Removed && self.skipped(probe)) {
                continue;
            }
            if change == Change::Modified
                && attrs.tracks(&np)
                && self.unchanged_large(&mut cache, &np, old.id())
            {
                continue;
            }
            let old_blob_head = (change == Change::Removed)
                .then(|| self.git.find_blob(old.id()).ok())
                .flatten()
                .map(|b| {
                    (
                        b.size() as u64,
                        b.content().iter().take(8000).copied().collect(),
                    )
                });
            let old_path = (change == Change::Renamed).then_some(op.clone());
            out.push(Raw {
                new_path: if change == Change::Removed { op } else { np },
                old_path,
                change,
                old_blob_head,
            });
        }
        cache.save();
        Ok(out)
    }

    /// A file git holds as a pointer is unchanged when its hash is the pointer's.
    fn unchanged_large(&self, cache: &mut StatCache, rel: &str, old: git2::Oid) -> bool {
        let Ok(blob) = self.git.find_blob(old) else {
            return false;
        };
        let Some(ptr) = Pointer::parse(blob.content()) else {
            return false;
        };
        cache
            .pointer(rel, &self.workdir.join(rel))
            .is_ok_and(|p| p.oid == ptr.oid)
    }

    /// A symlink, or a directory left by a nested repository: neither is a file k7 commits.
    fn skipped(&self, rel: &str) -> bool {
        match std::fs::symlink_metadata(self.workdir.join(rel)) {
            Ok(m) => !m.is_file(),
            Err(_) => true,
        }
    }
}

fn to_rel(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

pub(crate) struct Raw {
    pub new_path: String,
    pub old_path: Option<String>,
    pub change: Change,
    pub old_blob_head: Option<(u64, Vec<u8>)>,
}
