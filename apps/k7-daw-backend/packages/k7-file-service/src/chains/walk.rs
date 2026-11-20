//! chains/walk.rs — `libraryChainDirs`: every chain of every library, each ref once, a file's over a folder's.
//! What a reader gets wrong: a link is neither a folder nor a file here, as node's `readdir` typed it.

use std::collections::HashMap;

use song::templates::{ChainAt, Layout};

use crate::listings::{Kind, entries_of, is_doc_name, strip_doc_ext};
use crate::paths::join;
use crate::space::Declined;
use crate::tree::Limits;

/// One chain of a library: its ref, which library and path that is, and where it lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainDir {
    pub ref_: String,
    pub library: String,
    pub path: String,
    pub at: String,
    pub layout: Layout,
    pub library_root: String,
}

impl ChainDir {
    pub(crate) fn at(&self) -> ChainAt<'_> {
        ChainAt {
            at: &self.at,
            layout: self.layout,
            library_root: &self.library_root,
        }
    }
}

/// A folder holding a `graph.k7x` is a chain and its nodes are not read as chains.
const GRAPH_FILE: &str = "graph.k7x";

struct Frame {
    entries: Vec<(String, Kind)>,
    next: usize,
    abs: String,
    path: String,
}

/// Every chain under one library's `chains/` folder; an unreadable folder answers nothing.
fn walk_library(
    library: &str,
    library_root: &str,
    by_ref: &mut HashMap<String, ChainDir>,
    seen: &mut u64,
    limits: &Limits,
) -> Result<(), Declined> {
    let root = join(&[library_root, "chains"]);
    let mut put = |c: ChainDir| {
        if c.layout == Layout::File || !by_ref.contains_key(&c.ref_) {
            by_ref.insert(c.ref_.clone(), c);
        }
    };
    let mut stack: Vec<Frame> = Vec::new();
    let open = |abs: String, path: String, seen: &mut u64| -> Result<Option<Frame>, Declined> {
        let Some(entries) = entries_of(&abs) else {
            return Ok(None);
        };
        *seen += entries.len() as u64;
        if *seen > limits.max_seen {
            return Err(Declined::TooMany);
        }
        Ok(Some(Frame {
            entries,
            next: 0,
            abs,
            path,
        }))
    };
    // As `walkChains`: a graph file at the `chains/` root stops the library's walk, a chain of no name.
    if let Some(top) = open(root, String::new(), seen)?
        && !top
            .entries
            .iter()
            .any(|(n, k)| *k == Kind::File && n == GRAPH_FILE)
    {
        stack.push(top);
    }
    while let Some(frame) = stack.last_mut() {
        let Some((name, kind)) = frame.entries.get(frame.next).cloned() else {
            stack.pop();
            continue;
        };
        frame.next += 1;
        let abs = join(&[&frame.abs, &name]);
        let base = frame.path.clone();
        let level = stack.len();
        let sub = |n: &str| {
            if base.is_empty() {
                n.to_owned()
            } else {
                format!("{base}/{n}")
            }
        };
        match kind {
            Kind::Dir => {
                if u32::try_from(level).unwrap_or(u32::MAX) > limits.max_depth {
                    return Err(Declined::TooDeep);
                }
                let path = sub(&name);
                let Some(next) = open(abs.clone(), path.clone(), seen)? else {
                    continue;
                };
                if next
                    .entries
                    .iter()
                    .any(|(n, k)| *k == Kind::File && n == GRAPH_FILE)
                {
                    if !path.is_empty() {
                        put(ChainDir {
                            ref_: format!("@{library}/{path}"),
                            library: library.to_owned(),
                            path,
                            at: abs,
                            layout: Layout::Folder,
                            library_root: library_root.to_owned(),
                        });
                    }
                } else {
                    stack.push(next);
                }
            }
            Kind::File if is_doc_name(&name) => {
                let path = sub(strip_doc_ext(&name));
                put(ChainDir {
                    ref_: format!("@{library}/{path}"),
                    library: library.to_owned(),
                    path,
                    at: abs,
                    layout: Layout::File,
                    library_root: library_root.to_owned(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// The walk of every library in `roots`, the account's first as the roots order them, sorted by ref in code points.
pub(crate) fn walk(roots: &song::roots::Roots, limits: &Limits) -> Result<Vec<ChainDir>, Declined> {
    let mut by_ref = HashMap::new();
    let mut seen = 0;
    for l in &roots.libraries {
        walk_library(&l.name, &l.root, &mut by_ref, &mut seen, limits)?;
    }
    let mut all: Vec<ChainDir> = by_ref.into_values().collect();
    all.sort_by(|a, b| a.ref_.cmp(&b.ref_));
    Ok(all)
}
