//! rel.rs — `toRel`, an absolute path as the caller names it: a library file answers to `_libraries/<name>/...`.
//! What a reader gets wrong: it is lexical, so a canonical path (`\\?\C:`, `/private/var`) never reaches an answer (1220).

use crate::libraries::library_of;
use crate::paths::relative;
use crate::space::Space;

/// The folder a library path starts with; `space.rs` holds the same word, and a round-trip test the two equal.
const LIBRARIES_DIR: &str = "_libraries";

/// `toRel`: the library's virtual path, `.` for the root, else the path below the root, `/`-spelled.
pub fn to_rel(space: &Space<'_>, abs: &str) -> String {
    if let Some(lib) = library_of(space.home(), abs) {
        let below = relative(&lib.root, abs);
        let mut out = format!("{LIBRARIES_DIR}/{}", lib.name);
        if !below.is_empty() {
            out.push('/');
            out.push_str(&below);
        }
        return out;
    }
    if abs == space.root() {
        return ".".into();
    }
    relative(space.root(), abs)
}
