// confine.rs — the ONE check a path read from a song file passes before anything opens, copies or deletes it.
// A reader would get wrong that a `../` in a song file is another account's file, not a typo: the roots are the whole defence.

use crate::fs::Fs;
use crate::paths::{climbs_out, dirname, is_within, join, relative, resolve, same_path};
use crate::winpath::{is_win_form, user_path};

pub const AUTO_RENDERS_HOME_DIR: &str = ".renders";
const ACCOUNT_LIBRARY_DIR: &str = "library";
const LIBRARIES_HOME_DIR: &str = "libraries";

/// What `studioHome()` and the sharedFolders read from the process, given by the host instead.
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub studio_home: String,
    pub libraries_home: String,
    pub meta_root: String,
    pub models_root: String,
    /// An admin's editable libraries: name and root.
    pub overlays: Vec<(String, String)>,
    /// The libraries as the project's workspace pins them (name, folder read from); None reads each one's own folder.
    pub library_roots: Option<Vec<(String, String)>>,
}

impl Env {
    /// The defaults under `studio_home`: libraries, models, no overlay.
    #[must_use]
    pub fn at(studio_home: &str, meta_root: &str) -> Env {
        let home = resolve(studio_home);
        Env {
            libraries_home: join(&[&home, LIBRARIES_HOME_DIR]),
            models_root: join(&[&home, "models"]),
            meta_root: meta_root.to_owned(),
            studio_home: home,
            overlays: vec![],
            library_roots: None,
        }
    }
}

/// `isLibraryName`: lowercase letters, digits and dashes, 40 at most, and not a name a ref already means something by.
#[must_use]
pub fn is_library_name(name: &str) -> bool {
    let b = name.as_bytes();
    let shape = !b.is_empty()
        && b.len() <= 40
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-');
    shape && !["engine", "workspace", "account", "meta", "studio", "mediatek"].contains(&name)
}

fn real_or(fs: &dyn Fs, path: &str) -> Option<String> {
    fs.real_path(path)
}

/// `path` resolved and absolute when it lies inside one of `roots`, symlinks followed; None otherwise.
#[must_use]
pub fn confine_to(fs: &dyn Fs, roots: &[String], path: &str) -> Option<String> {
    if path.is_empty() || path.contains('\0') {
        return None;
    }
    let p = resolve(path);
    // A colon after the drive is an NTFS stream (`a.txt:x`), never a name a song file may reach.
    if is_win_form(&p) && p.split('/').skip(2).any(|s| s.contains(':')) {
        return None;
    }
    for raw in roots.iter().filter(|r| !r.is_empty()) {
        let root = resolve(raw);
        if !is_within(&root, &p) {
            continue;
        }
        let mut probe = p.clone();
        while !same_path(&probe, &root) && !fs.present(&probe) {
            probe = dirname(&probe);
        }
        // Below a root that does not exist nothing exists either, so no link can lead out of it.
        if !fs.present(&probe) {
            return Some(p);
        }
        let Some(real) = real_or(fs, &probe) else { continue };
        if is_within(&real_or(fs, &root).unwrap_or(root), &real) {
            return Some(p);
        }
        // A link inside this root may still land inside another allowed one.
        let elsewhere = |r: &String| {
            let r = resolve(r);
            is_within(&real_or(fs, &r).unwrap_or(r), &real)
        };
        if roots.iter().filter(|r| !r.is_empty()).any(elsewhere) {
            return Some(p);
        }
    }
    None
}

/// Every library folder this account reads: the account's own, each of the origin's, each overlay.
#[must_use]
pub fn library_read_roots(fs: &dyn Fs, env: &Env) -> Vec<String> {
    let mut out = vec![join(&[&env.studio_home, ACCOUNT_LIBRARY_DIR])];
    // No libraries yet is the ordinary state of a new machine.
    if let Ok(entries) = fs.list(&env.libraries_home) {
        for e in entries {
            if e.kind == song::Kind::Dir && is_library_name(&e.name) {
                out.push(join(&[&env.libraries_home, &e.name]));
            }
        }
    }
    out.extend(env.overlays.iter().map(|(_, root)| root.clone()));
    out
}

/// The workspace a project stands in: `<home>/workspace/<name>` when it is there, else its parent folder.
#[must_use]
pub fn workspace_dir_of(env: &Env, project_dir: &str) -> String {
    let dir = resolve(project_dir);
    let home = join(&[&env.studio_home, "workspace"]);
    let rel = relative(&home, &dir);
    if !rel.is_empty() && !climbs_out(&rel) {
        let first = rel.split(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("");
        return join(&[&home, first]);
    }
    dirname(&dir)
}

/// What a path a project's files name may reach.
#[must_use]
pub fn project_roots(fs: &dyn Fs, env: &Env, project_dir: &str) -> Vec<String> {
    let mut roots = vec![resolve(project_dir), workspace_dir_of(env, project_dir)];
    roots.extend(library_read_roots(fs, env));
    roots.push(env.meta_root.clone());
    roots.push(join(&[&env.studio_home, AUTO_RENDERS_HOME_DIR]));
    roots.push(env.models_root.clone());
    roots
}

/// `reference`, relative to `base` (the project by default), confined to the project's roots; None when it escapes.
#[must_use]
pub fn confine_to_project(
    fs: &dyn Fs,
    env: &Env,
    project_dir: &str,
    reference: &str,
    base: Option<&str>,
) -> Option<String> {
    if reference.is_empty() {
        return None;
    }
    let reference = user_path(reference)?;
    // An absolute reference replaces the base, as `path.resolve` does.
    let at = if reference.starts_with('/') {
        resolve(&reference)
    } else {
        resolve(&format!("{}/{reference}", base.unwrap_or(project_dir)))
    };
    confine_to(fs, &project_roots(fs, env, project_dir), &at)
}
