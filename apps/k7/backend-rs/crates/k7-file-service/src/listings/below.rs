//! below.rs — `filesBelow`: every file under a folder at any depth that a name test accepts (1411).
//! An explicit stack, so a deep nest cannot overflow; a folder that cannot be listed is no files.
//! What a reader gets wrong: below the folder asked a link is neither a folder nor a file, and nothing is hidden.

use super::{Kind, Spell, entries_of};
use crate::paths::join;
use crate::space::{Declined, Space};
use crate::tree::{Limits, mtime_ns};

/// A file found: where it is, as the walk spelt it, and as the workspace names it.
pub struct Found {
    pub abs: String,
    pub rel: String,
}

/// The files, in the order the TypeScript's recursion finds them, and what the walk met.
pub struct Below {
    pub files: Vec<Found>,
    pub newest_folder_ns: u64,
    pub depth: u32,
}

struct Frame {
    entries: Vec<(String, Kind)>,
    next: usize,
    abs: String,
}

/// The files below `at` that `keep` accepts; `seen` counts every entry any folder yields, across calls.
pub fn files_below(
    space: &Space<'_>,
    at: &str,
    keep: &dyn Fn(&str) -> bool,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Below, Declined> {
    let spell = Spell::new(space, at);
    let mut out = Below {
        files: Vec::new(),
        newest_folder_ns: 0,
        depth: 0,
    };
    let mut stack: Vec<Frame> = Vec::new();
    let mut enter =
        |abs: String, stack: &mut Vec<Frame>, out: &mut Below| -> Result<(), Declined> {
            let Some(entries) = entries_of(&abs) else {
                return Ok(());
            };
            *seen += entries.len() as u64;
            if *seen > limits.max_seen {
                return Err(Declined::TooMany);
            }
            if let Some(ns) = mtime_ns(&abs) {
                out.newest_folder_ns = out.newest_folder_ns.max(ns);
            }
            stack.push(Frame {
                entries,
                next: 0,
                abs,
            });
            Ok(())
        };
    enter(at.to_string(), &mut stack, &mut out)?;
    while let Some(frame) = stack.last_mut() {
        let Some((name, kind)) = frame.entries.get(frame.next) else {
            stack.pop();
            continue;
        };
        frame.next += 1;
        let (name, kind) = (name.clone(), *kind);
        if kind == Kind::Other || (kind == Kind::File && !keep(&name)) {
            continue;
        }
        // A name holding `\` is spelt with `/` as the shim's `join` spells it (1053).
        let abs = if name.contains('\\') {
            join(&[&frame.abs, &name])
        } else if frame.abs.ends_with('/') {
            format!("{}{name}", frame.abs)
        } else {
            format!("{}/{name}", frame.abs)
        };
        match kind {
            Kind::File => {
                let rel = spell.of(&abs);
                out.files.push(Found { abs, rel });
            }
            _ => {
                let level = u32::try_from(stack.len()).unwrap_or(u32::MAX);
                if level > limits.max_depth {
                    return Err(Declined::TooDeep);
                }
                out.depth = out.depth.max(level);
                enter(abs, &mut stack, &mut out)?;
            }
        }
    }
    Ok(out)
}
