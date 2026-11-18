//! marker.rs — the active-project marker `<studio>/workspace/.active-projects/<workspace>`: where it lives, how it is
//! read the way `defaultProject` reads it, and how it is written (2020 to 2025).
//! What a reader gets wrong: `read_raw` does NOT confine the rel to the root (`Space::active` does, 1040).

use std::fs;
use std::io::{ErrorKind, Read};

use super::ProjError;
use crate::atomic::write_atomic;
use crate::filter::code_of;
use crate::paths::{basename, dirname, is_within, join, resolve, resolve_all};
use crate::space::{Declined, MAX_REL, Space, js_trim};
use crate::workspaces::names::is_project_dir;

/// The folder of the libraries below a workspace, which no marker may name (1040).
const LIBRARIES_DIR: &str = "_libraries";
/// The folder beside the workspaces that holds each one's marker.
const MARKERS_DIR: &str = ".active-projects";
/// The marker a workspace once kept inside itself, which only a migration upgrades (2024).
const LEGACY_MARKER: &str = ".studio-active";
/// The most of a marker that is read; past it the marker is no path anyone wrote.
const MAX_MARKER: u64 = 1 << 20;

/// The marker's file: `<dirname(root)>/.active-projects/<basename(root)>`, `activeMarkerOf`.
pub fn marker_of(root: &str) -> String {
    let abs = resolve_all(&[root]);
    join(&[&dirname(&abs), MARKERS_DIR, &basename(&abs)])
}

/// Whether a failed read of the marker means "none": absent, below a file, or a folder where the file belongs (F4).
pub fn reads_as_none(kind: ErrorKind, is_folder: impl FnOnce() -> bool) -> bool {
    match kind {
        ErrorKind::NotFound | ErrorKind::NotADirectory | ErrorKind::IsADirectory => true,
        // Windows refuses to open a folder as a file.
        ErrorKind::PermissionDenied => is_folder(),
        _ => false,
    }
}

/// `readActiveMarker`: the trimmed rel resolved against the root, not confined (2030); `Declined` where a migration upgrades (2024).
pub fn read_raw(root: &str) -> Result<Option<String>, Declined> {
    let mut bytes = Vec::new();
    let read = fs::File::open(marker_of(root))
        .and_then(|f| f.take(MAX_MARKER + 1).read_to_end(&mut bytes));
    if let Err(e) = read {
        // Only a marker that is not there reads as none; any other failure is declined.
        let folder = || fs::symlink_metadata(marker_of(root)).is_ok_and(|m| m.is_dir());
        if !reads_as_none(e.kind(), folder) {
            return Err(Declined::WouldThrow);
        }
        // A legacy marker is moved by a read, a write the native side never makes.
        if fs::symlink_metadata(join(&[root, LEGACY_MARKER])).is_ok() {
            return Err(Declined::LegacyMarker);
        }
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let rel = js_trim(&text);
    if bytes.len() as u64 > MAX_MARKER || rel.len() > MAX_REL {
        return Err(Declined::LongPath);
    }
    Ok((!rel.is_empty()).then(|| resolve(root, rel)))
}

/// The open project as `Space::active` names it, but a marker that cannot be read declines instead of reading as none (F2).
pub fn active_strict(space: &Space<'_>) -> Result<Option<String>, Declined> {
    let root = space.root();
    if is_project_dir(root) {
        return Ok(Some(root.to_string()));
    }
    let Some(abs) = read_raw(root)? else {
        return Ok(None);
    };
    if is_within(&join(&[root, LIBRARIES_DIR]), &abs) {
        return Err(Declined::LibraryActive);
    }
    Ok((is_within(root, &abs) && is_project_dir(&abs)).then_some(abs))
}

fn io(e: &std::io::Error, syscall: &'static str, path: &str) -> ProjError {
    ProjError::Io {
        code: code_of(e),
        syscall,
        path: path.to_string(),
    }
}

/// `<rel>\n`, `""` for none, through `write_atomic`, the folder made first; whether the file changed (2022).
pub fn write_if_changed(root: &str, rel: &str) -> Result<bool, ProjError> {
    if rel.len() > MAX_REL {
        return Err(Declined::LongPath.into());
    }
    let at = marker_of(root);
    let text = if rel.is_empty() {
        String::new()
    } else {
        format!("{rel}\n")
    };
    if fs::read(&at).is_ok_and(|now| now == text.as_bytes()) {
        return Ok(false);
    }
    let folder = dirname(&at);
    fs::create_dir_all(&folder).map_err(|e| io(&e, "mkdir", &folder))?;
    write_atomic(&at, text.as_bytes()).map_err(|e| io(&e, "rename", &at))?;
    Ok(true)
}

/// `write_if_changed`, the answer dropped.
pub fn write(root: &str, rel: &str) -> Result<(), ProjError> {
    write_if_changed(root, rel).map(|_| ())
}

/// The marker emptied: no project open.
pub fn clear(root: &str) -> Result<(), ProjError> {
    write(root, "")
}
