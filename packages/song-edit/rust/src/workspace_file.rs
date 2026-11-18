// workspace_file.rs — a workspace file by its own name: plain I/O, a .k7x file's text as it is on disk.
// A reader would get wrong that this converts anything: it does not; workspace_doc.rs reads data and edits documents.

use crate::error::{Res, io_error};
use crate::fs::Fs;
use crate::sugar::sugar_for;
use k7x::edit::{Value, parse};

pub const K7X_EXT: &str = ".k7x";

/// `/\.k7x$/i`: the dot is a dot here, unlike `isDocFile`'s.
#[must_use]
pub fn is_k7x_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(K7X_EXT)
}

/// Is `name` a workspace document: a .k7x file, the only format a workspace holds.
#[must_use]
pub fn is_doc_name(name: &str) -> bool {
    is_k7x_path(name)
}

/// The data of a .k7x file already read, its short forms expanded by the kind its path names.
///
/// # Errors
/// A syntax error of the text, or what a short form does not read.
pub fn k7x_data(real: &str, raw: &str) -> Res<Value> {
    let data = parse(raw)?;
    match sugar_for(real) {
        Some(s) => s.try_expand(&data),
        None => Ok(data),
    }
}

#[must_use]
pub fn ws_exists(fs: &dyn Fs, path: &str) -> bool {
    fs.exists(path)
}

/// A file's text as it is on disk; fails like `readFileSync` when absent, and a byte that is not UTF-8 reads as U+FFFD.
///
/// # Errors
/// The `ENOENT` sentence of Node.
pub fn ws_read(fs: &dyn Fs, path: &str) -> Res<String> {
    let bytes = fs.read(path, song::Span::All).map_err(|e| io_error(&e, "open", path))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Writes a file whole or not at all.
///
/// # Errors
/// The file system's.
pub fn ws_write(fs: &mut dyn Fs, path: &str, content: &[u8]) -> Res<()> {
    fs.write_atomic(path, content).map_err(|e| io_error(&e, "open", path))
}

/// # Errors
/// A folder at `path`.
pub fn ws_remove(fs: &mut dyn Fs, path: &str) -> Res<()> {
    fs.remove(path).map_err(|e| io_error(&e, "rm", path))
}

/// # Errors
/// The file system's.
pub fn ws_rename(fs: &mut dyn Fs, from: &str, to: &str) -> Res<()> {
    fs.rename(from, to).map_err(|e| io_error(&e, "rename", from))
}

/// # Errors
/// The file system's.
pub fn ws_copy(fs: &mut dyn Fs, from: &str, to: &str) -> Res<()> {
    fs.copy(from, to).map_err(|e| io_error(&e, "copyfile", from))
}

/// A folder's entry names, sorted bytewise where `readdir` gives disk order.
///
/// # Errors
/// The `ENOENT` sentence of Node.
pub fn ws_list(fs: &dyn Fs, dir: &str) -> Res<Vec<String>> {
    Ok(fs
        .list(dir)
        .map_err(|e| io_error(&e, "scandir", dir))?
        .into_iter()
        .map(|e| e.name)
        .collect())
}
