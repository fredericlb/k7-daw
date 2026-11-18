//! names.rs — which names a workspace may have, the folders that are workspaces, the folder's own spelling of a name,
//! and the project folders under a root: `assertWorkspaceName`, `folderNames`, `projectDirsUnder`.
//! What a reader gets wrong: a link is no workspace and no project folder (a dirent's own type), and the walk is bounded.

use std::fs;

use super::BENCH;
use crate::filter::{project_app, skip};
use crate::libraries::Home;
use crate::paths::{basename, is_within, join, relative};
use crate::space::Declined;
use crate::tree::Limits;

/// `<studio>/workspace`, the folder every workspace lives in.
pub fn workspace_dir(home: &Home) -> String {
    join(&[&home.studio, "workspace"])
}

/// `assertWorkspaceName`: one ordinary segment by allow-list, `[A-Za-z0-9][A-Za-z0-9._ -]*`, so never dots alone.
pub fn name_ok(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_alphanumeric()
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b' ' | b'-'))
}

/// A folder whose name claims an app and that exists (links followed, as `statSync`).
pub fn is_project_dir(dir: &str) -> bool {
    project_app(&basename(dir)).is_some() && fs::metadata(dir).is_ok_and(|m| m.is_dir())
}

/// A dirent's path as the shim's `join` spells it: a name holding `\` reads as separators there.
pub(super) fn child(abs: &str, name: &str) -> String {
    if name.contains('\\') {
        join(&[abs, name])
    } else {
        format!("{abs}/{name}")
    }
}

/// A folder's names in the order node's `readdirSync` handed them: by bytes on Unix, the disk's own on Windows (libuv's).
pub(super) fn host_order(names: &mut [String]) {
    if cfg!(unix) {
        names.sort();
    }
}

/// `child`, or none when the dirent's spelling folds to a path that is not strictly below `abs` (`a\..`, `..\x`).
pub(super) fn child_below(abs: &str, name: &str) -> Option<String> {
    let at = child(abs, name);
    (at != abs && is_within(abs, &at)).then_some(at)
}

/// The names of the folders (a dirent's own type, so not a link) in `dir`; every entry counted against `max_seen`.
pub(super) fn dir_names(
    dir: &str,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Vec<(String, bool)>, Declined> {
    let Ok(read) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for e in read {
        let Ok(e) = e else {
            return Ok(Vec::new());
        };
        *seen += 1;
        if *seen > limits.max_seen {
            return Err(Declined::TooMany);
        }
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        out.push((e.file_name().to_string_lossy().into_owned(), is_dir));
    }
    Ok(out)
}

/// `folderNames`, bounded: a dirent folder, no dot, not the bench, in code point order.
pub fn folder_names_in(
    dir: &str,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Vec<String>, Declined> {
    let mut names: Vec<String> = dir_names(dir, limits, seen)?
        .into_iter()
        .filter(|(n, d)| *d && !n.starts_with('.') && n != BENCH)
        .map(|(n, _)| n)
        .collect();
    // UTF-8 byte order is code point order, which is what `byCodePoint` sorts by.
    names.sort();
    Ok(names)
}

/// `folderNames`: every workspace folder under `dir`, hidden servers' included; none when `dir` cannot be read.
pub fn folder_names(dir: &str) -> Vec<String> {
    let no_limit = Limits {
        max_seen: u64::MAX,
        ..Limits::default()
    };
    folder_names_in(dir, &no_limit, &mut 0).unwrap_or_default()
}

/// The spelling of `name` among `names` when the disk answered that it exists: the exact one, else the one equal
/// under ASCII case folding (1632), else the name asked.
pub fn spelled_among(names: &[String], name: &str) -> String {
    if names.iter().any(|n| n == name) {
        return name.into();
    }
    names
        .iter()
        .find(|n| n.eq_ignore_ascii_case(name))
        .cloned()
        .unwrap_or_else(|| name.into())
}

/// The folder's own spelling of `name` in `dir` (1632): `None` when `dir/name` does not exist (`existsSync`), else the
/// dirent equal to it, so a disk that folds case (APFS, NTFS) answers `Songs` for `songs`.
pub fn on_disk(dir: &str, name: &str) -> Option<String> {
    fs::metadata(join(&[dir, name])).ok()?;
    let mut names: Vec<String> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.eq_ignore_ascii_case(name))
        .collect();
    names.sort();
    Some(spelled_among(&names, name))
}

/// `on_disk`, bounded: the dirents it reads count against `limits` in `seen`, `TooMany` past them (F5).
pub fn spelled_in(
    dir: &str,
    name: &str,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Option<String>, Declined> {
    if fs::metadata(join(&[dir, name])).is_err() {
        return Ok(None);
    }
    let mut names: Vec<String> = dir_names(dir, limits, seen)?
        .into_iter()
        .map(|(n, _)| n)
        .filter(|n| n.eq_ignore_ascii_case(name))
        .collect();
    names.sort();
    Ok(Some(spelled_among(&names, name)))
}

struct Frame {
    abs: String,
    rel: String,
    kids: Vec<String>,
    next: usize,
}

fn frame(abs: String, rel: String, limits: &Limits, seen: &mut u64) -> Result<Frame, Declined> {
    let mut kids: Vec<String> = dir_names(&abs, limits, seen)?
        .into_iter()
        .filter(|(n, d)| *d && !skip(n))
        .map(|(n, _)| n)
        .collect();
    kids.sort();
    Ok(Frame {
        abs,
        rel,
        kids,
        next: 0,
    })
}

/// `projectDirsUnder`: every project folder under `root`, `/`-relative, at any depth, a project's own folders walked too.
/// An unreadable folder costs that folder; past `max_seen` entries or `max_depth` folders the walk is declined (1634).
pub fn project_dirs_under(
    root: &str,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Vec<String>, Declined> {
    let mut out = Vec::new();
    let mut stack = vec![frame(root.to_string(), String::new(), limits, seen)?];
    while let Some(top) = stack.last_mut() {
        let Some(name) = top.kids.get(top.next).cloned() else {
            stack.pop();
            continue;
        };
        top.next += 1;
        // A spelling that does not lead below its parent is a path the TypeScript loops on; it is skipped.
        let Some(abs) = child_below(&top.abs, &name) else {
            continue;
        };
        let rel = if name.contains('\\') {
            relative(root, &abs)
        } else if top.rel.is_empty() {
            name
        } else {
            format!("{}/{name}", top.rel)
        };
        if is_project_dir(&abs) {
            out.push(rel.clone());
        }
        if u32::try_from(stack.len()).unwrap_or(u32::MAX) > limits.max_depth {
            return Err(Declined::TooDeep);
        }
        let next = frame(abs, rel, limits, seen)?;
        stack.push(next);
    }
    Ok(out)
}
