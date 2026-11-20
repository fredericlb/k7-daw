//! walk.rs — every file under a scope of a workspace, minus what the workspace hides (`filesUnder`, 2131).
//! What a reader gets wrong: a dirent's own type decides, so a link is neither a file nor a folder and is never entered.

use std::io::{self, ErrorKind};

use k7_file_service::paths::join;

use crate::disk::confined_path;

/// A folder's entries as (name, is folder, is file), by name; none when it went away during the walk.
fn entries_of(dir: &str) -> io::Result<Vec<(String, bool, bool)>> {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut out = vec![];
    for e in read {
        let e = e?;
        let t = e.file_type()?;
        out.push((
            e.file_name().to_string_lossy().into_owned(),
            t.is_dir(),
            t.is_file(),
        ));
    }
    out.sort();
    Ok(out)
}

/// Whether a folder below `root` on the way to `rel` is a link: nothing is read or written through one (departs from the TypeScript).
pub fn through_link(root: &str, rel: &str) -> io::Result<bool> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut at = root.to_string();
    for part in &parts[..parts.len().saturating_sub(1)] {
        at = join(&[&at, part]);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

/// Every file under `scope` of `root`, depth first in name order; only what is gone reads as empty, a locked path is an error.
pub fn files_under(root: &str, scope: &str, hide: fn(&str) -> bool) -> io::Result<Vec<String>> {
    let abs = if scope.is_empty() {
        root.to_string()
    } else {
        let abs =
            confined_path(root, scope).map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))?;
        if through_link(root, scope)? {
            return Ok(vec![]);
        }
        abs
    };
    // A scope that is itself a link is neither file nor folder; only the root may be one.
    let stat = if scope.is_empty() {
        std::fs::metadata(&abs)
    } else {
        std::fs::symlink_metadata(&abs)
    };
    let md = match stat {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    if md.is_file() {
        return Ok(vec![scope.to_string()]);
    }
    if !md.is_dir() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    // An explicit stack of folders being read: a nest a thousand deep must not run out of the blocking pool's 2 MiB.
    let mut stack = vec![(scope.to_string(), entries_of(&abs)?.into_iter())];
    while let Some((rel, entries)) = stack.last_mut() {
        let Some((name, is_dir, is_file)) = entries.next() else {
            stack.pop();
            continue;
        };
        if hide(&name) {
            continue;
        }
        let child = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if is_dir {
            let below = entries_of(&join(&[root, &child]))?.into_iter();
            stack.push((child, below));
        } else if is_file {
            out.push(child);
        }
    }
    Ok(out)
}
