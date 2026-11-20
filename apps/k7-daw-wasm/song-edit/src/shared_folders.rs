// shared_folders.rs — the shared module folders a project sees, found by WHERE they are: `_modules/` beside it, then each library at its pin.
// A reader would get wrong that an old `_library-pins.yaml` is not read: its upgrade to k7x is the native server's, so no pins answer here.

use crate::confine::{Env, is_library_name};
use crate::fs::Fs;
use crate::paths::{climbs_out, dirname, join, relative, resolve};
use k7x::edit::{Value, parse};
use song::Kind;

pub const SHARED_MODULES_PATH: &str = "../_modules";
pub const SHARED_MODULES_DIR: &str = "_modules";
pub const ACCOUNT_LIBRARY: &str = "account";
pub const LIBRARY_PINS_FILE: &str = "_library-pins.k7x";
pub const UNPINNED_VERSION: &str = "1";
const ACCOUNT_LIBRARY_DIR: &str = "library";

/// A library by name and the folder it is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountLibrary {
    pub name: String,
    pub root: String,
}

/// The families a library repository carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Modules,
    Chains,
    Assemblies,
    Samples,
    Irs,
    Wavetables,
    Models,
}

impl Family {
    #[must_use]
    pub fn dir(self) -> &'static str {
        match self {
            Family::Modules => "modules",
            Family::Chains => "chains",
            Family::Assemblies => "assemblies",
            Family::Samples => "samples",
            Family::Irs => "irs",
            Family::Wavetables => "wavetables",
            Family::Models => "models",
        }
    }
}

/// A version folder's name as a version: `draft`, or a positive integer without leading zeros, six digits at most.
#[must_use]
pub fn version_of_dir(name: &str) -> Option<&str> {
    let digits =
        !name.is_empty() && name.len() <= 6 && !name.starts_with('0') && name.bytes().all(|b| b.is_ascii_digit());
    (name == "draft" || digits).then_some(name)
}

#[must_use]
pub fn account_library_home(env: &Env) -> String {
    join(&[&env.studio_home, ACCOUNT_LIBRARY_DIR])
}

#[must_use]
pub fn library_home_of(env: &Env, name: &str) -> String {
    join(&[&env.libraries_home, name])
}

fn version_dirs(fs: &dyn Fs, env: &Env, name: &str) -> Vec<String> {
    fs.list(&library_home_of(env, name))
        .map(|l| {
            l.into_iter()
                .filter(|e| e.kind == Kind::Dir)
                .filter_map(|e| version_of_dir(&e.name).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Is library `name` laid out as versions? One still in a single folder is its own version 1.
#[must_use]
pub fn is_versioned_library(fs: &dyn Fs, env: &Env, name: &str) -> bool {
    !version_dirs(fs, env, name).is_empty()
}

/// The folder version `v` of library `name` is read from.
#[must_use]
pub fn library_version_root(fs: &dyn Fs, env: &Env, name: &str, v: &str) -> String {
    let home = library_home_of(env, name);
    if is_versioned_library(fs, env, name) {
        join(&[&home, v])
    } else {
        home
    }
}

fn overlay_root(env: &Env, name: &str) -> Option<String> {
    env.overlays.iter().find(|(n, _)| n == name).map(|(_, r)| resolve(r))
}

fn pins_of(text: &str) -> Vec<(String, String)> {
    let Ok(Value::Map(m)) = parse(text) else { return vec![] };
    m.into_iter()
        .filter_map(|(name, v)| match v {
            Value::Str(s) if s == "draft" => Some((name, s)),
            Value::Num(n) if n.fract() == 0.0 && n >= 1.0 => Some((name, k7x::edit::js_number(n))),
            _ => None,
        })
        .collect()
}

/// The pins of the workspace `from` stands in: the nearest `_library-pins.k7x` above it, else none.
#[must_use]
pub fn workspace_pins(fs: &dyn Fs, from: &str) -> Vec<(String, String)> {
    let mut dir = resolve(from);
    loop {
        let file = join(&[&dir, LIBRARY_PINS_FILE]);
        if fs.exists(&file) {
            return fs.read_text(&file).map(|t| pins_of(&t)).unwrap_or_default();
        }
        let up = dirname(&dir);
        if up == dir {
            return vec![];
        }
        dir = up;
    }
}

/// The library `path` lies in, and the version folder holding it, or None.
#[must_use]
pub fn library_of(fs: &dyn Fs, env: &Env, path: &str) -> Option<AccountLibrary> {
    let abs = resolve(path);
    let account = account_library_home(env);
    if !climbs_out(&relative(&account, &abs)) {
        return Some(AccountLibrary {
            name: ACCOUNT_LIBRARY.to_owned(),
            root: account,
        });
    }
    for (name, _) in &env.overlays {
        let root = overlay_root(env, name)?;
        if !climbs_out(&relative(&root, &abs)) {
            return Some(AccountLibrary {
                name: name.clone(),
                root,
            });
        }
    }
    let rel = relative(&env.libraries_home, &abs);
    if rel.is_empty() || climbs_out(&rel) {
        return None;
    }
    let mut segs = rel.split(['/', '\\']).filter(|s| !s.is_empty());
    let name = segs.next()?;
    if !is_library_name(name) {
        return None;
    }
    let home = library_home_of(env, name);
    if is_versioned_library(fs, env, name) {
        let v = segs.next().and_then(version_of_dir);
        return Some(AccountLibrary {
            name: name.to_owned(),
            root: v.map_or_else(|| home.clone(), |v| join(&[&home, v])),
        });
    }
    if overlay_root(env, name).is_some() {
        return None;
    }
    Some(AccountLibrary {
        name: name.to_owned(),
        root: home,
    })
}

/// Where library `name` is read from for the workspace `from` stands in: its pin, else an overlay, else version 1.
#[must_use]
pub fn library_root(fs: &dyn Fs, env: &Env, name: &str, from: Option<&str>) -> String {
    if name == ACCOUNT_LIBRARY {
        return account_library_home(env);
    }
    if let Some(own) = from.and_then(|f| library_of(fs, env, f))
        && own.name == name
        && own.root != library_home_of(env, name)
    {
        return own.root;
    }
    let pins = from.map(|f| workspace_pins(fs, f)).unwrap_or_default();
    if let Some((_, v)) = pins.iter().find(|(n, _)| n == name) {
        return library_version_root(fs, env, name, v);
    }
    overlay_root(env, name).unwrap_or_else(|| library_version_root(fs, env, name, UNPINNED_VERSION))
}

/// Every library on this machine, by name order, each at the version `from`'s workspace pins.
#[must_use]
pub fn account_libraries(fs: &dyn Fs, env: &Env, from: Option<&str>) -> Vec<AccountLibrary> {
    let mut names: Vec<String> = fs
        .list(&env.libraries_home)
        .map(|l| {
            l.into_iter()
                .filter(|e| e.kind == Kind::Dir && is_library_name(&e.name))
                .map(|e| e.name)
                .collect()
        })
        .unwrap_or_default();
    for (n, _) in &env.overlays {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    names.sort_by_key(|n| n.encode_utf16().collect::<Vec<u16>>());
    names
        .into_iter()
        .map(|name| AccountLibrary {
            root: library_root(fs, env, &name, from),
            name,
        })
        .collect()
}

/// One family of every library, those that have it; the account's own first.
#[must_use]
pub fn library_family_roots(fs: &dyn Fs, env: &Env, kind: Family, from: Option<&str>) -> Vec<AccountLibrary> {
    let account = AccountLibrary {
        name: ACCOUNT_LIBRARY.to_owned(),
        root: account_library_home(env),
    };
    std::iter::once(account)
        .chain(account_libraries(fs, env, from))
        .map(|l| AccountLibrary {
            root: join(&[&l.root, kind.dir()]),
            name: l.name,
        })
        .filter(|l| fs.exists(&l.root))
        .collect()
}

/// One library's family folder at the version `from`'s workspace pins, or None when it carries none of it.
#[must_use]
pub fn library_family_root(fs: &dyn Fs, env: &Env, library: &str, kind: Family, from: Option<&str>) -> Option<String> {
    if !is_library_name(library) && library != ACCOUNT_LIBRARY {
        return None;
    }
    let root = join(&[&library_root(fs, env, library, from), kind.dir()]);
    fs.exists(&root).then_some(root)
}

/// Every shared module root this project inherits, nearest first.
#[must_use]
pub fn module_library_dirs(fs: &dyn Fs, env: &Env, dir: &str) -> Vec<String> {
    let mut out = vec![];
    let shared = resolve(&format!("{dir}/{SHARED_MODULES_PATH}"));
    if fs.exists(&shared) {
        out.push(shared);
    }
    for l in library_family_roots(fs, env, Family::Modules, Some(dir)) {
        if !out.contains(&l.root) {
            out.push(l.root);
        }
    }
    out
}

/// The namespace a module root answers to, decided by WHERE it is: `meta`, a library's name, else `workspace`.
#[must_use]
pub fn module_namespace_of(fs: &dyn Fs, env: &Env, root: &str) -> String {
    if resolve(root) == resolve(&env.meta_root) {
        return "meta".to_owned();
    }
    library_of(fs, env, root).map_or_else(|| "workspace".to_owned(), |l| l.name)
}
