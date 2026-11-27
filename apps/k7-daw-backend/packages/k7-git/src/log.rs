//! log.rs — the branch's commits, newest first, and how far it stands from the remote's.
//! What a reader gets wrong: `before` is a cursor (the page after that commit), and a merge's changed files
//! are read against its first parent.

use git2::Sort;

use crate::{Error, Repo, Result};

pub use git_rules::LogEntry;

fn device_of(message: &str) -> Option<String> {
    let last = message.trim_end().rsplit("\n\n").next()?;
    last.lines()
        .find_map(|l| l.strip_prefix("K7-Device:"))
        .map(|d| d.trim().to_string())
}

impl Repo {
    /// Up to `limit` commits from `HEAD`, those after `before` when it is given.
    pub fn log(&self, limit: usize, before: Option<&str>) -> Result<Vec<LogEntry>> {
        let Ok(head) = self.git.head().and_then(|h| h.peel_to_commit()) else {
            return Ok(Vec::new());
        };
        let mut walk = self.git.revwalk()?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        walk.push(head.id())?;
        let mut skipping = before.is_some();
        let mut out = Vec::new();
        for id in walk {
            let id = id?;
            if skipping {
                skipping = Some(id.to_string().as_str()) != before;
                continue;
            }
            if out.len() >= limit {
                break;
            }
            out.push(self.entry(id)?);
        }
        Ok(out)
    }

    fn entry(&self, id: git2::Oid) -> Result<LogEntry> {
        let c = self.git.find_commit(id)?;
        let tree = c.tree()?;
        let parent = c.parent(0).ok().map(|p| p.tree()).transpose()?;
        let diff = self
            .git
            .diff_tree_to_tree(parent.as_ref(), Some(&tree), None)?;
        let message = c.message().unwrap_or("");
        Ok(LogEntry {
            id: id.to_string(),
            subject: message.lines().next().unwrap_or("").trim().to_string(),
            author: c.author().name().unwrap_or("").to_string(),
            time: c.time().seconds(),
            device: device_of(message),
            files_changed: diff.deltas().len(),
        })
    }

    /// How many commits `HEAD` holds: what is ahead of a remote that has no branch yet.
    pub fn head_commits(&self) -> Result<usize> {
        let Ok(head) = self.git.refname_to_id("HEAD") else {
            return Ok(0);
        };
        let mut walk = self.git.revwalk()?;
        walk.push(head)?;
        Ok(walk.count())
    }

    /// `(ahead, behind)` of `HEAD` against `refs/remotes/origin/<branch>`; none when there is no such ref.
    pub fn ahead_behind(&self) -> Result<Option<(usize, usize)>> {
        let branch = self
            .branch()
            .ok_or_else(|| Error::Git("HEAD names no branch".into()))?;
        let Ok(up) = self
            .git
            .refname_to_id(&format!("refs/remotes/origin/{branch}"))
        else {
            return Ok(None);
        };
        let Ok(local) = self.git.refname_to_id("HEAD") else {
            let mut walk = self.git.revwalk()?;
            walk.push(up)?;
            return Ok(Some((0, walk.count())));
        };
        Ok(Some(self.git.graph_ahead_behind(local, up)?))
    }
}
