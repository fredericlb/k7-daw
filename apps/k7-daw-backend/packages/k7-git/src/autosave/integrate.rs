//! integrate.rs — taking in what the person's other devices auto-saved: their snapshot lands in the work tree, a
//! concurrent one is merged file by file, and the main branch follows theirs when it is only behind.
//! What a reader gets wrong: a file both sides changed keeps the NEWER snapshot's version whole (never a line merge),
//! the other staying in the branch's history; a main branch that diverged from theirs waits for the person's Update.

use git2::{IndexEntry, IndexTime, Oid};

use crate::lfs::{Landing, Pointer};
use crate::merge::assemble::Content;
use crate::merge::plan::{Case, case_of, files_of, paths, tree_of};
use crate::{Identity, Repo, Result};

/// What an integration came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// The remote holds nothing this device lacks.
    Nothing,
    /// Not now: a merge waits, the files moved meanwhile, or the main branches diverged.
    Wait,
    /// `files` changed in the work tree; `main_moved` when `HEAD` followed the other device's main commit.
    Landed { files: usize, main_moved: bool },
}

/// Brings the named large files' objects into the store before anything lands.
pub type FetchObjects<'a> = dyn Fn(&[(String, Pointer)]) -> Result<()> + 'a;

fn entry(path: &str, id: Oid, mode: u32) -> IndexEntry {
    IndexEntry {
        ctime: IndexTime::new(0, 0),
        mtime: IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: path.len().min(0xFFF) as u16,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

impl Repo {
    /// Flushes this device's edits, then takes in the remote's `branch` (fetched by the caller) through `landing`.
    pub fn autosave_integrate(
        &self,
        branch: &str,
        who: &Identity,
        device: &str,
        landing: &dyn Landing,
        fetch: &FetchObjects<'_>,
    ) -> Result<Picked> {
        if self.pending_file().exists() {
            return Ok(Picked::Wait);
        }
        self.autosave_commit(branch, who, device)?;
        let (tip, remote) = self.autosave_tips(branch);
        let Some(r) = remote else {
            return Ok(Picked::Nothing);
        };
        let head = self.head_id();
        let mine = tip.or(head);
        if mine.is_some_and(|m| self.has_in_history(m, r)) {
            return Ok(Picked::Nothing);
        }
        let theirs_base = self.base_of(r);
        let new_head = match (head, theirs_base) {
            (h, b) if h == b => h,
            (Some(h), Some(b)) if self.has_in_history(h, b) => Some(h),
            (Some(h), Some(b)) if self.has_in_history(b, h) => Some(b),
            (None, b) => b,
            (Some(h), None) => Some(h),
            _ => return Ok(Picked::Wait),
        };
        let (target, tree) = match mine {
            Some(m) if !self.has_in_history(r, m) || new_head != theirs_base => {
                let tree = self.merge_snapshots(m, r)?;
                let parents = [self.git.find_commit(m)?, self.git.find_commit(r)?];
                let id = self.snapshot_commit(
                    who,
                    device,
                    "Auto-save: merge",
                    tree,
                    new_head,
                    &parents,
                )?;
                (id, tree)
            }
            _ => (r, self.git.find_commit(r)?.tree_id()),
        };
        let before = files_of(tree_of(&self.git, mine).as_ref());
        let after = files_of(Some(&self.git.find_tree(tree)?));
        let mut plan = Vec::new();
        for path in paths(&before, &after, &Default::default()) {
            let (b, a) = (before.get(&path), after.get(&path));
            if b != a {
                let c = a.map_or(Content::Gone, |(i, m)| Content::Blob {
                    id: i.to_string(),
                    mode: *m,
                });
                plan.push((path, c));
            }
        }
        crate::paths::check_all(plan.iter().map(|(p, _)| p.as_str()))?;
        self.refuse_case_clash(plan.iter().map(|(p, _)| p.as_str()))?;
        let store = self.lfs_store();
        let wanted: Vec<(String, Pointer)> = plan
            .iter()
            .filter_map(|(p, c)| match c {
                Content::Blob { id, .. } => Oid::from_str(id)
                    .ok()
                    .and_then(|o| self.pointer_blob(o))
                    .filter(|ptr| !store.has(&ptr.oid))
                    .map(|ptr| (p.clone(), ptr)),
                _ => None,
            })
            .collect();
        if !wanted.is_empty() {
            fetch(&wanted)?;
        }
        self.require_objects(&plan)?;
        // An edit made while the objects came in stays: this turn lands nothing and the next one merges it.
        let expected = match mine {
            Some(m) => self.git.find_commit(m)?.tree_id(),
            None => self.git.treebuilder(None)?.write()?,
        };
        if self.work_tree()? != expected {
            return Ok(Picked::Wait);
        }
        self.land_all(landing, &plan)?;
        let main_moved = new_head != head;
        if let (true, Some(to), Some(name)) = (main_moved, new_head, self.branch()) {
            let refname = format!("refs/heads/{name}");
            match head {
                Some(old) => self.git.reference_matching(
                    &refname,
                    to,
                    true,
                    old,
                    "k7: auto-save follows the main branch",
                )?,
                None => self.git.reference(&refname, to, false, "k7: auto-save")?,
            };
            let mut index = self.git.index()?;
            index.read_tree(&self.git.find_commit(to)?.tree()?)?;
            index.write()?;
        }
        self.git.reference(
            &format!("refs/heads/{branch}"),
            target,
            true,
            "k7: auto-save taken in",
        )?;
        Ok(Picked::Landed {
            files: plan.len(),
            main_moved,
        })
    }

    /// The tree of two concurrent snapshots: each side's own changes, and the newer side's file where both changed it.
    fn merge_snapshots(&self, mine: Oid, theirs: Oid) -> Result<Oid> {
        let base = self.git.merge_base(mine, theirs).ok();
        let (b, m, t) = (
            files_of(tree_of(&self.git, base).as_ref()),
            files_of(tree_of(&self.git, Some(mine)).as_ref()),
            files_of(tree_of(&self.git, Some(theirs)).as_ref()),
        );
        let stamp = |id: Oid| {
            let time = self.git.find_commit(id).map_or(0, |c| c.time().seconds());
            (time, id)
        };
        // The id breaks a tie, so both devices pick the same side.
        let mine_newer = stamp(mine) > stamp(theirs);
        let mut index = git2::Index::new()?;
        if let Some(tree) = tree_of(&self.git, Some(mine)) {
            index.read_tree(&tree)?;
        }
        for path in paths(&b, &m, &t) {
            let pick = match case_of(
                b.get(&path).copied(),
                m.get(&path).copied(),
                t.get(&path).copied(),
            ) {
                Case::Same | Case::Mine => continue,
                Case::Theirs(x) => x,
                Case::Both { .. } if mine_newer => continue,
                Case::Both { theirs, .. } => theirs,
            };
            match pick {
                None => index.remove_path(std::path::Path::new(&path))?,
                Some((id, mode)) => index.add(&entry(&path, id, mode))?,
            }
        }
        Ok(index.write_tree_to(&self.git)?)
    }
}
