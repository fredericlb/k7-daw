//! resolve.rs — the end of a merge, and the fast-forward: files land through the capture's landing, the index is
//! set, `HEAD` moves. What a reader gets wrong: every file is written before `HEAD` moves and `k7-merge.json` goes
//! last, so a crash in between leaves a merge that resolves again, never a commit over files that are not there.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{IndexEntry, IndexTime, Oid, Signature};

use super::assemble::Content;
use super::{Begin, Choice};
use crate::lfs::{Landing, Pointer};
use crate::{Error, Identity, Repo, Result};

/// Numbers the landing files: two operations never share a path, whatever lock they hold.
static NEXT: AtomicU64 = AtomicU64::new(0);

fn index_entry(meta: &std::fs::Metadata, id: Oid, path: &str, mode: u32) -> IndexEntry {
    let t = |m: std::io::Result<std::time::SystemTime>| {
        let d = m
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .unwrap_or_default();
        IndexTime::new(d.as_secs() as i32, d.subsec_nanos())
    };
    IndexEntry {
        ctime: t(meta.modified()),
        mtime: t(meta.modified()),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: meta.len() as u32,
        id,
        flags: path.len().min(0xFFF) as u16,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

/// A path and what now stands there: its blob and mode, or nothing when it was removed.
type Landed = (String, Option<(Oid, u32)>);

impl Repo {
    /// One file of the new tree written to the work tree: bytes through the landing, a pointer's object from the store.
    fn land_file(
        &self,
        landing: &dyn Landing,
        rel: &str,
        c: &Content,
    ) -> Result<Option<(Oid, u32)>> {
        let tmp = self.gitdir.join("k7-merge-tmp");
        std::fs::create_dir_all(&tmp)?;
        let (bytes, id, mode): (Option<Vec<u8>>, Oid, u32) = match c {
            Content::Gone => {
                landing.remove(rel)?;
                return Ok(None);
            }
            Content::Blob { id, mode } => {
                let oid = Oid::from_str(id).map_err(|e| Error::Git(e.message().into()))?;
                (None, oid, *mode)
            }
            Content::Bytes { bytes, mode } => (Some(bytes.clone()), self.git.blob(bytes)?, *mode),
        };
        let from = tmp.join(format!(
            "land-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let large = self.pointer_blob(id);
        match (&bytes, large) {
            (None, Some(p)) => self.land_object(landing, rel, &p)?,
            (Some(b), _) => {
                std::fs::write(&from, b)?;
                landing.land(rel, &from)?;
            }
            (None, None) => {
                std::fs::write(&from, self.git.find_blob(id)?.content())?;
                landing.land(rel, &from)?;
            }
        }
        let _ = std::fs::remove_file(&from);
        Ok(Some((id, mode)))
    }

    /// Lands every file, each only while it is as the plan saw it: an edit made meanwhile stays and nothing else lands.
    pub(crate) fn land_all(
        &self,
        landing: &dyn Landing,
        files: &[(String, Content)],
    ) -> Result<Vec<Landed>> {
        let seen: Vec<_> = files.iter().map(|(p, _)| self.stamp(p)).collect();
        let mut changed = Vec::new();
        for ((path, content), was) in files.iter().zip(seen) {
            if self.stamp(path) != was {
                return Err(Error::Uncommitted);
            }
            changed.push((path.clone(), self.land_file(landing, path, content)?));
        }
        Ok(changed)
    }

    /// What a work file looks like from outside: whether it exists, its size and its time.
    fn stamp(&self, rel: &str) -> Option<(u64, Option<std::time::SystemTime>)> {
        let m = std::fs::symlink_metadata(self.workdir.join(rel)).ok()?;
        Some((m.len(), m.modified().ok()))
    }

    fn land_object(&self, landing: &dyn Landing, rel: &str, p: &Pointer) -> Result<()> {
        let store = self.lfs_store();
        if !store.has(&p.oid) {
            return Err(Error::Offline {
                path: rel.to_string(),
            });
        }
        landing.land(rel, &store.path(&p.oid))?;
        Ok(())
    }

    /// Sets the index to `tree` over the paths that changed, with stat of the files as landed.
    fn index_to(&self, changed: &[(String, Option<(Oid, u32)>)]) -> Result<git2::Oid> {
        let mut index = self.git.index()?;
        let in_step = match self.head_tree() {
            Some(t) => index.write_tree().is_ok_and(|id| id == t.id()),
            None => index.is_empty(),
        };
        if !in_step {
            match self.head_tree() {
                Some(t) => index.read_tree(&t)?,
                None => index.clear()?,
            }
        }
        for (path, now) in changed {
            match now {
                None => {
                    let _ = index.remove_path(Path::new(path));
                }
                Some((id, mode)) => {
                    let meta = std::fs::metadata(self.workdir.join(path))?;
                    index.add(&index_entry(&meta, *id, path, *mode))?;
                }
            }
        }
        index.write()?;
        Ok(index.write_tree()?)
    }

    /// Moves a branch that is only behind to `theirs`: the files land, `HEAD` follows, no commit is made.
    pub fn merge_fast_forward(
        &self,
        theirs: &str,
        planned_at: Option<&str>,
        landing: &dyn Landing,
    ) -> Result<()> {
        let id = Oid::from_str(theirs).map_err(|e| Error::Git(e.message().into()))?;
        let new = self.git.find_commit(id)?.tree()?;
        let old = self.head_tree();
        crate::paths::check_all(super::plan::files_of(Some(&new)).keys().map(String::as_str))?;
        let (before, after) = (
            super::plan::files_of(old.as_ref()),
            super::plan::files_of(Some(&new)),
        );
        let mut plan = Vec::new();
        for path in super::plan::paths(&before, &after, &Default::default()) {
            let (b, a) = (before.get(&path), after.get(&path));
            if b == a {
                continue;
            }
            let content = a.map_or(Content::Gone, |(i, m)| Content::Blob {
                id: i.to_string(),
                mode: *m,
            });
            plan.push((path, content));
        }
        self.refuse_case_clash(plan.iter().map(|(p, _)| p.as_str()))?;
        // Every byte is here before the first file lands, or nothing moves.
        self.require_objects(&plan)?;
        // The downloads took time: the branch and the work tree must still be what the plan saw.
        self.still_as_planned(planned_at, &plan)?;
        let changed = self.land_all(landing, &plan)?;
        self.index_to(&changed)?;
        let branch = self
            .branch()
            .ok_or_else(|| Error::Git("HEAD names no branch".into()))?;
        let name = format!("refs/heads/{branch}");
        match planned_at {
            Some(old) => {
                let old = Oid::from_str(old).map_err(|e| Error::Git(e.message().into()))?;
                self.git
                    .reference_matching(&name, id, true, old, "k7: pull")
                    .map_err(|_| Error::MergeMoved)?;
            }
            None => {
                self.git
                    .reference(&name, id, false, "k7: pull")
                    .map_err(|_| Error::MergeMoved)?;
            }
        }
        Ok(())
    }

    /// The branch and the work tree are as they were when the merge was planned, except for files already holding their planned bytes.
    fn still_as_planned(&self, head: Option<&str>, files: &[(String, Content)]) -> Result<()> {
        let now = self.git.refname_to_id("HEAD").ok().map(|h| h.to_string());
        if now.as_deref() != head {
            return Err(Error::MergeMoved);
        }
        for e in self.status()?.into_iter().filter(|e| !e.too_large) {
            let planned = files.iter().find(|(path, _)| *path == e.path);
            if !planned.is_some_and(|(_, c)| self.holds(&e.path, c)) {
                return Err(Error::Uncommitted);
            }
        }
        Ok(())
    }

    /// Whether the file at `rel` already is what `c` would write there.
    fn holds(&self, rel: &str, c: &Content) -> bool {
        let abs = self.workdir.join(rel);
        let want: Vec<u8> = match c {
            Content::Gone => return !abs.exists(),
            Content::Bytes { bytes, .. } => bytes.clone(),
            Content::Blob { id, .. } => {
                let Ok(oid) = Oid::from_str(id) else {
                    return false;
                };
                let Ok(b) = self.git.find_blob(oid) else {
                    return false;
                };
                if let Some(ptr) = self.pointer_blob(oid) {
                    return crate::lfs::store::hash_file(&abs).is_ok_and(|h| h.oid == ptr.oid);
                }
                b.content().to_vec()
            }
        };
        std::fs::read(abs).is_ok_and(|have| have == want)
    }

    /// The bytes of every large file about to land are in the store, or nothing lands.
    pub(crate) fn require_objects(&self, files: &[(String, Content)]) -> Result<()> {
        let store = self.lfs_store();
        for (rel, c) in files {
            if let Content::Blob { id, .. } = c
                && let Ok(oid) = Oid::from_str(id)
                && let Some(p) = self.pointer_blob(oid)
                && !store.has(&p.oid)
            {
                return Err(Error::Offline { path: rel.clone() });
            }
        }
        Ok(())
    }

    /// Every conflict chosen: the files land, the merge commit has both parents, the pending merge goes.
    pub fn merge_resolve(
        &self,
        choices: &[(String, Choice)],
        landing: &dyn Landing,
        who: &Identity,
        device: &str,
    ) -> Result<String> {
        if !who.is_complete() {
            return Err(Error::NoIdentity);
        }
        for (id, c) in choices {
            self.merge_choose(id, *c)?;
        }
        let p = self.merge_pending()?.ok_or(Error::NoMerge)?;
        let a = self.assemble(&p)?;
        if a.waiting > 0 {
            return Err(Error::Unresolved { n: a.waiting });
        }
        let head = self
            .git
            .find_commit(Oid::from_str(&p.head).map_err(|e| Error::Git(e.message().into()))?)?;
        let theirs = self
            .git
            .find_commit(Oid::from_str(&p.theirs).map_err(|e| Error::Git(e.message().into()))?)?;
        crate::paths::check_all(a.files.iter().map(|(p, _)| p.as_str()))?;
        self.refuse_case_clash(a.files.iter().map(|(p, _)| p.as_str()))?;
        self.still_as_planned(Some(&p.head), &a.files)?;
        self.require_objects(&a.files)?;
        let changed = self.land_all(landing, &a.files)?;
        let tree = self.git.find_tree(self.index_to(&changed)?)?;
        let sig = Signature::now(who.name.trim(), who.email.trim())?;
        let msg = format!(
            "Merge {}\n\nK7-Device: {}\n",
            &p.theirs[..p.theirs.len().min(7)],
            device.replace(['\n', '\r'], " ").trim()
        );
        let id = self
            .git
            .commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[&head, &theirs])?;
        let _ = std::fs::remove_dir_all(self.gitdir.join("k7-merge-tmp"));
        self.merge_abort()?;
        Ok(id.to_string())
    }

    /// Begins the merge of `theirs`; a fast-forward is done at once through `landing`.
    pub fn merge_or_forward(&self, theirs: &str, landing: &dyn Landing) -> Result<Begin> {
        let b = self.merge_begin(theirs)?;
        if let Begin::FastForward { theirs, head } = &b {
            self.merge_fast_forward(theirs, head.as_deref(), landing)?;
        }
        Ok(b)
    }
}
