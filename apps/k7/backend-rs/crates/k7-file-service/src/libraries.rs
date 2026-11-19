//! libraries.rs — where a library is read from: the account's own, or a pinned version.
//! `sharedFolders.ts`' `libraryRoot` and `libraryOf`, with the names and versions of `moduleTypes.ts` and `libraryVersions.ts`.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use crate::paths::{climbs_out, dirname, join, relative, resolve_all};
use crate::space::Declined;

/// The file a workspace keeps its pins in, at its root.
pub const PINS_FILE: &str = "_library-pins.k7x";
/// The pins' YAML spelling, which is declined.
pub const LEGACY_PINS_FILE: &str = "_library-pins.yaml";
/// The most a pins file is read at; a larger one is declined rather than held.
const MAX_PINS_BYTES: u64 = 16 << 20;
/// The account's own library, `@account/...`.
pub const ACCOUNT_LIBRARY: &str = "account";
/// What a library may be called, as `moduleTypes.ts` writes it; a test holds the two equal (1030).
pub const NAME_PATTERN: &str = "^[a-z0-9][a-z0-9-]{0,39}$";
/// A published version's folder name, as `libraryVersions.ts` writes it.
pub const VERSION_PATTERN: &str = "^[1-9][0-9]{0,5}$";
/// Names a library cannot take, because a ref spelt with them already means something else.
pub const RESERVED: [&str; 6] = [
    "engine",
    "workspace",
    ACCOUNT_LIBRARY,
    "meta",
    "studio",
    "mediatek",
];

/// Where the account's state and libraries are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
    pub studio: String,
    pub libraries: String,
}

impl Home {
    /// `studioHome()` and `librariesHome()` of the account's environment (1025).
    pub fn of(env: &BTreeMap<String, String>) -> Home {
        let get = |k: &str| env.get(k).map(String::as_str).filter(|v| !v.is_empty());
        let user = get("HOME").or_else(|| get("USERPROFILE")).unwrap_or("/");
        let default = join(&[user, ".k7"]);
        let studio = resolve_all(&[get("STUDIO_HOME").unwrap_or(&default)]);
        let libraries = match get("STUDIO_LIBRARIES_HOME") {
            Some(shared) => resolve_all(&[shared]),
            None => join(&[&studio, "libraries"]),
        };
        Home { studio, libraries }
    }

    /// The account's own library, beside its workspaces.
    pub fn account_home(&self) -> String {
        join(&[&self.studio, "library"])
    }

    /// A library's folder: its versions and its draft, or the whole library while it is one folder.
    pub fn library_home(&self, name: &str) -> String {
        join(&[&self.libraries, name])
    }
}

/// A version a workspace may pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Number(u32),
    Draft,
}

impl Version {
    fn spelling(self) -> String {
        match self {
            Version::Number(n) => n.to_string(),
            Version::Draft => "draft".into(),
        }
    }
}

/// A library and the folder it is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    pub name: String,
    pub root: String,
}

/// `isLibraryName`: a ref segment and a folder name at once, lowercase, and not a reserved word.
pub fn is_library_name(name: &str) -> bool {
    let b = name.as_bytes();
    let ok = !b.is_empty()
        && b.len() <= 40
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-');
    ok && !RESERVED.contains(&name)
}

/// `versionOfDir`: a folder name as the version it holds; `draft`, or a positive integer without leading zeros.
pub fn version_of_dir(name: &str) -> Option<Version> {
    if name == "draft" {
        return Some(Version::Draft);
    }
    let b = name.as_bytes();
    let ok = (1..=6).contains(&b.len())
        && (b'1'..=b'9').contains(&b[0])
        && b.iter().all(u8::is_ascii_digit);
    ok.then(|| Version::Number(name.parse().unwrap_or(u32::MAX)))
}

/// Pins from the text of a `_library-pins.k7x`: an entry that is no version is dropped, so it reads as unpinned.
fn pins_of(text: &str) -> Result<HashMap<String, Version>, Declined> {
    let bump = k7x::Bump::new();
    let Ok(parsed) = k7x::parse_tree_lossless(text, &bump) else {
        return Ok(HashMap::new());
    };
    let Some(tree) = parsed else {
        return Err(Declined::LossyText);
    };
    let mut out = HashMap::new();
    for e in tree.entries {
        let k7x::Node::Scalar { value, .. } = e.value else {
            continue;
        };
        match value {
            k7x::Scalar::Str("draft") => {
                out.insert(e.key.to_string(), Version::Draft);
            }
            k7x::Scalar::Num(n) if n.is_finite() && n.fract() == 0.0 && n >= 1.0 => {
                out.insert(
                    e.key.to_string(),
                    Version::Number(n.min(f64::from(u32::MAX)) as u32),
                );
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The pins of the workspace `from` stands in: the nearest `_library-pins.k7x` upward (1031); no pins when none.
pub fn workspace_pins(from: &str) -> Result<HashMap<String, Version>, Declined> {
    let mut dir = resolve_all(&[from]);
    loop {
        let file = join(&[&dir, PINS_FILE]);
        if std::fs::metadata(&file).is_ok() {
            let Ok(f) = std::fs::File::open(&file) else {
                return Ok(HashMap::new());
            };
            let mut bytes = Vec::new();
            if f.take(MAX_PINS_BYTES + 1).read_to_end(&mut bytes).is_err() {
                return Ok(HashMap::new());
            }
            if bytes.len() as u64 > MAX_PINS_BYTES {
                return Err(Declined::TooLarge);
            }
            return pins_of(&String::from_utf8_lossy(&bytes));
        }
        if std::fs::metadata(join(&[&dir, LEGACY_PINS_FILE])).is_ok() {
            return Err(Declined::LegacyPins);
        }
        let up = dirname(&dir);
        if up == dir {
            return Ok(HashMap::new());
        }
        dir = up;
    }
}

/// Whether library `name` is laid out as versions: a folder named as one is in it, found by one listing.
fn is_versioned(home: &Home, name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(home.library_home(name)) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_type().is_ok_and(|t| t.is_dir())
            && version_of_dir(&e.file_name().to_string_lossy()).is_some()
    })
}

/// The folder version `v` of `name` is read from; a library still in one folder is every version of itself.
fn version_root(home: &Home, name: &str, v: Version) -> String {
    let library = home.library_home(name);
    if is_versioned(home, name) {
        join(&[&library, &v.spelling()])
    } else {
        library
    }
}

thread_local! {
    static LIBRARY_OF_CALLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many times `library_of` has run on this thread: a test holds a level's paths to one lookup a row (1422).
pub fn library_of_calls() -> u64 {
    LIBRARY_OF_CALLS.with(std::cell::Cell::get)
}

/// `libraryOf`: the library `path` lies in and the version folder holding it (1035: no search over libraries).
pub fn library_of(home: &Home, path: &str) -> Option<Library> {
    LIBRARY_OF_CALLS.with(|n| n.set(n.get() + 1));
    let abs = resolve_all(&[path]);
    let account = home.account_home();
    if !climbs_out(&relative(&account, &abs)) {
        return Some(Library {
            name: ACCOUNT_LIBRARY.into(),
            root: account,
        });
    }
    let rel = relative(&home.libraries, &abs);
    if rel.is_empty() || climbs_out(&rel) {
        return None;
    }
    let mut segs = rel.split(['\\', '/']).filter(|s| !s.is_empty());
    let name = segs.next().unwrap_or("");
    if !is_library_name(name) {
        return None;
    }
    let library = home.library_home(name);
    if is_versioned(home, name) {
        let root = match segs.next().and_then(version_of_dir) {
            Some(v) => join(&[&library, &v.spelling()]),
            None => library,
        };
        return Some(Library {
            name: name.into(),
            root,
        });
    }
    Some(Library {
        name: name.into(),
        root: library,
    })
}

/// `libraryRoot` for the workspace `from` stands in (1033); a `Declined` where its pins are in the YAML spelling.
pub fn library_root(home: &Home, name: &str, from: Option<&str>) -> Result<String, Declined> {
    if name == ACCOUNT_LIBRARY {
        return Ok(home.account_home());
    }
    let from = from.filter(|f| !f.is_empty());
    // Standing inside a version of this library, that version.
    if let Some(own) = from.and_then(|f| library_of(home, f))
        && own.name == name
        && own.root != home.library_home(name)
    {
        return Ok(own.root);
    }
    let pins = match from {
        Some(f) => workspace_pins(f)?,
        None => HashMap::new(),
    };
    if let Some(v) = pins.get(name) {
        return Ok(version_root(home, name, *v));
    }
    Ok(version_root(home, name, Version::Number(1)))
}
