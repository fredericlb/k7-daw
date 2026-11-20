//! commit.rs — the whole tree as one commit, authored by the person, with the trailer `K7-Device:`.
//! What a reader gets wrong: the index is built here from the diff against `HEAD`, never by `add_all`, and
//! blobs are written without libgit2's filters; a `Cleaner` (LFS) may turn a file into a pointer before it is stored.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{IndexEntry, IndexTime, Signature};

use crate::changes::Raw;
use crate::lfs::Attributes;
use crate::lfs::store::StatCache;
use crate::lfs::sync::LfsCleaner;
use crate::{Change, Error, Identity, Repo, Result};

/// The largest file a commit takes: what the common git hosts take through LFS.
pub const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;

static DEFAULT_MAX: AtomicU64 = AtomicU64::new(MAX_FILE);

/// Test seam: the limit a repository opened from now on starts with.
pub fn set_default_max_file(bytes: u64) {
    DEFAULT_MAX.store(bytes, Ordering::SeqCst);
}

pub(crate) fn default_max_file() -> u64 {
    DEFAULT_MAX.load(Ordering::SeqCst)
}

/// What a tracked file becomes in git.
pub enum Cleaned {
    /// The file's own bytes.
    Plain,
    /// These bytes in its place (an LFS pointer).
    Pointer(Vec<u8>),
}

/// The clean step of a file about to be committed (LFS plugs in here).
pub trait Cleaner {
    fn clean(&self, rel: &str, abs: &Path, size: u64) -> Result<Cleaned>;
}

/// `message` with its trailer, which a second device name never duplicates.
fn with_trailer(message: &str, device: &str) -> String {
    let device = device.replace(['\n', '\r'], " ");
    format!("{}\n\nK7-Device: {}\n", message.trim_end(), device.trim())
}

fn index_time(t: std::io::Result<std::time::SystemTime>) -> IndexTime {
    let d = t
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default();
    IndexTime::new(d.as_secs() as i32, d.subsec_nanos())
}

impl Repo {
    /// Commit every change, returning the new commit's id; a file over the limit refuses it.
    pub fn commit(&self, message: &str, who: &Identity, device: &str) -> Result<String> {
        self.commit_excluding(message, who, device, &[])
    }

    /// The paths a commit leaves out when asked to: every file over the limit.
    pub fn too_large(&self) -> Result<Vec<String>> {
        Ok(self
            .status()?
            .into_iter()
            .filter(|e| e.too_large)
            .map(|e| e.path)
            .collect())
    }

    /// Commit every change but `exclude`, which may name files over the limit only.
    pub fn commit_excluding(
        &self,
        message: &str,
        who: &Identity,
        device: &str,
        exclude: &[String],
    ) -> Result<String> {
        self.prepare_attributes()?;
        let cleaner = LfsCleaner {
            attrs: Attributes::read(&self.workdir),
            store: self.lfs_store(),
            cache: std::cell::RefCell::new(StatCache::load(&self.gitdir)),
        };
        let id = self.commit_with(&cleaner, message, who, device, exclude);
        cleaner.cache.borrow_mut().save();
        id
    }

    pub fn commit_with(
        &self,
        cleaner: &dyn Cleaner,
        message: &str,
        who: &Identity,
        device: &str,
        exclude: &[String],
    ) -> Result<String> {
        if !who.is_complete() {
            return Err(Error::NoIdentity);
        }
        // A commit moves HEAD, which a waiting merge could then never resolve against.
        if self.pending_file().exists() {
            return Err(Error::MergeOpen);
        }
        let mut changes = self.deltas()?;
        for path in exclude {
            let large = changes
                .iter()
                .any(|c| c.new_path == *path && self.over_limit(c));
            if !large {
                return Err(Error::BadPath(format!("{path} is not a file to leave out")));
            }
        }
        changes.retain(|c| !exclude.contains(&c.new_path));
        if changes.is_empty() {
            return Err(Error::NothingToCommit);
        }
        for c in &changes {
            if self.over_limit(c) {
                let size = std::fs::metadata(self.workdir.join(&c.new_path))?.len();
                return Err(Error::TooBig {
                    path: c.new_path.clone(),
                    size,
                });
            }
        }
        let parent = self.git.head().ok().and_then(|h| h.peel_to_commit().ok());
        let mut index = self.git.index()?;
        let in_step = match &parent {
            Some(p) => index.write_tree().is_ok_and(|id| id == p.tree_id()),
            None => index.is_empty(),
        };
        if !in_step {
            match &parent {
                Some(p) => index.read_tree(&p.tree()?)?,
                None => index.clear()?,
            }
        }
        for c in &changes {
            self.apply(&mut index, cleaner, c)?;
        }
        index.write()?;
        let tree = self.git.find_tree(index.write_tree()?)?;
        let sig = Signature::now(who.name.trim(), who.email.trim())?;
        let parents: Vec<_> = parent.iter().collect();
        let id = self.git.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &with_trailer(message, device),
            &tree,
            &parents,
        )?;
        Ok(id.to_string())
    }

    fn over_limit(&self, c: &Raw) -> bool {
        c.change != Change::Removed
            && std::fs::metadata(self.workdir.join(&c.new_path))
                .is_ok_and(|m| m.len() > self.max_file)
    }

    fn apply(&self, index: &mut git2::Index, cleaner: &dyn Cleaner, c: &Raw) -> Result<()> {
        if let Some(old) = &c.old_path {
            index.remove_path(Path::new(old))?;
        }
        if c.change == Change::Removed {
            index.remove_path(Path::new(&c.new_path))?;
            return Ok(());
        }
        let abs = self.workdir.join(&c.new_path);
        let meta = std::fs::metadata(&abs)?;
        let id = match cleaner.clean(&c.new_path, &abs, meta.len())? {
            Cleaned::Plain => {
                let mut w = self.git.blob_writer(None)?;
                std::io::copy(&mut std::fs::File::open(&abs)?, &mut w)?;
                w.flush()?;
                w.commit()?
            }
            Cleaned::Pointer(bytes) => self.git.blob(&bytes)?,
        };
        let path = c.new_path.as_bytes().to_vec();
        index.add(&entry(&meta, id, path))?;
        Ok(())
    }
}

/// The stat data libgit2 compares a file with, so an unchanged file is not hashed again.
#[cfg(unix)]
fn stat(meta: &std::fs::Metadata) -> (u32, u32, u32, u32, u32, IndexTime) {
    use std::os::unix::fs::MetadataExt;
    let mode = if meta.mode() & 0o100 != 0 {
        0o100755
    } else {
        0o100644
    };
    let ctime = IndexTime::new(meta.ctime() as i32, meta.ctime_nsec() as u32);
    (
        mode,
        meta.dev() as u32,
        meta.ino() as u32,
        meta.uid(),
        meta.gid(),
        ctime,
    )
}

#[cfg(not(unix))]
fn stat(meta: &std::fs::Metadata) -> (u32, u32, u32, u32, u32, IndexTime) {
    (0o100644, 0, 0, 0, 0, index_time(meta.modified()))
}

fn entry(meta: &std::fs::Metadata, id: git2::Oid, path: Vec<u8>) -> IndexEntry {
    let (mode, dev, ino, uid, gid, ctime) = stat(meta);
    IndexEntry {
        ctime,
        mtime: index_time(meta.modified()),
        dev,
        ino,
        mode,
        uid,
        gid,
        file_size: meta.len() as u32,
        id,
        flags: path.len().min(0xFFF) as u16,
        flags_extended: 0,
        path,
    }
}
