//! path.rs — the names a replica gives the relay, checked before any reaches the disk: a workspace file's path
//! (`@studio/live-sync`'s `path.ts`), a workspace name, a library name, and the room folder they make.
//! What a reader gets wrong: a room folder is the workspace name ENCODED, so no name can climb out of `.sync-relay`.

use std::path::{Path, PathBuf};

use crate::json::{encode_uri_component, utf16_len};

/// The prefix that names a room of the shared account (the libraries) rather than one of the caller's.
pub const SHARED_PREFIX: &str = "@lib/";

/// The folder under an account's that holds its rooms.
pub const RELAY_DIR: &str = ".sync-relay";

/// Why `file` cannot name a workspace file, or None when it can (`syncPathError`).
pub fn sync_path_error(file: &str) -> Option<String> {
    if file.is_empty() {
        return Some("empty path".into());
    }
    if file.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}') {
        return Some("control character".into());
    }
    if file.contains('\\') {
        return Some("backslash".into());
    }
    let b = file.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return Some("drive letter".into());
    }
    if file.starts_with('/') {
        return Some("absolute path".into());
    }
    file.split('/')
        .find(|seg| seg.is_empty() || *seg == "." || *seg == "..")
        .map(|seg| format!("'{seg}' segment"))
}

/// The throw of `assertSyncPath`, or Ok.
pub fn assert_sync_path(file: &str) -> Result<(), String> {
    match sync_path_error(file) {
        None => Ok(()),
        Some(why) => Err(format!(
            "refused sync path {}: {why}",
            crate::json::stringify(&serde_json::Value::String(file.into()))
        )),
    }
}

/// A workspace name as a single folder name: nothing that could climb out of `.sync-relay`.
pub fn is_workspace_name(w: &str) -> bool {
    !w.is_empty()
        && utf16_len(w) <= 255
        && w != "."
        && w != ".."
        && !w.contains(['/', '\\'])
        && !w.contains('\u{0}')
}

/// Names a library cannot take (`RESERVED_LIBRARY_NAMES` in `song-schema`).
const RESERVED_LIBRARY_NAMES: [&str; 6] = [
    "engine",
    "workspace",
    "account",
    "meta",
    "studio",
    "mediatek",
];

/// `isLibraryName`: lowercase letters, digits and dashes, starting with a letter or digit, at most 40.
pub fn is_library_name(name: &str) -> bool {
    let b = name.as_bytes();
    let ok = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b.len() <= 40
        && ok(&b[0])
        && b[1..].iter().all(|c| ok(c) || *c == b'-')
        && !RESERVED_LIBRARY_NAMES.contains(&name)
}

/// A room's folder under an account's.
pub fn room_dir(account: &Path, workspace: &str) -> PathBuf {
    account
        .join(RELAY_DIR)
        .join(encode_uri_component(workspace))
}

/// Why a `hello` or `batch` cannot name this room, or None (`roomNameError`).
pub fn room_name_error(workspace: &str, shared: bool) -> Option<&'static str> {
    let Some(name) = workspace.strip_prefix(SHARED_PREFIX) else {
        return (!is_workspace_name(workspace)).then_some("not a workspace name");
    };
    if !shared {
        return Some("no library is followed here");
    }
    (!is_library_name(name)).then_some("not a library name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_paths() {
        for ok in ["a.txt", "dir/b.k7x", "é/ü", ".hidden", "a..b"] {
            assert_eq!(sync_path_error(ok), None, "{ok}");
        }
        for (bad, why) in [
            ("", "empty path"),
            ("a\nb", "control character"),
            ("a\u{7f}", "control character"),
            ("a\\b", "backslash"),
            ("C:x", "drive letter"),
            ("/etc", "absolute path"),
            ("a/../b", "'..' segment"),
            ("a//b", "'' segment"),
            ("./a", "'.' segment"),
            ("a/", "'' segment"),
        ] {
            assert_eq!(sync_path_error(bad).as_deref(), Some(why), "{bad:?}");
        }
        assert_eq!(
            assert_sync_path("../x").unwrap_err(),
            "refused sync path \"../x\": '..' segment"
        );
    }

    #[test]
    fn names() {
        assert!(is_workspace_name("band"));
        assert!(is_workspace_name("a b%"));
        assert!(!is_workspace_name(".."));
        assert!(!is_workspace_name("a/b"));
        assert!(!is_workspace_name(&"x".repeat(256)));
        assert!(is_library_name("k7-core"));
        assert!(!is_library_name("-x"));
        assert!(!is_library_name("Core"));
        assert!(!is_library_name("meta"));
        assert!(!is_library_name("mediatek"));
        assert_eq!(
            room_name_error("@lib/k7-core", false),
            Some("no library is followed here")
        );
        assert_eq!(room_name_error("@lib/K", true), Some("not a library name"));
        assert_eq!(room_name_error("@lib/k7-core", true), None);
        assert_eq!(
            room_dir(Path::new("/d"), "a b/"),
            Path::new("/d").join(".sync-relay").join("a%20b%2F")
        );
    }
}
