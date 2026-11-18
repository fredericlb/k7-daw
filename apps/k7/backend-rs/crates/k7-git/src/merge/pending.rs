//! pending.rs — the merge in progress, in `<gitdir>/k7-merge.json`, written whole or not at all.
//! What a reader gets wrong: the file holds what the person chose and where each path stands, never a file's bytes
//! (those are objects in the repository), so deleting it undoes the merge exactly.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Choice, Conflict};
use crate::{Error, Repo, Result};

pub const FILE: &str = "k7-merge.json";

/// One piece of a text merged by lines: settled text, or the conflict that stands in its place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Seg {
    Ok { text: String },
    Conflict { id: String },
}

/// How a path's final content is made once the choices are in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "plan", rename_all = "lowercase")]
pub enum Plan {
    /// This blob (merged, or taken from one side), or the path removed.
    Result {
        blob: Option<String>,
        mode: u32,
    },
    /// Mine with the k7x ops that found their value applied; each aside op waits on its conflict.
    K7x {
        start: String,
        mode: u32,
        aside: Vec<(String, Value)>,
    },
    Text {
        segs: Vec<Seg>,
        mode: u32,
    },
    /// Mine or theirs whole; both means theirs beside under `<name> (<author>).<ext>`.
    Whole {
        mine: Option<String>,
        theirs: Option<String>,
        mode: u32,
        author: String,
    },
    Presence {
        kept: Option<String>,
        mode: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub plan: Plan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub head: String,
    pub theirs: String,
    pub base: Option<String>,
    pub entries: Vec<Entry>,
    pub conflicts: Vec<Conflict>,
}

/// What the screen reads of a pending merge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub theirs: String,
    pub conflicts: Vec<Conflict>,
    /// The large files of their side the merge needs bytes of, for the caller to bring in first.
    pub lfs: Vec<(String, crate::Pointer)>,
}

impl Repo {
    pub(crate) fn pending_file(&self) -> PathBuf {
        self.gitdir.join(FILE)
    }

    /// The merge waiting for choices, when there is one.
    pub fn merge_pending(&self) -> Result<Option<Pending>> {
        match std::fs::read(self.pending_file()) {
            Ok(b) => serde_json::from_slice(&b)
                .map(Some)
                .map_err(|e| Error::Io(format!("{FILE}: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn merge_save(&self, p: &Pending) -> Result<()> {
        let text = serde_json::to_string_pretty(p).map_err(|e| Error::Io(e.to_string()))?;
        k7_sync::fsx::write_atomic(&self.pending_file(), &text)?;
        Ok(())
    }

    /// Deletes the pending merge and nothing else: `HEAD` and every file are as they were.
    pub fn merge_abort(&self) -> Result<()> {
        match std::fs::remove_file(self.pending_file()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// Records one choice; a choice that makes a `.k7x` unreadable is refused with the line and not kept.
    pub fn merge_choose(&self, id: &str, choice: Choice) -> Result<()> {
        let mut p = self.merge_pending()?.ok_or(Error::NoMerge)?;
        let c = p
            .conflicts
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| Error::BadPath(id.to_string()))?;
        let allowed = match choice {
            Choice::Both => c.both,
            Choice::Mine | Choice::Theirs => c.kind != super::Kind::Presence,
            Choice::Keep | Choice::Remove => c.kind == super::Kind::Presence,
        };
        if !allowed {
            return Err(Error::BadPath(format!(
                "{id}: that choice is not offered here"
            )));
        }
        let before = c.choice;
        c.choice = Some(choice);
        if let Err(e) = self.assemble(&p).map(drop) {
            if let Some(c) = p.conflicts.iter_mut().find(|c| c.id == id) {
                c.choice = before;
            }
            return Err(e);
        }
        self.merge_save(&p)
    }
}
