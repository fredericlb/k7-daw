//! versions.rs — `libraryVersionsAt`: each library's version as the workspace plays it, what that version needs, and
//! the newest this k7 could move to, here or at the origin (1430). The hub's answers arrive as a `Remote`.
//! What a reader gets wrong: a manifest that parses badly is none, but one that cannot be read, beside a YAML one, is declined.

use std::collections::HashMap;

use super::families::library_names;
use crate::libraries::{Home, Version, version_of_dir, workspace_pins};
use crate::paths::join;
use crate::read::open_regular;
use crate::space::Declined;
use crate::tree::Limits;

/// What this k7 can run: a version whose `requires` is above it is not offered. A test holds it equal to `libraryVersions.ts`.
pub const LIBRARY_LEVEL: u32 = 2;
/// The version a library the pins do not name resolves to.
pub const UNPINNED_VERSION: u32 = 1;
/// A published version's manifest, and the YAML spelling, which is declined.
const MANIFEST: &str = "library.k7x";
const LEGACY_MANIFEST: &str = "library.yaml";

/// The versions the origin publishes of each library, as `(version, requires)`, asked by the front (1431).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Remote {
    pub by_name: HashMap<String, Vec<(u32, u32)>>,
}

/// One library's row: the version played, what it needs of the engine's machine, the newest it could move to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionRow {
    pub pinned: Version,
    pub requires: Option<u32>,
    pub upgrade: Option<u32>,
}

/// `libraryManifestData`'s `requires` of a `library.k7x` text: `None` when it is no manifest.
fn requires_of(text: &str) -> Result<Option<u32>, Declined> {
    let bump = k7x::Bump::new();
    let tree = match k7x::parse_tree_lossless(text, &bump) {
        Ok(Some(t)) => t,
        Ok(None) => return Err(Declined::LossyText),
        Err(_) => return Ok(None),
    };
    let int = |key: &str| {
        tree.entries
            .iter()
            .find(|e| e.key == key)
            .and_then(|e| match e.value {
                k7x::Node::Scalar {
                    value: k7x::Scalar::Num(n),
                    ..
                } if n.is_finite() && n.fract() == 0.0 && n >= 1.0 => {
                    Some(n.min(f64::from(u32::MAX)) as u32)
                }
                _ => None,
            })
    };
    let named = tree.entries.iter().any(|e| {
        e.key == "name"
            && matches!(
                e.value,
                k7x::Node::Scalar {
                    value: k7x::Scalar::Str(_),
                    ..
                }
            )
    });
    Ok((named && int("version").is_some()).then(|| int("requires").unwrap_or(1)))
}

/// `libraryManifest(name, v)?.requires`: `None` for a version with no manifest.
fn manifest_requires(lib: &Local, v: u32, limits: &Limits) -> Result<Option<u32>, Declined> {
    use std::io::Read;
    let root = lib.root_of(v);
    let read = open_regular(&join(&[&root, MANIFEST])).ok().and_then(|f| {
        let mut bytes = Vec::new();
        f.take(limits.max_text.saturating_add(1))
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    });
    let Some(bytes) = read else {
        // No native reader takes YAML: a manifest in that spelling is declined.
        if std::fs::metadata(join(&[&root, LEGACY_MANIFEST])).is_ok() {
            return Err(Declined::LegacyManifest);
        }
        return Ok(None);
    };
    if bytes.len() as u64 > limits.max_text {
        return Err(Declined::TooLarge);
    }
    requires_of(&String::from_utf8_lossy(&bytes))
}

/// One library as this machine holds it: its folder, whether it is laid out as versions, its published versions.
struct Local {
    library: String,
    versioned: bool,
    versions: Vec<u32>,
}

impl Local {
    /// `libraryVersionRoot`, from the one listing of the library's folder rather than one per version.
    fn root_of(&self, v: u32) -> String {
        if self.versioned {
            join(&[&self.library, &v.to_string()])
        } else {
            self.library.clone()
        }
    }
}

/// `libraryVersions`: the published versions of `name` here, oldest first; one still in a single folder is `[1]`.
fn local_versions(home: &Home, name: &str, limits: &Limits) -> Result<Local, Declined> {
    let library = home.library_home(name);
    let mut local = Local {
        library: library.clone(),
        versioned: false,
        versions: Vec::new(),
    };
    if std::fs::metadata(&library).is_err() {
        return Ok(local);
    }
    let mut dirs = Vec::new();
    if let Ok(read) = std::fs::read_dir(&library) {
        for (n, e) in read.flatten().enumerate() {
            if n as u64 >= limits.max_seen {
                return Err(Declined::TooMany);
            }
            if e.file_type().is_ok_and(|t| t.is_dir())
                && let Some(v) = version_of_dir(&e.file_name().to_string_lossy())
            {
                dirs.push(v);
            }
        }
    }
    if dirs.is_empty() {
        local.versions = vec![UNPINNED_VERSION];
        return Ok(local);
    }
    local.versioned = true;
    let mut numbers: Vec<u32> = dirs
        .into_iter()
        .filter_map(|v| match v {
            Version::Number(n) => Some(n),
            Version::Draft => None,
        })
        .collect();
    numbers.sort_unstable();
    local.versions = numbers;
    Ok(local)
}

/// `newestRunnableVersion`: the highest version a k7 at `LIBRARY_LEVEL` runs, here or among `remote`.
fn newest(lib: &Local, remote: &[(u32, u32)], limits: &Limits) -> Result<u32, Declined> {
    let mut best = None;
    for &v in &lib.versions {
        if manifest_requires(lib, v, limits)?.unwrap_or(1) <= LIBRARY_LEVEL {
            best = best.max(Some(v));
        }
    }
    for (v, requires) in remote {
        if *requires <= LIBRARY_LEVEL {
            best = best.max(Some(*v));
        }
    }
    Ok(best.unwrap_or(UNPINNED_VERSION))
}

/// `newestRunnableVersion(name)` among the versions on this machine: what a workspace pinned afresh plays.
pub fn newest_here(home: &Home, name: &str, limits: &Limits) -> Result<u32, Declined> {
    newest(&local_versions(home, name, limits)?, &[], limits)
}

/// Each library's row for the workspace `dir` stands in; `LegacyPins` and `LegacyManifest` are declined.
pub fn versions_at(
    home: &Home,
    dir: &str,
    remote: &Remote,
    limits: &Limits,
) -> Result<HashMap<String, VersionRow>, Declined> {
    let pins = workspace_pins(dir)?;
    let mut out = HashMap::new();
    for name in library_names(home) {
        let pinned = pins
            .get(&name)
            .copied()
            .unwrap_or(Version::Number(UNPINNED_VERSION));
        let mut row = VersionRow {
            pinned,
            requires: None,
            upgrade: None,
        };
        if let Version::Number(p) = pinned {
            let none = Vec::new();
            let lib = local_versions(home, &name, limits)?;
            let newest = newest(&lib, remote.by_name.get(&name).unwrap_or(&none), limits)?;
            row.requires = manifest_requires(&lib, p, limits)?;
            row.upgrade = (newest > p).then_some(newest);
        }
        out.insert(name, row);
    }
    Ok(out)
}
