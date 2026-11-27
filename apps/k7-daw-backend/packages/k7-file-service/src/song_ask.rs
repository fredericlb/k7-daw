//! song_ask.rs — `songRoots` of song's `songRoots.ts`, field for field: every folder a compile may read, in the order it keys on.
//! What a reader gets wrong: the roots hash into the session's key, so an order or a spelling that differs is a second compile (1117, 1514).

use std::collections::BTreeMap;

use song::roots::Library as RootLibrary;
use song::{PathRule, Roots};

use crate::confine::Probe;
use crate::libraries::{ACCOUNT_LIBRARY, Home, is_library_name, library_of, library_root};
use crate::paths::{climbs_out, dirname, join, relative, resolve_all, resolve_in};
use crate::space::Declined;

/// The folders of a library repository the compile reads, each possibly a link out of the library (737).
const FAMILIES: [&str; 8] = [
    "modules",
    "chains",
    "assemblies",
    "console",
    "samples",
    "irs",
    "wavetables",
    "models",
];

/// What the roots are made from: the account's home, the project, the code root and the account's environment.
pub struct Input<'a> {
    pub home: &'a Home,
    pub project: &'a str,
    pub code_root: &'a str,
    pub env: &'a BTreeMap<String, String>,
}

/// The libraries on disk by name, bytes order (1513): directories only, as `readdir`'s `isDirectory` keeps them.
pub(crate) fn library_names(home: &Home) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(&home.libraries) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_library_name(n))
        .collect();
    names.sort();
    names
}

/// A value of the environment as JavaScript's truthiness reads it.
fn env_of<'a>(env: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    env.get(key).map(String::as_str).filter(|v| !v.is_empty())
}

/// `workspaceDirOf`: `<studio>/workspace/<name>` when the project is in it, else its parent folder.
pub(crate) fn workspace_dir_of(home: &Home, project: &str) -> String {
    let workspaces = join(&[&home.studio, "workspace"]);
    let rel = relative(&workspaces, project);
    if rel.is_empty() || climbs_out(&rel) {
        return dirname(project);
    }
    join(&[&workspaces, rel.split(['\\', '/']).next().unwrap_or("")])
}

/// `songRoots(project)`; a `Declined` where the roots depend on a file no native reader takes.
pub fn song_roots(input: &Input<'_>, probe: &dyn Probe) -> Result<Roots, Declined> {
    let home = input.home;
    let code = resolve_all(&[input.code_root]);
    let project = resolve_in(&code, &[input.project]);
    let names = library_names(home);
    let mut libraries = vec![RootLibrary {
        name: ACCOUNT_LIBRARY.into(),
        root: home.account_home(),
        editable: true,
    }];
    for name in &names {
        let root = library_root(home, name, Some(&project))?;
        libraries.push(RootLibrary {
            name: name.clone(),
            root,
            editable: false,
        });
    }
    // `libraryReadRoots()`: the account's, each library's own folder, each overlay's; then a family folder that is a link.
    let mut also: Vec<String> = Vec::new();
    let mut add = |p: String| {
        if !also.contains(&p) {
            also.push(p);
        }
    };
    add(home.account_home());
    for name in &names {
        add(home.library_home(name));
    }
    for lib in &libraries {
        for family in FAMILIES {
            let at = join(&[&lib.root, family]);
            if std::fs::symlink_metadata(&at).is_ok_and(|m| m.is_symlink())
                && let Some(real) = probe.real(&at)
            {
                add(real);
            }
        }
    }
    let meta = join(&[&code, "apps", "k7-daw-wasm", "song", "meta"]);
    let shared = resolve_all(&[&project, "../_modules"]);
    let namespace = if shared == meta {
        "meta".to_owned()
    } else {
        library_of(home, &shared).map_or_else(|| "workspace".to_owned(), |l| l.name)
    };
    let models = match env_of(input.env, "STUDIO_MODELS") {
        Some(m) => resolve_in(&code, &[m]),
        None => join(&[&home.studio, "models"]),
    };
    Ok(Roots {
        workspace: Some(dirname(&project)),
        workspace_namespace: (namespace != "workspace").then_some(namespace),
        confine: Some(workspace_dir_of(home, &project)),
        libraries,
        meta: Some(meta),
        scan: Some(join(&[&home.studio, "exts.json"])),
        renders: Some(join(&[&home.studio, ".renders"])),
        models: Some(models),
        // The project's own `samples/` is first and the crate adds it itself, so only the machine's banks are named.
        sample_banks: ["DIRT_SAMPLES", "STRUDEL_SAMPLES"]
            .iter()
            .filter_map(|k| env_of(input.env, k).map(str::to_owned))
            .collect(),
        also_readable: also,
        paths: PathRule::HOST,
        project,
    })
}
