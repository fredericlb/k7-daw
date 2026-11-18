//! filter.rs — which names a workspace hides, which files it never reads as text, which folders claim a project,
//! and the errno name of an I/O error: `workspaceFilter.ts`, `projectApp` and `host/fs.rs`.
//! What a reader gets wrong: `code_of` and `rename_retry` hold the crate's `cfg!(windows)`, the host's own (32 and 33 are EBUSY).

use crate::paths::extname;

/// Files shown in the tree but not editable as text.
const BINARY_EXT: [&str; 14] = [
    ".wav",
    ".mp3",
    ".ogg",
    ".flac",
    ".vstpreset",
    ".png",
    ".ico",
    ".jpg",
    ".jpeg",
    ".mp4",
    ".mov",
    ".m4v",
    ".webm",
    ".mkv",
];

/// `isBinary`: the lower-cased extension (`.wav` alone is no extension) is in the table.
pub fn is_binary(name: &str) -> bool {
    let ext = extname(name).to_lowercase();
    BINARY_EXT.contains(&ext.as_str())
}

/// `skip`: hidden files, dependencies, the recycle bin, the journal and the engine's waveform sidecars.
pub fn skip(name: &str) -> bool {
    name.starts_with('.')
        || name == "node_modules"
        || name == "_recycle_bin"
        || name == "_history"
        || name.ends_with(".cache")
}

/// `projectApp`: the app a folder with this name claims; the name alone, whether it compiles is another question.
pub fn project_app(name: &str) -> Option<&'static str> {
    (name.len() > ".k7".len() && name.ends_with(".k7")).then_some("k7")
}

/// node's errno name for an I/O error, as `host/fs.rs` names it.
pub fn code_of(e: &std::io::Error) -> &'static str {
    use std::io::ErrorKind::*;
    // A file another process holds without sharing (ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION).
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return "EBUSY";
    }
    match e.kind() {
        NotFound => "ENOENT",
        ResourceBusy => "EBUSY",
        AlreadyExists => "EEXIST",
        PermissionDenied => "EACCES",
        NotADirectory => "ENOTDIR",
        IsADirectory => "EISDIR",
        DirectoryNotEmpty => "ENOTEMPTY",
        InvalidInput | InvalidFilename => "EINVAL",
        _ => "EIO",
    }
}

/// Whether a rename is retried: only Windows lets a scanner or an indexer hold a file (1750).
pub fn rename_retry() -> bool {
    cfg!(windows)
}
