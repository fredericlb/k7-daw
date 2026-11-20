//! plan.rs — a write judged and not done: every guard and stat `workspaceEntries.ts` runs, in its order, and the
//! answer the route would give. What a reader gets wrong: nothing is created here, not a folder, a bin or a temp.

use serde_json::{Value, json};

use super::apply::recycle_target_in;
use super::guards::{guard_mutable, guard_shared, is_recycled, not_engine, ref_at, refuse_yaml};
use super::{EntryError, Op, Outcome, Probe};
use crate::libraries::library_of;
use crate::paths::{basename, dirname, is_within, join};
use crate::rel::to_rel;
use crate::space::{Space, js_trim};

/// `isPlainSegment`: not empty, not `.`, not `..`.
pub fn is_plain(seg: &str) -> bool {
    !seg.is_empty() && seg != "." && seg != ".."
}

/// `basename(name).replace(/[\\/]/g, "").trim()`: the one segment a folder or a rename name keeps.
pub fn clean_name(name: &str) -> String {
    let base = basename(name).replace(['/', '\\'], "");
    js_trim(&base).to_string()
}

/// `dir` as the routes join it: `.` or empty is the root, a trailing `/` dropped.
fn dir_base(dir: &str) -> &str {
    if dir.is_empty() || dir == "." {
        ""
    } else {
        dir.trim_end_matches('/')
    }
}

fn joined(dir: &str, clean: &str) -> String {
    let base = dir_base(dir);
    if base.is_empty() {
        clean.to_string()
    } else {
        format!("{base}/{clean}")
    }
}

/// The rel an upload of `name` into `dir` is gated by (1226): segments trimmed as JavaScript trims, blanks dropped.
pub fn upload_rel(dir: &str, name: &str) -> Result<String, EntryError> {
    let segs: Vec<&str> = name
        .split(['\\', '/'])
        .map(js_trim)
        .filter(|s| !s.is_empty())
        .collect();
    if segs.is_empty() || segs.iter().any(|s| *s == "." || *s == "..") {
        return Err(EntryError::Refused(format!("invalid filename: {name}")));
    }
    Ok(joined(dir, &segs.join("/")))
}

/// The write `op` asks, judged, with nothing written: the outcome it would have, or why it is refused.
pub fn plan(space: &Space<'_>, op: &Op<'_>, probe: &dyn Probe) -> Result<Outcome, EntryError> {
    match *op {
        Op::Folder { dir, name } => folder(space, dir, name, probe),
        Op::Rename { path, name } => rename(space, path, name, probe),
        Op::Move { path, to_dir } => move_to(space, path, to_dir, probe),
        Op::Delete { path } => delete(space, path, probe),
        Op::Upload { dir, name } => upload(space, dir, name, probe),
    }
}

fn exists_or_not_found(abs: &str, rel: &str, probe: &dyn Probe) -> Result<(), EntryError> {
    if probe.exists(abs) {
        Ok(())
    } else {
        Err(EntryError::NotFound { rel: rel.into() })
    }
}

/// The rel `createFolderIn` gates: the cleaned name below `dir`, or the sentence for a name that is no segment.
pub fn folder_rel(dir: &str, name: &str) -> Result<String, EntryError> {
    let clean = clean_name(name);
    if !is_plain(&clean) {
        return Err(EntryError::Refused(format!("invalid folder name: {name}")));
    }
    Ok(joined(dir, &clean))
}

/// The name a rename keeps: one plain segment that is no YAML name.
pub fn renamed(name: &str) -> Result<String, EntryError> {
    let clean = clean_name(name);
    if !is_plain(&clean) {
        return Err(EntryError::Refused(format!("invalid name: {name}")));
    }
    refuse_yaml(&clean)?;
    Ok(clean)
}

fn folder(
    space: &Space<'_>,
    dir: &str,
    name: &str,
    probe: &dyn Probe,
) -> Result<Outcome, EntryError> {
    let rel = folder_rel(dir, name)?;
    let abs = space.safe_path(&rel, probe)?;
    guard_shared(space, &abs, "create a folder in")?;
    if is_recycled(space, &abs) {
        return Err(EntryError::Refused(
            "can't create a folder in the recycle bin".into(),
        ));
    }
    if probe.exists(&abs) {
        return Err(EntryError::Exists { rel });
    }
    Ok(Outcome::Folder { abs })
}

fn rename(
    space: &Space<'_>,
    path: &str,
    name: &str,
    probe: &dyn Probe,
) -> Result<Outcome, EntryError> {
    let abs = space.safe_path(path, probe)?;
    exists_or_not_found(&abs, path, probe)?;
    let active = space.active()?;
    guard_mutable(space, active.as_deref(), &abs, "rename")?;
    let library = library_of(space.home(), &abs);
    let old_ref = library.as_ref().and_then(|l| ref_at(l, &abs));
    if let (Some(l), Some(_)) = (&library, &old_ref) {
        not_engine(l, &abs)?;
    }
    let clean = renamed(name)?;
    let dest = space.safe_path(&to_rel(space, &join(&[&dirname(&abs), &clean])), probe)?;
    if dest == abs {
        return Ok(Outcome::Unchanged { abs });
    }
    if probe.exists(&dest) {
        return Err(EntryError::Exists {
            rel: to_rel(space, &dest),
        });
    }
    let new_ref = library.as_ref().and_then(|l| ref_at(l, &dest));
    let refs = old_ref.zip(new_ref).map(|(a, b)| (a, b, true));
    Ok(Outcome::Rename {
        from: abs,
        to: dest,
        refs,
    })
}

fn move_to(
    space: &Space<'_>,
    path: &str,
    to_dir: &str,
    probe: &dyn Probe,
) -> Result<Outcome, EntryError> {
    let abs = space.safe_path(path, probe)?;
    exists_or_not_found(&abs, path, probe)?;
    let active = space.active()?;
    guard_mutable(space, active.as_deref(), &abs, "move")?;
    let dest_dir = space.safe_path(to_dir, probe)?;
    guard_shared(space, &dest_dir, "move into")?;
    if !probe.is_dir(&dest_dir) {
        return Err(EntryError::Refused(format!("not a folder: {to_dir}")));
    }
    if is_recycled(space, &dest_dir) {
        return Err(EntryError::Refused(
            "can't move something into the recycle bin — delete it instead".into(),
        ));
    }
    if dest_dir == dirname(&abs) {
        return Ok(Outcome::Unchanged { abs });
    }
    if is_within(&abs, &dest_dir) {
        return Err(EntryError::Refused(format!(
            "can't move '{}' into itself",
            basename(&abs)
        )));
    }
    let dest = join(&[&dest_dir, &basename(&abs)]);
    if probe.exists(&dest) {
        return Err(EntryError::Exists {
            rel: to_rel(space, &dest),
        });
    }
    Ok(Outcome::Move {
        from: abs,
        to: dest,
    })
}

fn delete(space: &Space<'_>, path: &str, probe: &dyn Probe) -> Result<Outcome, EntryError> {
    let abs = space.safe_path(path, probe)?;
    exists_or_not_found(&abs, path, probe)?;
    let active = space.active()?;
    guard_mutable(space, active.as_deref(), &abs, "delete")?;
    let root = library_of(space.home(), &abs).map_or_else(|| space.root().to_string(), |l| l.root);
    let bin = join(&[&root, super::guards::RECYCLE_DIR]);
    let to = recycle_target_in(&bin, &basename(&abs), &|p| probe.exists(p));
    Ok(Outcome::Recycle { from: abs, to })
}

fn upload(
    space: &Space<'_>,
    dir: &str,
    name: &str,
    probe: &dyn Probe,
) -> Result<Outcome, EntryError> {
    let rel = upload_rel(dir, name)?;
    refuse_yaml(&rel)?;
    let abs = space.safe_path(&rel, probe)?;
    guard_shared(space, &abs, "add a file to")?;
    if probe.exists(&abs) {
        return Err(EntryError::Exists { rel });
    }
    Ok(Outcome::Upload { abs })
}

/// The route's body for an outcome, in the literal's key order; what a plan cannot know (`repointed`) is not in it.
pub fn answer_of(space: &Space<'_>, done: &Outcome) -> Value {
    match done {
        Outcome::Folder { abs } | Outcome::Unchanged { abs } | Outcome::Upload { abs } => {
            json!({ "path": to_rel(space, abs) })
        }
        Outcome::Rename { to, .. } | Outcome::Move { to, .. } => {
            json!({ "path": to_rel(space, to) })
        }
        Outcome::Recycle { to, .. } => json!({ "recycled": to_rel(space, to) }),
    }
}
