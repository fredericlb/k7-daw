//! commit.rs — a planned set landed whole or not at all (3-D7): every new file staged beside its target, every old one renamed
//! aside, the new ones renamed in; any failure renames everything back and removes what the set made.
//! What a reader gets wrong: the caller holds the write lock; the disk is `Fs`, so a test makes the Nth rename fail.

use std::fs::File;
use std::io;
use std::time::Duration;

use super::plan::{Act, Planned};
use crate::atomic::{Fs, rename_retrying, temp_for};
use crate::filter::{code_of, is_binary, rename_retry};
use crate::paths::{basename, dirname};

/// A failed landing: the errno name and a sentence that names no folder of the server; the disk is as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    pub code: &'static str,
    pub what: String,
}

/// What a landing changed, for the caller's notices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Landed {
    /// Every absolute path the set wrote, removed or moved away.
    pub touched: Vec<String>,
    /// A file appeared or went, or a folder was made: the listings are outdated.
    pub structural: bool,
}

enum Undo {
    /// Put the old file back over whatever the set put there.
    Restore { backup: String, target: String },
    /// The set made this file; take it away.
    Remove(String),
    /// A moved file goes back where it came from.
    Back { from: String, to: String },
}

struct Step {
    target: String,
    /// The new content, staged; `None` for a removal and for a move (its source is renamed).
    staged: Option<String>,
    /// A move's source.
    source: Option<String>,
    /// A staged output taken from a job, removed after the landing.
    consumed: Option<String>,
}

fn failed(e: &io::Error, what: &str) -> Failed {
    Failed {
        code: code_of(e),
        what: format!("{what}: {e}"),
    }
}

fn rename(fs: &dyn Fs, from: &str, to: &str) -> io::Result<()> {
    rename_retrying(fs, from, to, rename_retry(), &|d: Duration| {
        std::thread::sleep(d)
    })
}

fn exists(p: &str) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// The folders above `abs` that do not exist yet, outermost first.
fn missing_parents(abs: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut dir = dirname(abs);
    while !exists(&dir) && dirname(&dir) != dir {
        out.push(dir.clone());
        dir = dirname(&dir);
    }
    out.reverse();
    out
}

struct Rollback<'a> {
    fs: &'a dyn Fs,
    staged: Vec<String>,
    made: Vec<String>,
    undo: Vec<Undo>,
}

impl Rollback<'_> {
    /// Everything the set did, undone in reverse; a step that cannot be undone is skipped, never fatal.
    fn run(self) {
        for u in self.undo.iter().rev() {
            match u {
                Undo::Restore { backup, target } => {
                    let _ = std::fs::remove_file(target);
                    let _ = rename(self.fs, backup, target);
                }
                Undo::Remove(t) => self.fs.remove(t),
                Undo::Back { from, to } => {
                    let _ = rename(self.fs, to, from);
                }
            }
        }
        for s in &self.staged {
            self.fs.remove(s);
        }
        for d in self.made.iter().rev() {
            let _ = std::fs::remove_dir(d);
        }
    }
}

/// Lands the set. `snapshot` is asked, once the staging has succeeded and before the first rename, for every binary file about to be
/// replaced or removed, so the journal can keep its pre-image; a text file is the journal's image already.
pub fn commit(
    fs: &dyn Fs,
    planned: &Planned,
    snapshot: &mut dyn FnMut(&str),
) -> Result<Landed, Failed> {
    let mut rb = Rollback {
        fs,
        staged: Vec::new(),
        made: Vec::new(),
        undo: Vec::new(),
    };
    let steps = match stage(&mut rb, planned) {
        Ok(s) => s,
        Err(e) => {
            rb.run();
            return Err(e);
        }
    };
    let mut landed = Landed {
        structural: !rb.made.is_empty(),
        ..Landed::default()
    };
    for s in &steps {
        for p in [Some(&s.target), s.source.as_ref()].into_iter().flatten() {
            if is_binary(&basename(p)) && exists(p) {
                snapshot(p);
            }
        }
    }
    let mut backups = Vec::new();
    for s in &steps {
        if let Err(e) = land(&mut rb, s, &mut backups, &mut landed) {
            rb.run();
            return Err(e);
        }
    }
    for b in &backups {
        fs.remove(b);
    }
    for s in &steps {
        if let Some(c) = &s.consumed {
            fs.remove(c);
        }
    }
    Ok(landed)
}

/// The staging: parents made and each new file written beside its target; no target is touched.
fn stage(rb: &mut Rollback<'_>, planned: &Planned) -> Result<Vec<Step>, Failed> {
    let mut steps = Vec::new();
    for a in &planned.acts {
        let (_, abs) = a.target();
        let abs = abs.to_string();
        match a {
            Act::Text { text, .. } => {
                if std::fs::read(&abs).is_ok_and(|b| b == text.as_bytes()) {
                    continue;
                }
                let tmp = stage_with(rb, &abs, &mut text.as_bytes())?;
                steps.push(Step {
                    target: abs,
                    staged: Some(tmp),
                    source: None,
                    consumed: None,
                });
            }
            Act::Read { .. } => {}
            Act::Remove { .. } => {
                if !exists(&abs) {
                    continue;
                }
                steps.push(Step {
                    target: abs,
                    staged: None,
                    source: None,
                    consumed: None,
                });
            }
            Act::Copy { from, .. } | Act::Adopt { from, .. } => {
                let mut f = File::open(from).map_err(|e| failed(&e, "read"))?;
                let tmp = stage_with(rb, &abs, &mut f)?;
                let consumed = matches!(a, Act::Adopt { .. }).then(|| from.clone());
                steps.push(Step {
                    target: abs,
                    staged: Some(tmp),
                    source: None,
                    consumed,
                });
            }
            Act::Move { from, .. } => {
                for d in missing_parents(&abs) {
                    std::fs::create_dir(&d).map_err(|e| failed(&e, "mkdir"))?;
                    rb.made.push(d);
                }
                steps.push(Step {
                    target: abs,
                    staged: None,
                    source: Some(from.clone()),
                    consumed: None,
                });
            }
        }
    }
    Ok(steps)
}

/// The parents of `abs` made, then `data` written to a hidden temp beside it with the target's mode.
fn stage_with(rb: &mut Rollback<'_>, abs: &str, data: &mut dyn io::Read) -> Result<String, Failed> {
    for d in missing_parents(abs) {
        std::fs::create_dir(&d).map_err(|e| failed(&e, "mkdir"))?;
        rb.made.push(d);
    }
    let tmp = temp_for(abs);
    rb.staged.push(tmp.clone());
    rb.fs
        .write_temp(&tmp, data, rb.fs.mode_of(abs))
        .map_err(|e| failed(&e, "write"))?;
    Ok(tmp)
}

/// One step landed: the old file aside, the new one in, each undo logged before the rename it undoes.
fn land(
    rb: &mut Rollback<'_>,
    s: &Step,
    backups: &mut Vec<String>,
    landed: &mut Landed,
) -> Result<(), Failed> {
    let fs = rb.fs;
    if exists(&s.target) {
        let backup = temp_for(&s.target);
        rename(fs, &s.target, &backup).map_err(|e| failed(&e, "rename"))?;
        // Logged after the rename it undoes: a refused one leaves the old file where it is, and nothing to restore.
        rb.undo.push(Undo::Restore {
            backup: backup.clone(),
            target: s.target.clone(),
        });
        backups.push(backup);
        if s.staged.is_none() && s.source.is_none() {
            landed.structural = true;
        }
    } else {
        rb.undo.push(Undo::Remove(s.target.clone()));
        landed.structural = true;
    }
    landed.touched.push(s.target.clone());
    if let Some(tmp) = &s.staged {
        rename(fs, tmp, &s.target).map_err(|e| failed(&e, "rename"))?;
        rb.staged.retain(|t| t != tmp);
    }
    if let Some(from) = &s.source {
        rb.undo.push(Undo::Back {
            from: from.clone(),
            to: s.target.clone(),
        });
        rename(fs, from, &s.target).map_err(|e| failed(&e, "rename"))?;
        landed.touched.push(from.clone());
        landed.structural = true;
    }
    Ok(())
}
