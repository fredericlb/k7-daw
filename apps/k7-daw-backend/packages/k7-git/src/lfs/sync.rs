//! sync.rs — what the repository does with large files: the clean step of a commit, the pointers a tree holds, the
//! bytes a push owes the host and a pull brings in, the smudge into the work tree, and the prune.
//! What a reader gets wrong: a store object is removed only when the remote's tip holds it AND the work tree holds a file
//! with the same hash; the files land through a `Landing`, the seam the capture's own landing plugs into.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use git2::{Delta, ObjectType, Oid, TreeWalkMode, TreeWalkResult};

use super::attributes::{self, Attributes};
use super::connect;
use super::pointer::{MAX_POINTER, Pointer};
use super::store::{StatCache, Store};
use crate::commit::{Cleaned, Cleaner};
use crate::{Change, Error, Net, Origin, Repo, Result};

/// Where a pulled file lands: the capture's landing in the front, a direct write in tests and clones.
pub trait Landing {
    /// Puts the bytes of `from` at `rel`, whole or not at all.
    fn land(&self, rel: &str, from: &Path) -> std::io::Result<()>;
    /// Removes `rel`, as a pull removes what the remote removed.
    fn remove(&self, rel: &str) -> std::io::Result<()>;
}

/// Writes beside the target under an atomic-write temp name and renames over it.
pub struct Direct(pub PathBuf);

impl Landing for Direct {
    fn remove(&self, rel: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.0.join(rel)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn land(&self, rel: &str, from: &Path) -> std::io::Result<()> {
        let to = self.0.join(rel);
        let dir = to.parent().unwrap_or(&self.0);
        std::fs::create_dir_all(dir)?;
        let name = to.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        let mut nonce = [0u8; 4];
        let _ = getrandom::fill(&mut nonce);
        let temp = dir.join(format!(
            ".{name}.{}.{:08x}.tmp",
            std::process::id(),
            u32::from_le_bytes(nonce)
        ));
        std::fs::copy(from, &temp)?;
        std::fs::rename(&temp, &to).inspect_err(|_| {
            let _ = std::fs::remove_file(&temp);
        })
    }
}

/// The pointer a work file already is, when it is small enough to be one.
fn read_pointer(abs: &Path, size: u64) -> Option<Vec<u8>> {
    if size >= MAX_POINTER as u64 {
        return None;
    }
    let bytes = std::fs::read(abs).ok()?;
    Pointer::parse(&bytes).map(|_| bytes)
}

/// The clean step: a tracked file goes into the store and a pointer takes its place in git.
pub(crate) struct LfsCleaner {
    pub attrs: Attributes,
    pub store: Store,
    pub cache: RefCell<StatCache>,
}

impl Cleaner for LfsCleaner {
    fn clean(&self, rel: &str, abs: &Path, size: u64) -> Result<Cleaned> {
        if !self.attrs.tracks(rel) {
            return Ok(Cleaned::Plain);
        }
        // A file whose bytes already are a pointer is stored as that pointer, never as an object of its own.
        if let Some(own) = read_pointer(abs, size) {
            return Ok(Cleaned::Pointer(own));
        }
        let p = self.store.put_file(abs)?;
        self.cache.borrow_mut().remember(rel, abs, &p);
        Ok(Cleaned::Pointer(p.text().into_bytes()))
    }
}

impl Repo {
    pub(crate) fn lfs_store(&self) -> Store {
        Store::in_gitdir(&self.gitdir)
    }

    /// A line in `.gitattributes` for each large binary file no line covers yet, before the commit reads the tree.
    pub(crate) fn prepare_attributes(&self) -> Result<()> {
        let attrs = Attributes::read(&self.workdir);
        let mut added = false;
        for d in self.deltas()? {
            if d.change == Change::Removed || attrs.tracks(&d.new_path) {
                continue;
            }
            let abs = self.workdir.join(&d.new_path);
            if std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0) > attributes::BIG
                && crate::changes::looks_binary(&crate::changes::file_head(&abs))
            {
                attributes::add_path(&self.workdir, &d.new_path)?;
                added = true;
            }
        }
        let _ = added;
        Ok(())
    }

    /// Every large file a tree holds, by path.
    pub fn lfs_pointers(&self, tree: &git2::Tree<'_>) -> Result<Vec<(String, Pointer)>> {
        let mut out = Vec::new();
        tree.walk(TreeWalkMode::PreOrder, |dir, e| {
            if e.kind() == Some(ObjectType::Blob)
                && let Some(p) = self.pointer_blob(e.id())
            {
                out.push((format!("{dir}{}", e.name().unwrap_or("")), p));
            }
            TreeWalkResult::Ok
        })?;
        Ok(out)
    }

    /// The large files `HEAD` holds.
    pub fn lfs_head(&self) -> Result<Vec<(String, Pointer)>> {
        match self.head_tree() {
            Some(t) => self.lfs_pointers(&t),
            None => Ok(Vec::new()),
        }
    }

    /// The large files the remote branch's tip holds: the objects the host is known to have.
    pub fn lfs_held(&self) -> Result<Vec<(String, Pointer)>> {
        let tip = self.remote_tip().and_then(|t| Oid::from_str(&t).ok());
        match tip.map(|t| self.git.find_commit(t).and_then(|c| c.tree())) {
            Some(tree) => self.lfs_pointers(&tree?),
            None => Ok(Vec::new()),
        }
    }

    /// The prune a pull or a merge may do: only objects the remote's tip holds, so an unpushed take is never lost.
    pub fn lfs_prune_held(&self) -> Result<()> {
        self.lfs_prune(&self.lfs_held()?);
        Ok(())
    }

    /// The objects the commits `local` has beyond `remote` name: what a push owes the host.
    pub(crate) fn lfs_owed(
        &self,
        local: Oid,
        remote: Option<Oid>,
    ) -> Result<Vec<(String, Pointer)>> {
        let mut walk = self.git.revwalk()?;
        walk.push(local)?;
        if let Some(r) = remote {
            let _ = walk.hide(r);
        }
        let mut out: Vec<(String, Pointer)> = Vec::new();
        for id in walk {
            let c = self.git.find_commit(id?)?;
            let parent = c.parent(0).ok().map(|p| p.tree()).transpose()?;
            let others = (1..c.parent_count())
                .map(|i| c.parent(i)?.tree())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let diff = self
                .git
                .diff_tree_to_tree(parent.as_ref(), Some(&c.tree()?), None)?;
            for d in diff.deltas() {
                if !matches!(
                    d.status(),
                    Delta::Added | Delta::Modified | Delta::Renamed | Delta::Copied
                ) {
                    continue;
                }
                let Some(path) = d
                    .new_file()
                    .path()
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                else {
                    continue;
                };
                let id = d.new_file().id();
                // A merge owes only what every parent lacks: what a parent holds the host has through it.
                let known = others
                    .iter()
                    .any(|t| t.get_path(Path::new(&path)).is_ok_and(|e| e.id() == id));
                if !known
                    && let Some(p) = self.pointer_blob(id)
                    && !out.iter().any(|(_, q)| q.oid == p.oid)
                {
                    out.push((path, p));
                }
            }
        }
        Ok(out)
    }

    /// Makes sure each owed object is in the store, re-adding it from a work file with the same hash.
    pub(crate) fn lfs_stock(&self, owed: &[(String, Pointer)]) -> Result<()> {
        let store = self.lfs_store();
        let mut cache = StatCache::load(&self.gitdir);
        for (rel, p) in owed {
            if store.has(&p.oid) {
                continue;
            }
            let abs = self.workdir.join(rel);
            let held = cache.pointer(rel, &abs).is_ok_and(|q| q.oid == p.oid);
            if !held {
                return Err(Error::Network(format!(
                    "the bytes of {rel} are not on this device: restore the file before pushing"
                )));
            }
            store.put_file(&abs)?;
        }
        cache.save();
        Ok(())
    }

    /// Sends the owed objects to the host, before the ref moves; a host without LFS is `NoLfs` and nothing is sent.
    pub(crate) fn lfs_send(
        &self,
        net: &Arc<Net>,
        origin: &Origin,
        owed: &[(String, Pointer)],
    ) -> Result<()> {
        if owed.is_empty() {
            return Ok(());
        }
        self.lfs_stock(owed)?;
        let objs: Vec<Pointer> = owed.iter().map(|(_, p)| p.clone()).collect();
        connect(net, origin)?.upload(&self.lfs_store(), &objs)
    }

    /// Whether the host takes large files: none when it does, the sentence for the screen when it does not.
    pub fn lfs_probe(&self, net: &Arc<Net>, origin: &Origin) -> Option<String> {
        match connect(net, origin).and_then(|t| t.probe()) {
            Err(e @ Error::NoLfs { .. }) => Some(e.to_string()),
            _ => None,
        }
    }

    /// The files whose bytes are not in the work tree as the pointers say, with the object each wants.
    fn lfs_wanting(&self, files: &[(String, Pointer)]) -> Vec<(String, Pointer)> {
        let mut cache = StatCache::load(&self.gitdir);
        let out = files
            .iter()
            .filter(|(rel, p)| {
                !cache
                    .pointer(rel, &self.workdir.join(rel))
                    .is_ok_and(|q| q.oid == p.oid)
            })
            .cloned()
            .collect();
        cache.save();
        out
    }

    /// Brings in the objects of `files` the store lacks, then lands each file whose bytes differ from its pointer.
    pub fn lfs_pull(
        &self,
        net: &Arc<Net>,
        origin: &Origin,
        files: &[(String, Pointer)],
        landing: &dyn Landing,
    ) -> Result<()> {
        crate::paths::check_all(files.iter().map(|(p, _)| p.as_str()))?;
        let store = self.lfs_store();
        let wanting = self.lfs_wanting(files);
        let missing: Vec<&(String, Pointer)> =
            wanting.iter().filter(|(_, p)| !store.has(&p.oid)).collect();
        if !missing.is_empty() {
            let mut objs: Vec<Pointer> = missing.iter().map(|(_, p)| p.clone()).collect();
            objs.dedup_by(|a, b| a.oid == b.oid);
            let first = missing[0].0.clone();
            connect(net, origin)?
                .download(&store, &objs)
                .map_err(|e| match e {
                    Error::Network(_) => Error::Offline { path: first },
                    e => e,
                })?;
        }
        // Every object is in the store before the first file lands, or nothing does.
        if let Some((rel, _)) = wanting.iter().find(|(_, p)| !store.has(&p.oid)) {
            return Err(Error::Offline { path: rel.clone() });
        }
        let mut cache = StatCache::load(&self.gitdir);
        for (rel, p) in &wanting {
            landing.land(rel, &store.path(&p.oid))?;
            cache.remember(rel, &self.workdir.join(rel), p);
        }
        cache.save();
        Ok(())
    }

    /// Brings the objects of `files` into the store without landing any file.
    pub fn lfs_fetch_objects(
        &self,
        net: &Arc<Net>,
        origin: &Origin,
        files: &[(String, Pointer)],
    ) -> Result<()> {
        let store = self.lfs_store();
        let mut objs: Vec<Pointer> = files
            .iter()
            .filter(|(_, p)| !store.has(&p.oid))
            .map(|(_, p)| p.clone())
            .collect();
        objs.dedup_by(|a, b| a.oid == b.oid);
        if objs.is_empty() {
            return Ok(());
        }
        connect(net, origin)?.download(&store, &objs)
    }

    /// The large files of commit `theirs` whose bytes are not in the work tree: what a pull must bring in.
    pub fn lfs_wanted_by(&self, theirs: &str) -> Result<Vec<(String, Pointer)>> {
        let id = Oid::from_str(theirs).map_err(|e| Error::Git(e.message().into()))?;
        let files = self.lfs_pointers(&self.git.find_commit(id)?.tree()?)?;
        Ok(self.lfs_wanting(&files))
    }

    /// Removes from the store each object of `files` that the host has and the work tree holds with the same hash.
    pub fn lfs_prune(&self, files: &[(String, Pointer)]) {
        let store = self.lfs_store();
        for (rel, p) in files {
            let abs = self.workdir.join(rel);
            // A deletion is decided from the bytes as they are now, never from a cached stat, and never through a link.
            let plain = std::fs::symlink_metadata(&abs).is_ok_and(|m| m.is_file());
            if plain
                && store.has(&p.oid)
                && crate::lfs::store::hash_file(&abs).is_ok_and(|q| q.oid == p.oid)
            {
                let _ = store.remove(&p.oid);
            }
        }
    }
}
