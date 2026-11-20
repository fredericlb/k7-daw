//! confine.rs — the second gate, `confineTo` of song's `confine.ts`: a path inside a root lexically AND through the
//! links on disk. The disk is a `Probe`, so a test spells Windows roots and links on any host (1022).
//! What a reader gets wrong: each call looks at the disk afresh; nothing is remembered across requests.

use crate::libraries::Home;
use crate::paths::{dirname, is_within, join, resolve_all};
use crate::song_ask::{library_names, workspace_dir_of};

/// What the gate asks of the disk.
pub trait Probe {
    /// Whether anything is at `p`, a dangling link included (`lstat`).
    fn present(&self, p: &str) -> bool;
    /// The real path of an existing path, `/`-spelled; `None` when it cannot be read.
    fn real(&self, p: &str) -> Option<String>;
}

/// The real disk.
pub struct Disk;

impl Probe for Disk {
    fn present(&self, p: &str) -> bool {
        std::fs::symlink_metadata(p).is_ok()
    }

    fn real(&self, p: &str) -> Option<String> {
        spelled(&std::fs::canonicalize(p).ok()?)
    }
}

/// A canonical path in the `/` spelling every gate compares by; `None` for what the path library cannot hold.
/// Only `X:\` and `\\?\X:\` (and a posix path) pass: a UNC root would collapse to `/srv` in `join`, a device path has no root at all.
pub fn spelled(real: &std::path::Path) -> Option<String> {
    let s = real.to_string_lossy();
    let s = s
        .strip_prefix(r"\\?\")
        .filter(|r| drive_form(r))
        .unwrap_or(&s);
    let slashed = s.replace('\\', "/");
    (slashed.starts_with('/') && !slashed.starts_with("//") || drive_form(&slashed))
        .then_some(slashed)
}

fn drive_form(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// `path` resolved when it lies inside one of `roots`, links followed; `None` otherwise.
pub fn confine_to(roots: &[&str], path: &str, probe: &dyn Probe) -> Option<String> {
    if path.is_empty() || path.contains('\0') {
        return None;
    }
    let p = resolve_all(&[path]);
    for raw in roots {
        if raw.is_empty() {
            continue;
        }
        let root = resolve_all(&[raw]);
        if !is_within(&root, &p) {
            continue;
        }
        let mut at = p.clone();
        while at != root && !probe.present(&at) {
            let up = dirname(&at);
            // A path whose parent is itself and is absent has no ancestor on disk.
            if up == at {
                break;
            }
            at = up;
        }
        // Below a root that does not exist nothing exists either, so no link can lead out of it.
        if !probe.present(&at) {
            return Some(p);
        }
        let Some(real) = probe.real(&at).filter(|r| !r.is_empty()) else {
            continue;
        };
        if is_within(&probe.real(&root).unwrap_or_else(|| root.clone()), &real) {
            return Some(p);
        }
        // A link inside this root may still land inside another allowed one.
        let elsewhere = roots.iter().filter(|r| !r.is_empty()).any(|r| {
            let r = resolve_all(&[r]);
            is_within(&probe.real(&r).unwrap_or(r), &real)
        });
        if elsewhere {
            return Some(p);
        }
    }
    None
}

/// `projectRoots` (song's confine.ts:98): the project, its workspace, then `libraryReadRoots` (confine.ts:70) — the account's, each library's, each overlay.
pub fn project_roots(home: &Home, project: &str, meta_modules: &str, models: &str) -> Vec<String> {
    let project = resolve_all(&[project]);
    let names = library_names(home);
    let mut roots = vec![
        project.clone(),
        workspace_dir_of(home, &project),
        home.account_home(),
    ];
    roots.extend(names.iter().map(|n| home.library_home(n)));
    roots.extend([
        meta_modules.to_owned(),
        join(&[&home.studio, ".renders"]),
        models.to_owned(),
    ]);
    roots
}

#[cfg(test)]
mod spelled_tests {
    use super::spelled;
    use crate::paths::join;
    use std::path::Path;

    #[test]
    fn a_verbatim_drive_path_is_spelled_the_way_the_gates_compare() {
        assert_eq!(spelled(Path::new(r"\\?\C:\a\b")).as_deref(), Some("C:/a/b"));
        assert_eq!(spelled(Path::new(r"C:\a")).as_deref(), Some("C:/a"));
        assert_eq!(spelled(Path::new("/tmp/x")).as_deref(), Some("/tmp/x"));
    }

    // A UNC or device path has no root the path library holds: it is refused, never collapsed to `/srv`.
    #[test]
    fn unc_and_device_paths_are_refused_not_collapsed() {
        for p in [
            r"\\?\UNC\srv\share\p",
            r"\\srv\share\p",
            r"\\.\COM1",
            r"\\?\Volume{1}\x",
            r"\\?\GLOBALROOT\x",
        ] {
            assert_eq!(spelled(Path::new(p)), None, "{p}");
        }
        // And what is kept survives a join with its root.
        let kept = spelled(Path::new(r"\\?\C:\ws")).unwrap();
        assert_eq!(join(&[&kept, "x"]), "C:/ws/x");
    }
}
