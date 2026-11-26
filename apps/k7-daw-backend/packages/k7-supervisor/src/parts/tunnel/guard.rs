//! guard.rs — what a job checks of a path before it writes or reads it: that it lies in a root the server takes
//! back, or inside this machine's copies of the servers' folders, once links are resolved. The tunnel fetched
//! every file a request names, so these keep a job from going past them. Shared by every job that names a path.

use std::path::{Component, Path, PathBuf};

use crate::core::Ctx;
use crate::paths::utf8;
use crate::tunnel::{JobContext, segments::relative_under};

/// Refuse a job whose `path` would write, or stamp, a file outside a root the server takes back.
pub fn require_writable(ctx: &JobContext, path: &str, what: &str) -> Result<(), String> {
    if ctx.writable(path) {
        return Ok(());
    }
    Err(format!(
        "{what}: refused a path outside this server's writable folders"
    ))
}

/// `p` with its longest existing prefix resolved through links, the rest appended as written.
fn weakly_canonical(p: &Path) -> PathBuf {
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    let mut at = p.to_path_buf();
    loop {
        if let Ok(real) = at.canonicalize() {
            return rest.iter().rev().fold(real, |acc, s| acc.join(s));
        }
        match (at.file_name().map(|n| n.to_os_string()), at.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                at = parent.to_path_buf();
            }
            _ => return p.to_path_buf(),
        }
    }
}

/// Whether a LOCAL path is inside this machine's copies of the servers' roots, by spelling and once links are resolved.
pub(super) fn mapped(cx: &Ctx, path: &str) -> bool {
    let root = cx.paths.studio_dir().join("remote");
    if relative_under(&utf8(&root), path).is_none() {
        return false;
    }
    let (real, real_root) = (weakly_canonical(Path::new(path)), weakly_canonical(&root));
    real.starts_with(&real_root) && !real.components().any(|c| c == Component::ParentDir)
}

pub fn require_mapped(cx: &Ctx, path: &str, what: &str) -> Result<(), String> {
    if mapped(cx, path) {
        return Ok(());
    }
    Err(format!(
        "{what}: refused a path outside this machine's copies of the server's folders"
    ))
}
