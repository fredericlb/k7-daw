//! tree.rs — the workspace tree as `fileTree` lists it, written once as JSON text in the literal's key order.
//! An explicit stack and two path buffers, so a deep tree cannot overflow and no entry copies its path (1051).
//! What a reader gets wrong: the limits weigh every entry a folder yields, the hidden ones too, not what is written.

use std::time::UNIX_EPOCH;

use crate::confine::{Disk, confine_to};
use crate::filter::{is_binary, project_app, skip};
use crate::paths::{climbs_out, join, relative};
use crate::rel::to_rel;
use crate::space::{Declined, MAX_REL, Space};

/// What a native read costs at most; past any, the piece is declined (1052).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Entries any `readdir` of the walk yields.
    pub max_seen: u64,
    /// Bytes of the JSON written.
    pub max_bytes: usize,
    /// Folders nested below the root.
    pub max_depth: u32,
    /// Bytes of a file read as text.
    pub max_text: u64,
    /// Bytes of a relative path asked.
    pub max_rel: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_seen: 2_000_000,
            max_bytes: 64 << 20,
            max_depth: 256,
            max_text: 64 << 20,
            max_rel: MAX_REL,
        }
    }
}

/// The tree and what the walk met.
#[derive(Debug)]
pub struct Tree {
    pub json: String,
    /// Entries written.
    pub entries: u64,
    /// Folders that could not be listed and stand empty (1054).
    pub unlistable: u32,
    /// The deepest folder, the root's children's folder being 1.
    pub depth: u32,
    /// The newest modification time of a folder walked, in nanoseconds.
    pub newest_folder_ns: u64,
    /// The first of those folders, as the walk spelt them, up to `MAX_UNLISTED`.
    pub unlisted: Vec<String>,
}

/// Unlistable folders a tree names; the count goes on past it.
pub const MAX_UNLISTED: usize = 32;

struct Item {
    off: u32,
    len: u32,
    dir: bool,
}

/// One folder's visible entries, names in one arena, and where the walk is in them.
struct Frame {
    names: String,
    items: Vec<Item>,
    next: usize,
    rel_len: usize,
    abs_len: usize,
}

/// What the walk has counted so far.
struct Walk {
    seen: u64,
    newest: u64,
    /// Bytes of the names kept: each is written twice, so past `max_bytes` the output must be.
    kept: usize,
}

/// A path's modification time in nanoseconds, following links; `None` when it cannot be read.
pub fn mtime_ns(abs: &str) -> Option<u64> {
    let modified = std::fs::metadata(abs).and_then(|m| m.modified()).ok()?;
    let d = modified.duration_since(UNIX_EPOCH).ok();
    Some(d.map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)))
}

/// A folder's entries, folders first then by the bytes of the name; `None` when it cannot be listed.
fn list(abs: &str, walk: &mut Walk, limits: &Limits) -> Result<Option<Frame>, Declined> {
    let Ok(read) = std::fs::read_dir(abs) else {
        return Ok(None);
    };
    if let Some(ns) = mtime_ns(abs) {
        walk.newest = walk.newest.max(ns);
    }
    let mut frame = Frame {
        names: String::new(),
        items: Vec::new(),
        next: 0,
        rel_len: 0,
        abs_len: 0,
    };
    for e in read {
        let Ok(e) = e else { return Ok(None) };
        walk.seen += 1;
        if walk.seen > limits.max_seen {
            return Err(Declined::TooMany);
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        if skip(&name) {
            continue;
        }
        let dir = e.file_type().is_ok_and(|t| t.is_dir());
        let (Ok(off), Ok(len)) = (u32::try_from(frame.names.len()), u32::try_from(name.len()))
        else {
            return Err(Declined::TooLarge);
        };
        walk.kept = walk.kept.saturating_add(name.len());
        if walk.kept > limits.max_bytes {
            return Err(Declined::TooLarge);
        }
        frame.names.push_str(&name);
        frame.items.push(Item { off, len, dir });
    }
    let names = &frame.names;
    let at = |i: &Item| &names[i.off as usize..(i.off + i.len) as usize];
    frame
        .items
        .sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| at(a).cmp(at(b))));
    Ok(Some(frame))
}

fn quoted(out: &mut Vec<u8>, s: &str) {
    // A `&str` serializes to a `Vec` without failing.
    let _ = serde_json::to_writer(&mut *out, s);
}

/// The tree of the workspace as `GET /tree` answers it; a `Declined` where a limit is passed.
pub fn tree(space: &Space<'_>, limits: &Limits) -> Result<Tree, Declined> {
    let root = space.root();
    tree_at(space, root, "", root, limits, &mut 0)
}

/// The walk of `tree` rooted at `folder`, every path spelt `prefix` + `/` + the path below (1422); `seen` runs across calls.
pub fn tree_at(
    space: &Space<'_>,
    folder: &str,
    prefix: &str,
    confine: &str,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Tree, Declined> {
    let root = folder;
    let prefix = if prefix == "." { "" } else { prefix };
    let mut full = String::new();
    let mut walk = Walk {
        seen: *seen,
        newest: 0,
        kept: 0,
    };
    let (mut unlistable, mut depth, mut entries) = (0u32, 0u32, 0u64);
    let mut unlisted: Vec<String> = Vec::new();
    let mut rel = String::new();
    let mut abs = root.to_string();
    let mut out: Vec<u8> = Vec::with_capacity(4096);
    out.push(b'[');
    let mut stack: Vec<Frame> = Vec::new();
    let empty = || Frame {
        names: String::new(),
        items: Vec::new(),
        next: 0,
        rel_len: 0,
        abs_len: 0,
    };
    let mut top = match list(&abs, &mut walk, limits)? {
        Some(f) => f,
        None => {
            unlistable += 1;
            unlisted.push(abs.clone());
            empty()
        }
    };
    top.abs_len = abs.len();
    stack.push(top);
    while let Some(frame) = stack.last_mut() {
        if frame.next == frame.items.len() {
            out.push(b']');
            let done = stack.pop().unwrap_or_else(empty);
            rel.truncate(done.rel_len);
            abs.truncate(done.abs_len);
            if !stack.is_empty() {
                out.push(b'}');
            }
            continue;
        }
        if frame.next > 0 {
            out.push(b',');
        }
        let (off, len, dir) = (
            frame.items[frame.next].off as usize,
            frame.items[frame.next].len as usize,
            frame.items[frame.next].dir,
        );
        frame.next += 1;
        let name = &frame.names[off..off + len];
        let (rel_was, abs_was) = (rel.len(), abs.len());
        // A name holding `\` is spelt with `/` as the shim's `join` spells it, and the walk goes on that spelling (1053).
        let spelt = name
            .contains('\\')
            .then(|| relative(root, &join(&[&abs, name])));
        let mut descend = true;
        match &spelt {
            None => {
                if !rel.is_empty() {
                    rel.push('/');
                }
                rel.push_str(name);
                abs.push('/');
                abs.push_str(name);
            }
            Some(s) => {
                let tail = if rel.is_empty() {
                    Some(s.as_str())
                } else {
                    s.strip_prefix(rel.as_str())
                        .and_then(|t| t.strip_prefix('/'))
                };
                match tail.filter(|t| {
                    !t.is_empty() && t.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
                }) {
                    Some(t) => {
                        if !rel.is_empty() {
                            rel.push('/');
                        }
                        rel.push_str(t);
                        abs.push('/');
                        abs.push_str(t);
                        // The spelling may name a link's far side: it is listed only inside the workspace (1053).
                        descend = confine_to(&[confine], &abs, &Disk).is_some();
                    }
                    None => descend = false,
                }
            }
        }
        let path: &str = spelt.as_deref().unwrap_or(&rel);
        out.extend_from_slice(b"{\"name\":");
        quoted(&mut out, name);
        out.extend_from_slice(b",\"path\":");
        if prefix.is_empty() {
            quoted(&mut out, path);
        } else if spelt.as_deref().is_some_and(climbs_out) {
            // A spelling that leaves the folder is named by the library or workspace it lands in.
            quoted(&mut out, &to_rel(space, &join(&[&abs[..abs_was], name])));
        } else {
            full.clear();
            full.push_str(prefix);
            full.push('/');
            full.push_str(path);
            quoted(&mut out, &full);
        }
        entries += 1;
        if !dir {
            out.extend_from_slice(b",\"dir\":false,\"kind\":");
            out.extend_from_slice(if is_binary(name) {
                b"\"binary\""
            } else {
                b"\"text\""
            });
            out.push(b'}');
            rel.truncate(rel_was);
            abs.truncate(abs_was);
        } else {
            out.extend_from_slice(b",\"dir\":true");
            if project_app(name).is_some()
                && descend
                && std::fs::metadata(&abs).is_ok_and(|m| m.is_dir())
            {
                out.extend_from_slice(b",\"project\":true,\"app\":\"k7\"");
            }
            out.extend_from_slice(b",\"children\":[");
            let level = u32::try_from(stack.len()).unwrap_or(u32::MAX);
            if level > limits.max_depth {
                return Err(Declined::TooDeep);
            }
            depth = depth.max(level);
            let listed = if descend {
                list(&abs, &mut walk, limits)?
            } else {
                None
            };
            if listed.is_none() {
                unlistable += 1;
                if descend && unlisted.len() < MAX_UNLISTED {
                    unlisted.push(abs.clone());
                }
            }
            let mut child = listed.unwrap_or_else(empty);
            (child.rel_len, child.abs_len) = (rel_was, abs_was);
            stack.push(child);
        }
        if out.len() > limits.max_bytes {
            return Err(Declined::TooLarge);
        }
    }
    let json = String::from_utf8(out).map_err(|_| Declined::LossyText)?;
    *seen = walk.seen;
    Ok(Tree {
        json,
        entries,
        unlistable,
        depth,
        newest_folder_ns: walk.newest,
        unlisted,
    })
}
