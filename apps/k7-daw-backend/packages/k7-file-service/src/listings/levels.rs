//! levels.rs — `/module-libraries`: the module levels a project reaches, nearest first, each with the tree of its
//! folder, a library's sound families, and the version it plays (1420 to 1423). `workspaceModules.ts`' `moduleLibrariesOf`.
//! What a reader gets wrong: a level's paths are spelt by one `to_rel` of its folder, never one per entry (1422).

use std::collections::HashMap;

use super::engine::engine_row;
use super::families::{account_libraries, family_roots, from_of};
use super::versions::{Remote, VersionRow, versions_at};
use super::{Listed, Stamp, quoted};
use crate::libraries::{Home, Library, Version, library_of};
use crate::paths::{join, resolve};
use crate::rel::to_rel;
use crate::space::{Declined, Space};
use crate::tree::{Limits, MAX_UNLISTED, tree_at};

/// The shared modules folder beside a project, and the sound families a library repository carries.
const SHARED_MODULES_PATH: &str = "../_modules";
const SOUND_FAMILIES: [&str; 4] = ["samples", "irs", "wavetables", "models"];

/// `moduleRootsOf`: the folders a level is listed at, with the namespace each answers to, nearest first (1421).
pub fn module_roots(home: &Home, dir: &str, libraries: &[Library]) -> Vec<(String, String)> {
    let mut abs: Vec<String> = Vec::new();
    let shared = resolve(dir, SHARED_MODULES_PATH);
    if std::fs::metadata(&shared).is_ok() {
        abs.push(shared);
    }
    for l in family_roots(home, libraries, "modules") {
        if !abs.contains(&l.root) {
            abs.push(l.root);
        }
    }
    let mut roots: Vec<(String, String)> = abs
        .into_iter()
        .map(|a| {
            (
                library_of(home, &a).map_or_else(|| "workspace".to_string(), |l| l.name),
                a,
            )
        })
        .collect();
    // Every library is a row, one with no `modules/` yet included: its chains are listed under it.
    for lib in libraries {
        if !roots.iter().any(|(ns, _)| *ns == lib.name) {
            roots.push((lib.name.clone(), join(&[&lib.root, "modules"])));
        }
    }
    roots
}

/// What the rows have used of the listing's limits, and what they met.
struct Used {
    seen: u64,
    bytes: usize,
    newest: u64,
    depth: u32,
    unlisted: Vec<String>,
}

/// The tree of `at` as `nodes` is written, spelt by `at`'s own `to_rel`; `[]` where `at` is not there.
fn nodes_of(
    space: &Space<'_>,
    at: &str,
    path: &str,
    used: &mut Used,
    limits: &Limits,
) -> Result<String, Declined> {
    if std::fs::metadata(at).is_err() {
        return Ok("[]".into());
    }
    let left = Limits {
        max_bytes: limits.max_bytes.saturating_sub(used.bytes),
        ..*limits
    };
    let t = tree_at(space, at, path, at, &left, &mut used.seen)?;
    used.bytes += t.json.len();
    used.newest = used.newest.max(t.newest_folder_ns);
    used.depth = used.depth.max(t.depth);
    for p in t.unlisted {
        if used.unlisted.len() < MAX_UNLISTED && !used.unlisted.contains(&p) {
            used.unlisted.push(p);
        }
    }
    Ok(t.json)
}

fn version_json(out: &mut Vec<u8>, row: &VersionRow) {
    out.extend_from_slice(b",\"version\":{\"pinned\":");
    match row.pinned {
        Version::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
        Version::Draft => out.extend_from_slice(b"\"draft\""),
    }
    if let Some(r) = row.requires {
        out.extend_from_slice(format!(",\"requires\":{r}").as_bytes());
    }
    if let Some(u) = row.upgrade {
        out.extend_from_slice(format!(",\"upgrade\":{u}").as_bytes());
    }
    if let Some(u) = row.used {
        out.extend_from_slice(format!(",\"used\":{u}").as_bytes());
    }
    out.push(b'}');
}

/// `moduleLibrariesOf` with each row's version, as `/module-libraries` answers.
pub fn module_libraries(
    space: &Space<'_>,
    remote: &Remote,
    limits: &Limits,
) -> Result<Listed, Declined> {
    let from = from_of(space)?;
    let home = space.home();
    let libraries = account_libraries(home, Some(&from))?;
    let versions: HashMap<String, VersionRow> = versions_at(home, &from, remote, limits)?;
    let mut used = Used {
        seen: 0,
        bytes: 0,
        newest: 0,
        depth: 0,
        unlisted: Vec::new(),
    };
    let mut out = b"{\"libraries\":[".to_vec();
    let mut rows = Vec::new();
    for (ns, abs) in module_roots(home, &from, &libraries) {
        if !rows.is_empty() {
            out.push(b',');
        }
        let path = to_rel(space, &abs);
        let exists = std::fs::metadata(&abs).is_ok();
        out.extend_from_slice(b"{\"ns\":");
        quoted(&mut out, &ns);
        out.extend_from_slice(b",\"path\":");
        quoted(&mut out, &path);
        out.extend_from_slice(format!(",\"exists\":{exists},\"nodes\":").as_bytes());
        out.extend_from_slice(nodes_of(space, &abs, &path, &mut used, limits)?.as_bytes());
        // A library repository's sound folders too, walked like its modules.
        if let Some(lib) = libraries.iter().find(|l| l.name == ns) {
            for family in SOUND_FAMILIES {
                let at = join(&[&lib.root, family]);
                out.extend_from_slice(format!(",\"{family}\":").as_bytes());
                let spelt = to_rel(space, &at);
                out.extend_from_slice(nodes_of(space, &at, &spelt, &mut used, limits)?.as_bytes());
            }
        }
        if let Some(v) = versions.get(&ns) {
            version_json(&mut out, v);
        }
        out.push(b'}');
        if out.len() > limits.max_bytes {
            return Err(Declined::TooLarge);
        }
        rows.push(Stamp {
            file: None,
            folder: abs,
        });
    }
    if let Some(engine) = engine_row(home)? {
        if !rows.is_empty() {
            out.push(b',');
        }
        out.extend_from_slice(engine.as_bytes());
        rows.push(Stamp {
            file: None,
            folder: home.studio.clone(),
        });
    }
    out.extend_from_slice(b"]}");
    if out.len() > limits.max_bytes {
        return Err(Declined::TooLarge);
    }
    Ok(Listed {
        json: String::from_utf8(out).map_err(|_| Declined::LossyText)?,
        rows,
        newest_folder_ns: used.newest,
        depth: used.depth + 2,
        unlisted: used.unlisted,
    })
}
