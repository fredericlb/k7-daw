//! demos.rs — Examples kept stocked with the bundled demos, as `demos.ts` mirrors them: a demo is replaced whole when
//! either side moved since the last mirror, and left alone when both are as it left them.
//! What a reader gets wrong: the stamps are spelled as the TypeScript spells them (`String(mtimeMs)`, UTF-16 order),
//! because the stamps file written by one implementation is read by the other whenever the switch flips (1641).

use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::Read;

use serde_json::{Map, Value, json};
use sha1::{Digest, Sha1};

use super::copy::{apply_entries, plan_tree};
use super::ms;
use super::names::{child_below, dir_names, host_order};
use crate::entries::apply::{rename_once, temp_for};
use crate::paths::{basename, dirname, join};
use crate::tree::Limits;

/// Beside the demos, hidden from every walk; Examples removed takes it along, so the next start mirrors all.
const STAMP_FILE: &str = ".studio-demos.json";
/// The most a stamps file is read at; a larger one reads as none, as a file that does not parse.
const MAX_STAMPS: u64 = 64 << 20;
/// How far up from the code the bundle is looked for.
const LEVELS: usize = 8;

/// The bundle's folder: the account's `STUDIO_DEMOS` when it exists, else the first `demos` folder walking up from the code's root; none is no mirror.
pub fn demos_dir(env_demos: Option<&str>, code_root: &str) -> Option<String> {
    if let Some(env) = env_demos.filter(|e| !e.is_empty())
        && fs::metadata(env).is_ok()
    {
        return Some(env.to_string());
    }
    let mut dir = join(&[code_root]);
    for _ in 0..LEVELS {
        let candidate = join(&[&dir, "demos"]);
        if fs::metadata(&candidate).is_ok() {
            return Some(candidate);
        }
        let up = dirname(&dir);
        if up == dir {
            break;
        }
        dir = up;
    }
    None
}

/// The bundle's demo folders: the dirents that are folders and no dot name, in the host's listing order.
pub fn bundled_names(src: &str) -> Vec<String> {
    let mut names: Vec<String> = dir_names(src, &Limits::default(), &mut 0)
        .unwrap_or_default()
        .into_iter()
        .filter(|(n, d)| *d && !n.starts_with('.'))
        .map(|(n, _)| n)
        .collect();
    host_order(&mut names);
    names
}

/// The caches and VCS noise a checkout accumulates: per-machine state, and a stale cache is worse than none.
fn keep(name: &str) -> bool {
    name != "node_modules" && name != ".git" && !name.starts_with(".studio-")
}

/// `treeStamp`: a SHA-1 of every file the mirror copies, by path, size and mtime; "" when the walk fails.
pub fn tree_stamp(dir: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut stack = vec![(dir.to_string(), String::new())];
    while let Some((at, rel)) = stack.pop() {
        let Ok(read) = fs::read_dir(&at) else {
            return String::new();
        };
        for e in read {
            let Ok(e) = e else {
                return String::new();
            };
            let name = e.file_name().to_string_lossy().into_owned();
            // A `\`-spelled name folding out of its folder loops the TypeScript's walk until it fails, so the walk fails.
            let Some(abs) = child_below(&at, &name) else {
                return String::new();
            };
            if !keep(&basename(&abs)) {
                continue;
            }
            let r = format!("{rel}/{name}");
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push((abs, r));
            } else {
                let Ok(m) = fs::metadata(&abs) else {
                    return String::new();
                };
                lines.push(format!(
                    "{r}:{}:{}",
                    m.len(),
                    k7_js::number::to_string(ms(m.modified()))
                ));
            }
        }
    }
    // `Array.prototype.sort()` orders by UTF-16 code units, which is not the bytes' order past the BMP.
    lines.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let mut h = Sha1::new();
    h.update(lines.join("\n").as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// `readStamps`: the stamps file's JSON object, or none for a file that is missing, large or does not parse.
fn read_stamps(examples: &str) -> Map<String, Value> {
    let Ok(f) = fs::File::open(join(&[examples, STAMP_FILE])) else {
        return Map::new();
    };
    let mut bytes = Vec::new();
    if f.take(MAX_STAMPS + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > MAX_STAMPS {
        return Map::new();
    }
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn write_stamps(examples: &str, stamps: &Map<String, Value>) {
    let file = join(&[examples, STAMP_FILE]);
    let rand = RandomState::new().build_hasher().finish() as u32;
    let tmp = temp_for(&file, std::process::id(), rand);
    let text = Value::Object(stamps.clone()).to_string();
    // Unwritten, the next start mirrors everything again, as it would without a stamp.
    if fs::write(&tmp, text).is_err()
        || rename_once(&crate::entries::apply::Disk, &tmp, &file).is_err()
    {
        let _ = fs::remove_file(&tmp);
    }
}

/// Both stamps as the last mirror left them: nothing to copy.
fn current(was: Option<&Value>, from: &str, dest: &str) -> bool {
    was.is_some_and(|w| {
        w.get("src").and_then(Value::as_str) == Some(from)
            && w.get("dest").and_then(Value::as_str) == Some(&tree_stamp(dest))
    })
}

/// What a mirror would do: the demos it copies, and the ones it leaves, both in the bundle's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mirror {
    pub copy: Vec<String>,
    pub kept: Vec<String>,
}

/// `mirrorDemos`, read-only: which demos differ from the last mirror. Writes and creates nothing.
pub fn plan_mirror(src: &str, examples: &str) -> Mirror {
    let mut m = Mirror {
        copy: Vec::new(),
        kept: Vec::new(),
    };
    let stamps = read_stamps(examples);
    for name in bundled_names(src) {
        let from = tree_stamp(&join(&[src, &name]));
        if current(stamps.get(&name), &from, &join(&[examples, &name])) {
            m.kept.push(name);
        } else {
            m.copy.push(name);
        }
    }
    m
}

/// Replace each bundled demo folder wholesale, leaving the user's own projects alone; the names it synced. Never fails:
/// one bad demo costs that demo, and Examples that cannot be made is no mirror.
pub fn mirror(src: &str, examples: &str) -> Vec<String> {
    let names = bundled_names(src);
    if names.is_empty() || fs::create_dir_all(examples).is_err() {
        return Vec::new();
    }
    let stamps = read_stamps(examples);
    let mut next = Map::new();
    let mut synced = Vec::new();
    let limits = Limits::default();
    for folder in names {
        let dest = join(&[examples, &folder]);
        let from = tree_stamp(&join(&[src, &folder]));
        let was = stamps.get(&folder);
        if current(was, &from, &dest) {
            if let Some(w) = was {
                next.insert(folder, w.clone());
            }
            continue;
        }
        // Replace, don't merge: removed first, so a file taken out of a demo cannot linger.
        let copied = super::life::rm(&dest, true).and_then(|()| {
            let plan = plan_tree(&join(&[src, &folder]), &dest, &keep, &limits, &mut 0)?;
            apply_entries(&plan)
        });
        if copied.is_ok() {
            next.insert(
                folder.clone(),
                json!({ "src": from, "dest": tree_stamp(&dest) }),
            );
            synced.push(folder);
        }
    }
    write_stamps(examples, &next);
    synced
}
