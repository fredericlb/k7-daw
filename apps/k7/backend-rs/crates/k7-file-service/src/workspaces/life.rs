//! life.rs — a workspace made and removed: `createWorkspace` and the disk's part of `removeWorkspace`.
//! What a reader gets wrong: neither sends a word to the capture; the end of a removal's sync, the pins of a new
//! workspace and every memo are the front's own (1621, 1622), and a removal here is only the disk.

use std::fs;

use super::WsError;
use super::names::{name_ok, on_disk, workspace_dir};
use crate::filter::code_of;
use crate::libraries::{Home, is_library_name};
use crate::paths::{basename, dirname, join};
use crate::space::Declined;

/// The folder beside the workspaces that holds each one's open-project marker.
const MARKERS_DIR: &str = ".active-projects";

/// Whether the account has a library on disk: a folder of the hub's libraries folder with a library's name.
fn has_library(home: &Home) -> bool {
    fs::read_dir(&home.libraries).is_ok_and(|read| {
        read.flatten().any(|e| {
            e.file_type().is_ok_and(|t| t.is_dir())
                && is_library_name(&e.file_name().to_string_lossy())
        })
    })
}

/// `createWorkspace` judged: whether it would be created. No library on disk is declined (1662), whose refusal asks the hub.
pub fn plan_make(home: &Home, name: &str) -> Result<bool, WsError> {
    if !has_library(home) {
        return Err(Declined::NoLibraries.into());
    }
    if !name_ok(name) {
        return Err(WsError::Invalid { name: name.into() });
    }
    Ok(fs::metadata(join(&[&workspace_dir(home), name])).is_err())
}

/// `createWorkspace`: the folder made whole unless it is there; whether it was made.
pub fn make(home: &Home, name: &str) -> Result<bool, WsError> {
    if !name_ok(name) {
        return Err(WsError::Invalid { name: name.into() });
    }
    let root = join(&[&workspace_dir(home), name]);
    if fs::metadata(&root).is_ok() {
        return Ok(false);
    }
    fs::create_dir_all(&root).map_err(|e| WsError::Io {
        code: code_of(&e),
        syscall: "mkdir",
        path: root.clone(),
    })?;
    Ok(true)
}

/// `removeWorkspace` judged: the root of the workspace in the folder's own spelling (1632).
pub fn plan_remove(home: &Home, name: &str) -> Result<String, WsError> {
    if !name_ok(name) {
        return Err(WsError::Invalid { name: name.into() });
    }
    let dir = workspace_dir(home);
    let spelled = on_disk(&dir, name).ok_or_else(|| WsError::NoSuch { name: name.into() })?;
    Ok(join(&[&dir, &spelled]))
}

/// The shim's `rmSync(path, { force: true, recursive })`: a link is removed and not followed, a junction is a folder,
/// and a path that is not there is not an error.
pub fn rm(path: &str, recursive: bool) -> Result<(), WsError> {
    let fail = |e: &std::io::Error| WsError::Io {
        code: code_of(e),
        syscall: "rm",
        path: path.into(),
    };
    let m = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(fail(&e)),
    };
    let r = if m.is_dir() {
        if recursive {
            fs::remove_dir_all(path)
        } else {
            fs::remove_dir(path)
        }
    } else {
        // A Windows junction is a folder to the file API.
        fs::remove_file(path).or_else(|e| {
            if m.file_type().is_symlink() {
                fs::remove_dir(path)
            } else {
                Err(e)
            }
        })
    };
    match r {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other.map_err(|e| fail(&e)),
    }
}

/// The folder and everything under it, for good, then its marker beside it (not in it): no workspace made again under
/// the name reopens a ghost.
pub fn remove(_home: &Home, root: &str) -> Result<(), WsError> {
    rm(root, true)?;
    rm(
        &join(&[&dirname(root), MARKERS_DIR, &basename(root)]),
        false,
    )
}
