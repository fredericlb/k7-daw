//! families.rs — the libraries' families: which libraries an account holds, a family's folder in each, and the chains
//! and assemblies they carry (1415 to 1418). `sharedFolders.ts`, `libraryChains.ts` and `assemblies.ts`, walked here.
//! What a reader gets wrong: a ref held as a file and as a folder is the file's, and the walks list in byte order.

use std::collections::HashMap;

use super::{
    Kind, Listed, Spell, Stamp, entries_of, is_doc_name, quoted, strip_doc_ext, utf16_cmp,
};
use crate::libraries::{ACCOUNT_LIBRARY, Home, Library, is_library_name, library_root};
use crate::paths::{dirname, join};
use crate::rel::to_rel;
use crate::space::{Declined, Space};
use crate::tree::{Limits, mtime_ns};

/// The graph file that makes a folder a chain, the folder of a folder chain's nodes, and an assembly's file.
const GRAPH_FILE: &str = "graph.k7x";
const NODES_DIR: &str = "nodes";
const ASSEMBLY_FILE: &str = "assembly.k7x";

/// The names of the libraries in the libraries' home, sorted (1415).
pub fn library_names(home: &Home) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(&home.libraries) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_library_name(n))
        .collect();
    names.sort();
    names
}

/// `accountLibraries(from)`: each library at the version `from`'s workspace pins.
pub fn account_libraries(home: &Home, from: Option<&str>) -> Result<Vec<Library>, Declined> {
    library_names(home)
        .into_iter()
        .map(|name| {
            let root = library_root(home, &name, from)?;
            Ok(Library { name, root })
        })
        .collect()
}

/// `libraryFamilyRoots`: the account's library first, then the others, each `<root>/<kind>` where that exists.
pub fn family_roots(home: &Home, libraries: &[Library], kind: &str) -> Vec<Library> {
    let account = Library {
        name: ACCOUNT_LIBRARY.into(),
        root: home.account_home(),
    };
    std::iter::once(&account)
        .chain(libraries)
        .map(|l| Library {
            name: l.name.clone(),
            root: join(&[&l.root, kind]),
        })
        .filter(|l| std::fs::metadata(&l.root).is_ok())
        .collect()
}

/// The folder the listings read from: the open project, else the workspace (1420).
pub fn from_of(space: &Space<'_>) -> Result<String, Declined> {
    Ok(space.active()?.unwrap_or_else(|| space.root().to_string()))
}

/// One chain found: its stamp's file, and what the row says.
struct Chain {
    at: String,
    path: String,
    is_file: bool,
    nodes: Vec<String>,
}

struct Walked {
    seen: u64,
    newest: u64,
    depth: u32,
}

/// `nodesIn`: a folder chain's node files, as `.sort()` sorts them; a chain with no `nodes/` yet has none.
fn nodes_in(at: &str, walked: &mut Walked, limits: &Limits) -> Result<Vec<String>, Declined> {
    let Some(entries) = entries_of(&join(&[at, NODES_DIR])) else {
        return Ok(Vec::new());
    };
    walked.seen += entries.len() as u64;
    if walked.seen > limits.max_seen {
        return Err(Declined::TooMany);
    }
    let mut out: Vec<String> = entries
        .into_iter()
        .filter(|(n, k)| *k == Kind::File && is_doc_name(n))
        .map(|(n, _)| n)
        .collect();
    out.sort_by(|a, b| utf16_cmp(a, b));
    Ok(out)
}

struct Frame {
    entries: Vec<(String, Kind)>,
    next: usize,
    abs: String,
    path: String,
}

/// A chain table being filled: a ref held both ways is the file's.
struct Found<'a> {
    by_ref: &'a mut HashMap<String, Chain>,
    library: &'a str,
}

impl Found<'_> {
    fn put(&mut self, c: Chain, path: &str) {
        let r = format!("@{}/{path}", self.library);
        if c.is_file || !self.by_ref.contains_key(&r) {
            self.by_ref.insert(r, c);
        }
    }
}

/// A folder looked at: a chain when a graph file is in it, else the frame its entries are walked by.
fn visit(
    abs: String,
    path: String,
    spell: &Spell<'_>,
    found: &mut Found<'_>,
    walked: &mut Walked,
    limits: &Limits,
) -> Result<Option<Frame>, Declined> {
    let Some(entries) = entries_of(&abs) else {
        return Ok(None);
    };
    walked.seen += entries.len() as u64;
    if walked.seen > limits.max_seen {
        return Err(Declined::TooMany);
    }
    if let Some(ns) = mtime_ns(&abs) {
        walked.newest = walked.newest.max(ns);
    }
    // A graph file makes a FOLDER a chain, so the walk stops there and never reads its nodes as chains.
    if entries
        .iter()
        .any(|(n, k)| *k == Kind::File && n == GRAPH_FILE)
    {
        if !path.is_empty() {
            let nodes = nodes_in(&abs, walked, limits)?;
            let chain = Chain {
                path: spell.of(&abs),
                at: abs,
                is_file: false,
                nodes,
            };
            found.put(chain, &path);
        }
        return Ok(None);
    }
    Ok(Some(Frame {
        entries,
        next: 0,
        abs,
        path,
    }))
}

/// `walkChains` over one `chains/` folder: every chain, a folder chain stopping the walk where it stands.
fn walk_chains(
    spell: &Spell<'_>,
    root: &str,
    found: &mut Found<'_>,
    walked: &mut Walked,
    limits: &Limits,
) -> Result<(), Declined> {
    let mut stack: Vec<Frame> = Vec::new();
    stack.extend(visit(
        root.to_string(),
        String::new(),
        spell,
        found,
        walked,
        limits,
    )?);
    while let Some(frame) = stack.last_mut() {
        let Some((name, kind)) = frame.entries.get(frame.next) else {
            stack.pop();
            continue;
        };
        frame.next += 1;
        let (name, kind) = (name.clone(), *kind);
        let abs = join(&[&frame.abs, &name]);
        let base = frame.path.clone();
        let sub = |n: &str| match base.is_empty() {
            true => n.to_string(),
            false => format!("{base}/{n}"),
        };
        match kind {
            Kind::Dir => {
                let level = u32::try_from(stack.len()).unwrap_or(u32::MAX);
                if level > limits.max_depth {
                    return Err(Declined::TooDeep);
                }
                walked.depth = walked.depth.max(level);
                let path = sub(&name);
                stack.extend(visit(abs, path, spell, found, walked, limits)?);
            }
            Kind::File if is_doc_name(&name) => {
                let path = sub(strip_doc_ext(&name));
                let chain = Chain {
                    path: spell.of(&abs),
                    at: abs,
                    is_file: true,
                    nodes: Vec::new(),
                };
                found.put(chain, &path);
            }
            _ => {}
        }
    }
    Ok(())
}

/// `libraryChainsOf`: every chain of every library, by ref in code point order.
pub fn chains(space: &Space<'_>, limits: &Limits) -> Result<Listed, Declined> {
    let from = from_of(space)?;
    let home = space.home();
    let libraries = account_libraries(home, Some(&from))?;
    let mut walked = Walked {
        seen: 0,
        newest: 0,
        depth: 0,
    };
    let mut by_ref: HashMap<String, Chain> = HashMap::new();
    for l in family_roots(home, &libraries, "chains") {
        let spell = Spell::new(space, &l.root);
        let mut found = Found {
            by_ref: &mut by_ref,
            library: &l.name,
        };
        walk_chains(&spell, &l.root, &mut found, &mut walked, limits)?;
    }
    let mut all: Vec<(String, Chain)> = by_ref.into_iter().collect();
    all.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = b"{\"chains\":[".to_vec();
    let mut rows = Vec::with_capacity(all.len());
    for (i, (r, c)) in all.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"ref\":");
        quoted(&mut out, r);
        out.extend_from_slice(b",\"path\":");
        quoted(&mut out, &c.path);
        out.extend_from_slice(b",\"nodes\":[");
        for (j, n) in c.nodes.iter().enumerate() {
            if j > 0 {
                out.push(b',');
            }
            quoted(&mut out, n);
        }
        out.extend_from_slice(b"]}");
        if out.len() > limits.max_bytes {
            return Err(Declined::TooLarge);
        }
        rows.push(if c.is_file {
            Stamp {
                file: Some(c.at.clone()),
                folder: dirname(&c.at),
            }
        } else {
            Stamp {
                file: None,
                folder: c.at.clone(),
            }
        });
    }
    out.extend_from_slice(b"]}");
    Ok(Listed {
        json: String::from_utf8(out).map_err(|_| Declined::LossyText)?,
        rows,
        newest_folder_ns: walked.newest,
        depth: 0,
        unlisted: Vec::new(),
    })
}

/// A folder's names sorted as `.sort()` sorts them; a failure is what the TypeScript throws (1418).
fn names_sorted(abs: &str, walked: &mut Walked, limits: &Limits) -> Result<Vec<String>, Declined> {
    let Some(entries) = entries_of(abs) else {
        return Err(Declined::WouldThrow);
    };
    walked.seen += entries.len() as u64;
    if walked.seen > limits.max_seen {
        return Err(Declined::TooMany);
    }
    if let Some(ns) = mtime_ns(abs) {
        walked.newest = walked.newest.max(ns);
    }
    let mut names: Vec<String> = entries.into_iter().map(|(n, _)| n).collect();
    names.sort_by(|a, b| utf16_cmp(a, b));
    Ok(names)
}

/// `libraryAssembliesOf`: every folder holding an `assembly.k7x`, library by library, in the walk's order.
pub fn assemblies(space: &Space<'_>, limits: &Limits) -> Result<Listed, Declined> {
    let from = from_of(space)?;
    let home = space.home();
    let libraries = account_libraries(home, Some(&from))?;
    let mut walked = Walked {
        seen: 0,
        newest: 0,
        depth: 0,
    };
    let mut out = b"{\"assemblies\":[".to_vec();
    let mut rows: Vec<Stamp> = Vec::new();
    let is_folder = |p: &str| {
        std::fs::metadata(p)
            .map(|m| m.is_dir())
            .map_err(|_| Declined::WouldThrow)
    };
    for l in family_roots(home, &libraries, "assemblies") {
        // (names, next, folder, ref path) per level; the family root itself is never an assembly.
        let mut stack: Vec<(Vec<String>, usize, String, Vec<String>)> = vec![(
            names_sorted(&l.root, &mut walked, limits)?,
            0,
            l.root.clone(),
            Vec::new(),
        )];
        while let Some((names, next, folder, rel)) = stack.last_mut() {
            let Some(name) = names.get(*next) else {
                stack.pop();
                continue;
            };
            *next += 1;
            let child = join(&[folder, name]);
            if !is_folder(&child)? {
                continue;
            }
            let mut path = rel.clone();
            path.push(name.clone());
            if std::fs::metadata(join(&[&child, ASSEMBLY_FILE])).is_ok() {
                if !rows.is_empty() {
                    out.push(b',');
                }
                out.extend_from_slice(b"{\"ref\":");
                quoted(&mut out, &format!("@{}/{}", l.name, path.join("/")));
                out.extend_from_slice(b",\"path\":");
                quoted(&mut out, &to_rel(space, &child));
                out.extend_from_slice(b",\"file\":");
                let file = join(&[&child, ASSEMBLY_FILE]);
                quoted(&mut out, &to_rel(space, &file));
                out.push(b'}');
                if out.len() > limits.max_bytes {
                    return Err(Declined::TooLarge);
                }
                rows.push(Stamp {
                    file: Some(file),
                    folder: child,
                });
                continue;
            }
            let level = u32::try_from(stack.len()).unwrap_or(u32::MAX);
            if level > limits.max_depth {
                return Err(Declined::TooDeep);
            }
            walked.depth = walked.depth.max(level);
            let names = names_sorted(&child, &mut walked, limits)?;
            stack.push((names, 0, child, path));
        }
    }
    out.extend_from_slice(b"]}");
    Ok(Listed {
        json: String::from_utf8(out).map_err(|_| Declined::LossyText)?,
        rows,
        newest_folder_ns: walked.newest,
        depth: 0,
        unlisted: Vec::new(),
    })
}
