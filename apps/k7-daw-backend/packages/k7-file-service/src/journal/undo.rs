//! undo.rs — an undo or a redo: alone behind every record in flight and ahead of any asked later (1730, 1734), one side of the
//! step applied, the cursor moved only after the files did (1751, 1752). What a reader gets wrong: blobs are fetched BEFORE this
//! takes the gate (1743); a step that changed or a blob still missing under the gate answers `Retry`, for the caller to go again.

use serde_json::Value;

use super::apply::{ApplyError, Writer, apply_side};
use super::blobs::{BlobError, Missing, Side, delete, steps};
use super::format::{EngineRef, Entry};
use super::{Journals, locked, ring};
use crate::libraries::Home;

/// One undo or redo asked.
pub struct Undo<'a> {
    pub home: &'a Home,
    pub root: &'a str,
    pub dir: &'a str,
    pub redo: bool,
    /// The step the caller fetched blobs for; another on top means start again.
    pub expect: Option<u64>,
    /// The engine bound now.
    pub bound: Option<EngineRef>,
    /// The workspace's two gates for a recorded path (1795): the absolute path, or their sentence.
    pub confine: &'a (dyn Fn(&str) -> Result<String, String> + Sync),
    pub writer: &'a Writer<'a>,
    /// Blob copies fetched for this step, deleted once written over the project (1743).
    pub fetched: &'a [String],
}

/// What an undo needs before it takes the gate: the step on top, the blobs to fetch, the engine that keeps them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Planned {
    pub id: Option<u64>,
    pub missing: Vec<Missing>,
    pub kept: Option<EngineRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Undone {
    /// Nothing to undo (or redo).
    Nothing,
    /// Applied: the step's peek.
    Moved(Value),
    /// The step changed, or a blob is not here yet: fetch again and ask again.
    Retry(Vec<Missing>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    Blob(BlobError),
    Apply(ApplyError),
    Journal(String),
}

impl Journals {
    /// The blobs an undo of the step on top needs and does not have, or the sentence it stops with: for the caller to fetch.
    pub fn plan(
        &self,
        home: &Home,
        root: &str,
        dir: &str,
        redo: bool,
        bound: Option<&EngineRef>,
    ) -> Result<(Option<u64>, Vec<Missing>), BlobError> {
        self.plan_by(home, root, dir, redo, &|_| bound.cloned())
            .map(|p| (p.id, p.missing))
    }

    /// `plan` for a caller that does not know the engine bound: the step's own is assumed and answered (1743).
    pub fn plan_kept(
        &self,
        home: &Home,
        root: &str,
        dir: &str,
        redo: bool,
    ) -> Result<Planned, BlobError> {
        self.plan_by(home, root, dir, redo, &|e| e.kept_on.clone().flatten())
    }

    fn plan_by(
        &self,
        home: &Home,
        root: &str,
        dir: &str,
        redo: bool,
        bound: &dyn Fn(&Entry) -> Option<EngineRef>,
    ) -> Result<Planned, BlobError> {
        let r = self.root(root);
        let mut st = locked(&r.state);
        let open = Self::open_in(&mut st, home, dir);
        let j = &open.held.journal;
        let at = if redo {
            j.cursor
        } else {
            j.cursor.wrapping_sub(1)
        };
        let Some(e) = j.entries.get(at).filter(|_| redo || j.cursor > 0) else {
            return Ok(Planned::default());
        };
        let side = if redo { Side::After } else { Side::Before };
        let kept = bound(e);
        let mut missing = Vec::new();
        for s in steps(&open.store, e, side, kept.as_ref()) {
            missing.push(s?);
        }
        Ok(Planned {
            id: Some(e.id),
            missing,
            kept,
        })
    }

    /// Apply the step on top, alone: after every record in flight, before any asked later (1730).
    pub async fn undo(&self, u: &Undo<'_>) -> Result<Undone, Failure> {
        let gate = self.root(u.root).gate.clone();
        let _alone = gate.write_owned().await;
        self.undo_held(u)
    }

    /// The part under the gate.
    pub fn undo_held(&self, u: &Undo<'_>) -> Result<Undone, Failure> {
        let r = self.root(u.root);
        let mut st = locked(&r.state);
        let open = Self::open_in(&mut st, u.home, u.dir);
        if let Some(why) = &open.held.unread {
            return Err(Failure::Journal(why.clone()));
        }
        let j = &open.held.journal;
        let at = if u.redo {
            j.cursor
        } else {
            j.cursor.wrapping_sub(1)
        };
        let Some(e) = j
            .entries
            .get(at)
            .filter(|_| u.redo || j.cursor > 0)
            .cloned()
        else {
            return Ok(Undone::Nothing);
        };
        if u.expect.is_some_and(|id| id != e.id) {
            return Ok(Undone::Retry(vec![]));
        }
        let side = if u.redo { Side::After } else { Side::Before };
        let mut missing = Vec::new();
        for s in steps(&open.store, &e, side, u.bound.as_ref()) {
            missing.push(s.map_err(Failure::Blob)?);
        }
        if !missing.is_empty() {
            return Ok(Undone::Retry(missing));
        }
        apply_side(&open.store, &e, side, u.confine, u.writer).map_err(Failure::Apply)?;
        // The files are written; only now does the cursor move (1752).
        let open = st.open.as_mut().expect("opened");
        open.held.journal.cursor = if u.redo { at + 1 } else { at };
        let store = open.store.clone();
        let peek = ring::peek(&e);
        self.save(&mut st).map_err(Failure::Journal)?;
        delete(&store, u.fetched);
        Ok(Undone::Moved(peek))
    }
}
