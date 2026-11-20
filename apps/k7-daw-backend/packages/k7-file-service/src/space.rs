//! space.rs — one workspace as the file service sees it: its root, the open project, and `safePath`'s two gates.
//! What a reader gets wrong: a path outside `_libraries/` asks the disk nothing but the gate's own `lstat`s;
//! the marker is read only when a library path needs the open project (1040).

use std::fmt;

use crate::confine::{Probe, confine_to};
use crate::filter::project_app;
use crate::libraries::{ACCOUNT_LIBRARY, Home, is_library_name, library_root};
use crate::paths::{basename, dirname, is_within, join, resolve, resolve_all};

/// The folder a library path starts with: virtual, held on no workspace's disk.
const LIBRARIES_DIR: &str = "_libraries";
/// Where each workspace's open-project marker lives, beside the workspaces.
const MARKERS_DIR: &str = ".active-projects";
/// The longest `rel` a native read takes: the gate climbs one `lstat` per segment (1024).
pub const MAX_REL: usize = 16 * 1024;

/// Why a native piece does not answer: the route is declined (1010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Declined {
    Busy,
    LibraryActive,
    LongPath,
    LossyText,
    NoLibraries,
    NotRegular,
    SurfacesOutside,
    TooLarge,
    TooDeep,
    TooMany,
    WouldThrow,
}

impl Declined {
    /// Every reason, for a test that holds each to a refusal (4-7).
    pub const ALL: [Declined; 11] = [
        Declined::Busy,
        Declined::LibraryActive,
        Declined::LongPath,
        Declined::LossyText,
        Declined::NoLibraries,
        Declined::NotRegular,
        Declined::SurfacesOutside,
        Declined::TooLarge,
        Declined::TooDeep,
        Declined::TooMany,
        Declined::WouldThrow,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Declined::Busy => "busy",
            Declined::LibraryActive => "library-active",
            Declined::LongPath => "long-path",
            Declined::LossyText => "lossy-text",
            Declined::NoLibraries => "no-libraries",
            Declined::NotRegular => "not-regular",
            Declined::SurfacesOutside => "surfaces-outside",
            Declined::TooLarge => "too-large",
            Declined::TooDeep => "too-deep",
            Declined::TooMany => "too-many",
            Declined::WouldThrow => "would-throw",
        }
    }
}

/// A path the gates refuse, in the sentences `safePath` throws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    Escapes { what: &'static str, rel: String },
    NoLibrary { rel: String },
    Declined(Declined),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Escapes { what, rel } => write!(f, "path escapes the {what}: {rel}"),
            PathError::NoLibrary { rel } => write!(f, "names no library: {rel}"),
            PathError::Declined(d) => write!(f, "declined: {}", d.as_str()),
        }
    }
}

/// `String.prototype.trim`: JavaScript's white space, which is not Rust's (U+FEFF in, U+0085 out).
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}

/// A folder whose name claims an app and that exists.
pub fn is_project_dir(dir: &str) -> bool {
    project_app(&basename(dir)).is_some() && std::fs::metadata(dir).is_ok_and(|m| m.is_dir())
}

/// A workspace's root and what its paths are confined by.
pub struct Space<'h> {
    home: &'h Home,
    root: String,
    root_is_project: bool,
}

impl<'h> Space<'h> {
    /// A workspace at `root`; the one `stat` is for a root named like a project.
    pub fn open(home: &'h Home, root: &str) -> Space<'h> {
        let root = resolve_all(&[root]);
        let root_is_project = is_project_dir(&root);
        Space {
            home,
            root,
            root_is_project,
        }
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn home(&self) -> &Home {
        self.home
    }

    /// The open project, the root when it is one, else the marker's (1040); a `Declined` where a guess would be wrong (1041).
    pub fn active(&self) -> Result<Option<String>, Declined> {
        if self.root_is_project {
            return Ok(Some(self.root.clone()));
        }
        let marker = join(&[&dirname(&self.root), MARKERS_DIR, &basename(&self.root)]);
        let rel = match std::fs::read(&marker) {
            Ok(bytes) => js_trim(&String::from_utf8_lossy(&bytes)).to_string(),
            Err(_) => {
                return Ok(None);
            }
        };
        if rel.is_empty() {
            return Ok(None);
        }
        if rel == LIBRARIES_DIR || rel.starts_with("_libraries/") || rel.starts_with("_libraries\\")
        {
            return Err(Declined::LibraryActive);
        }
        let abs = resolve(&self.root, &rel);
        Ok((is_within(&self.root, &abs) && is_project_dir(&abs)).then_some(abs))
    }

    /// `safePath`: `rel` resolved inside the workspace or, below `_libraries/`, that library's own root; the gate's sentence if refused.
    pub fn safe_path(&self, rel: &str, probe: &dyn Probe) -> Result<String, PathError> {
        if rel.len() > MAX_REL {
            return Err(PathError::Declined(Declined::LongPath));
        }
        let segs: Vec<&str> = rel
            .split(['\\', '/'])
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();
        if segs.first() == Some(&LIBRARIES_DIR) {
            // Each library is confined to its OWN root, wherever an overlay points it.
            if let Some(name) = segs
                .get(1)
                .filter(|s| is_library_name(s) || **s == ACCOUNT_LIBRARY)
            {
                let open = self.active().map_err(PathError::Declined)?;
                let from = open.as_deref().unwrap_or(&self.root);
                let root =
                    library_root(self.home, name, Some(from)).map_err(PathError::Declined)?;
                let mut parts: Vec<&str> = vec![&root];
                parts.extend(&segs[2..]);
                return confined(&resolve_all(&parts), &root, rel, "libraries", probe);
            }
            if segs.len() > 1 {
                return Err(PathError::NoLibrary { rel: rel.into() });
            }
            let home = &self.home.libraries;
            return confined(home, home, rel, "libraries", probe);
        }
        let abs = if rel == "." {
            self.root.clone()
        } else {
            resolve(&self.root, rel)
        };
        confined(&abs, &self.root, rel, "workspace", probe)
    }
}

fn confined(
    abs: &str,
    root: &str,
    rel: &str,
    what: &'static str,
    probe: &dyn Probe,
) -> Result<String, PathError> {
    confine_to(&[root], abs, probe).ok_or_else(|| PathError::Escapes {
        what,
        rel: rel.into(),
    })
}
