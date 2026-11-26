//! snapshot.rs — an auto-save: the work tree as a commit on the technical branch, and the reset a Save makes.
//! What a reader gets wrong: neither moves `HEAD` nor writes the index; an unchanged tree on an unchanged base is no commit.

use git2::{Oid, Signature};

use super::base_trailer;
use crate::{Error, Identity, Repo, Result};

impl Repo {
    /// A snapshot of the work tree on `branch`: none when it holds nothing new or a merge waits for choices.
    pub fn autosave_commit(
        &self,
        branch: &str,
        who: &Identity,
        device: &str,
    ) -> Result<Option<String>> {
        if !who.is_complete() {
            return Err(Error::NoIdentity);
        }
        if self.pending_file().exists() {
            return Ok(None);
        }
        let head = self.head_id();
        let tree = self.work_tree()?;
        let (tip, _) = self.autosave_tips(branch);
        let parent = tip.or(head);
        let parent_tree = parent
            .and_then(|p| self.git.find_commit(p).ok())
            .map(|c| c.tree_id());
        let on_head = tip.is_none_or(|t| self.base_of(t) == head);
        if parent_tree == Some(tree) && on_head {
            return Ok(None);
        }
        if parent.is_none() && self.git.find_tree(tree)?.is_empty() {
            return Ok(None);
        }
        let mut parents = Vec::new();
        if let Some(p) = parent {
            parents.push(self.git.find_commit(p)?);
        }
        // The main commit joins as a parent when it moved, so another device's fetch brings it.
        if let (Some(_), false, Some(h)) = (tip, on_head, head) {
            parents.push(self.git.find_commit(h)?);
        }
        let id = self.snapshot_commit(who, device, "Auto-save", tree, head, &parents)?;
        self.git
            .reference(&format!("refs/heads/{branch}"), id, true, "k7: auto-save")?;
        Ok(Some(id.to_string()))
    }

    pub(crate) fn snapshot_commit(
        &self,
        who: &Identity,
        device: &str,
        subject: &str,
        tree: Oid,
        base: Option<Oid>,
        parents: &[git2::Commit<'_>],
    ) -> Result<Oid> {
        let sig = Signature::now(who.name.trim(), who.email.trim())?;
        let msg = format!(
            "{subject}\n\n{}\nK7-Device: {}\n",
            base_trailer(base),
            device.replace(['\n', '\r'], " ").trim()
        );
        let refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
        Ok(self
            .git
            .commit(None, &sig, &sig, &msg, &self.git.find_tree(tree)?, &refs)?)
    }

    /// After a Save: `branch` restarts at `HEAD`, unless the remote holds a snapshot this device never took in.
    pub fn autosave_reset(&self, branch: &str) -> Result<bool> {
        let Some(head) = self.head_id() else {
            return Ok(false);
        };
        let (tip, remote) = self.autosave_tips(branch);
        let seen = match remote {
            None => true,
            Some(r) => tip.is_some_and(|t| self.has_in_history(t, r)),
        };
        if !seen {
            return Ok(false);
        }
        self.git.reference(
            &format!("refs/heads/{branch}"),
            head,
            true,
            "k7: a save resets the auto-save",
        )?;
        let expected = remote.map_or("-".to_string(), |r| r.to_string());
        std::fs::write(self.lease_file(), format!("{branch} {expected}\n"))?;
        Ok(true)
    }

    /// The remote tip a pending reset may replace: `Some(None)` when the remote had no such branch.
    pub(crate) fn lease_of(&self, branch: &str) -> Option<Option<Oid>> {
        let text = std::fs::read_to_string(self.lease_file()).ok()?;
        let (b, tip) = text.trim().split_once(' ')?;
        if b != branch {
            return None;
        }
        Some(Oid::from_str(tip).ok())
    }

    pub(crate) fn drop_lease(&self) {
        let _ = std::fs::remove_file(self.lease_file());
    }
}
