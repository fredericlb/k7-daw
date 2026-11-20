// views/walk.rs — the folder walks of the hints: the audio under a project, a module's `assets/`, memoised per folder.
// A reader would get wrong that a link is entered (the Source reports it as its target): the depth and the Spend bound a loop.

use super::Spend;
use crate::source::path::{join, parent};
use crate::source::{Entry, Kind, Source, SourceError, Stamp};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// Folders deep the media walk looks below the project (decision 630).
pub const MEDIA_DEPTH: usize = 32;
/// Folders deep a module's `assets/` is read (the TypeScript's `maxDepth`).
pub const ASSET_DEPTH: usize = 6;
/// What asking for a folder weighs, found or not: a call on the Source.
const ASK_COST: usize = 8;
/// What a folder that was there weighs, all told: a disk read behind the call.
const LIST_COST: usize = 200;
/// The extensions a file-valued param takes: `model` a NAM file or an RTNeural JSON, `onnx` its own.
const ASSET_EXT: [&str; 3] = [".nam", ".json", ".onnx"];
const AUDIO_EXT: [&str; 8] = [".wav", ".aif", ".aiff", ".flac", ".mp3", ".ogg", ".m4a", ".opus"];

/// A folder as a loop test sees it: its stamp and what it lists, which tells two folders made in one clock tick apart.
type Print = (Stamp, u64);

/// A folder already on the walk's path, and the one above it.
struct OnPath {
    print: Print,
    id: Option<String>,
    up: Option<usize>,
}

fn print_of(source: &dyn Source, abs: &str, entries: &[Entry]) -> Option<Print> {
    let mut h = DefaultHasher::new();
    for e in entries {
        (&e.name, e.kind == Kind::Dir).hash(&mut h);
    }
    Some((source.stat(abs).ok()?.stamp, h.finish()))
}

/// The folder a loop test is asked about: where it is, how deep, and what it lists.
struct Folder<'a> {
    abs: &'a str,
    entries: &'a [Entry],
    depth: usize,
    max_depth: usize,
}

/// Whether a folder the walk never listed below is a loop; only `Missing` says "never listed", so a refusal or an I/O error is a real folder.
fn never_listed_below(source: &dyn Source, f: &Folder<'_>, spend: &mut Spend) -> bool {
    if f.depth >= f.max_depth {
        return false;
    }
    let mut kids = f
        .entries
        .iter()
        .filter(|e| e.kind == Kind::Dir && !e.name.starts_with('.'));
    let Some(first) = kids.next() else { return false };
    // Every folder below must be unlisted: an incomplete bundle that lacks one child still asks for its files.
    std::iter::once(first)
        .chain(kids)
        .all(|e| spend.step() && matches!(source.list(&join(f.abs, &e.name)), Err(SourceError::Missing)))
}

/// Whether this folder is `from` or above it on the path: by its identity where the source has one, else by print and the memory rule.
fn is_loop(
    source: &dyn Source,
    path: &[OnPath],
    from: Option<usize>,
    here: (&Print, &Option<String>),
    f: &Folder<'_>,
    spend: &mut Spend,
) -> bool {
    let mut memory: Option<bool> = None;
    std::iter::successors(from, |i| path.get(*i).and_then(|n| n.up)).any(|i| {
        let Some(n) = path.get(i) else { return false };
        match (here.1, &n.id) {
            (Some(a), Some(b)) => a == b,
            _ => n.print == *here.0 && *memory.get_or_insert_with(|| never_listed_below(source, f, spend)),
        }
    })
}

/// Every kept file under `root` as (folder, name), relative with `/`; a folder already on the path (or `above` it) is not entered.
pub fn list_under(
    source: &dyn Source,
    root: &str,
    max_depth: usize,
    above: usize,
    spend: &mut Spend,
    keep: &dyn Fn(&str) -> bool,
) -> Result<Vec<(String, String)>, SourceError> {
    let mut found = Vec::new();
    let mut path: Vec<OnPath> = Vec::new();
    let mut up = None;
    let mut over = parent(root);
    for _ in 0..above {
        let Some(dir) = over else { break };
        let Some(listing) = source.list(&dir).ok() else { break };
        let Some(print) = print_of(source, &dir, &listing) else {
            break;
        };
        if !spend.steps(LIST_COST) {
            return Ok(found);
        }
        path.push(OnPath {
            print,
            id: source.folder_id(&dir),
            up,
        });
        up = Some(path.len() - 1);
        over = parent(&dir);
    }
    let mut stack: Vec<(String, String, usize, Option<usize>)> = vec![(root.to_owned(), String::new(), 0, up)];
    while let Some((abs, rel, depth, above_it)) = stack.pop() {
        if !spend.steps(ASK_COST) {
            return Ok(found);
        }
        // `Missing` ends the call; any other trouble with a folder leaves it out of the walk.
        let entries = match source.list(&abs) {
            Ok(e) => e,
            Err(SourceError::Missing) => return Err(SourceError::Missing),
            Err(_) => continue,
        };
        if !spend.steps(LIST_COST - ASK_COST) {
            return Ok(found);
        }
        let Some(print) = print_of(source, &abs, &entries) else {
            continue;
        };
        let id = source.folder_id(&abs);
        let folder = Folder {
            abs: &abs,
            entries: &entries,
            depth,
            max_depth,
        };
        if is_loop(source, &path, above_it, (&print, &id), &folder, spend) {
            continue;
        }
        path.push(OnPath {
            print,
            id,
            up: above_it,
        });
        let here = Some(path.len() - 1);
        for e in entries {
            if !spend.step() {
                return Ok(found);
            }
            if e.name.starts_with('.') {
                continue;
            }
            match e.kind {
                Kind::Dir if depth < max_depth => {
                    let child = if rel.is_empty() {
                        e.name.clone()
                    } else {
                        format!("{rel}/{}", e.name)
                    };
                    if !spend.bytes(child.len()) {
                        return Ok(found);
                    }
                    stack.push((join(&abs, &e.name), child, depth + 1, here));
                }
                Kind::Dir => {}
                Kind::File if keep(&e.name) => {
                    if !spend.bytes(rel.len() + e.name.len()) {
                        return Ok(found);
                    }
                    found.push((rel.clone(), e.name));
                }
                Kind::File => {}
            }
        }
    }
    Ok(found)
}

/// A name's extension as `path.extname` has it, lowercased; a name that is only an extension has none.
fn ext_of(name: &str) -> Option<String> {
    let at = name.rfind('.').filter(|i| *i > 0)?;
    Some(name[at..].to_lowercase())
}

/// The audio files of a project by their project-relative folder, files and folders sorted (decision 630).
pub fn media_of(
    source: &dyn Source,
    project: &str,
    spend: &mut Spend,
) -> Result<Vec<(String, Vec<String>)>, SourceError> {
    let is_audio = |name: &str| ext_of(name).is_some_and(|e| AUDIO_EXT.contains(&e.as_str()));
    let files = list_under(source, project, MEDIA_DEPTH, MEDIA_DEPTH, spend, &is_audio)?;
    let mut by_folder: HashMap<String, Vec<String>> = HashMap::new();
    for (folder, name) in files {
        by_folder.entry(folder).or_default().push(name);
    }
    let mut out: Vec<(String, Vec<String>)> = by_folder.into_iter().collect();
    for (_, names) in &mut out {
        names.sort_unstable();
    }
    out.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The files a module's `assets/` holds, memoised per folder: ancestors repeat across modules (decision 631).
#[derive(Default)]
pub struct Assets {
    memo: HashMap<String, Arc<Vec<String>>>,
}

impl Assets {
    /// The asset files under `<folder>/assets`, relative with `/`, sorted; walked once per folder.
    pub fn assets_in(
        &mut self,
        source: &dyn Source,
        folder: &str,
        spend: &mut Spend,
    ) -> Result<Arc<Vec<String>>, SourceError> {
        if let Some(held) = self.memo.get(folder) {
            return Ok(held.clone());
        }
        let is_asset = |name: &str| {
            let lower = name.to_lowercase();
            ASSET_EXT.iter().any(|e| lower.ends_with(e))
        };
        let files = list_under(source, &join(folder, "assets"), ASSET_DEPTH, 0, spend, &is_asset)?;
        let mut paths: Vec<String> = files
            .into_iter()
            .map(|(dir, name)| if dir.is_empty() { name } else { format!("{dir}/{name}") })
            .collect();
        paths.sort_unstable();
        let held = Arc::new(paths);
        // A walk the budget cut is not kept: a later ask must not take half a folder for the whole.
        if !spend.capped() {
            self.memo.insert(folder.to_owned(), held.clone());
        }
        Ok(held)
    }
}
