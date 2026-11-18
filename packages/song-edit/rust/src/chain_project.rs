// chain_project.rs — a chain project read into chains, nodes and texts, and its writers: chain files and `layout.k7x`.
// A reader would get wrong that a chain is read twice: song's decoder vets it and types it for `to_segments`; the editor keeps its own plain data.

use crate::chain_data::{Chain, Item, Step, is_track_item, is_video_item, lane_of, name_of};
use crate::chain_item_edit::{CHAINS_DIR, TRACKS_DIR, chain_file_path};
use crate::error::{Res, fail, io_error};
use crate::format_chain::format_chain;
use crate::fs::Fs;
use crate::paths::{basename, join};
use crate::sugar::{is_doc_file, strip_doc_ext};
use crate::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{Document, Value, js_number, print_key};
use song::PathRule;
use song::routing::ChainFile as TypedChain;
use std::collections::HashSet;
use std::sync::Arc;

pub const LAYOUT_FILE: &str = "layout.k7x";
const PICTURE_TRACK_REF: &str = "@meta/picture-track";

/// One node as the loader sees it: where it sits, and what it says.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainNode {
    pub name: String,
    pub id: String,
    pub chain: String,
    /// The subchain of `chain` it sits in, absent for the chain's own steps.
    pub sub: Option<String>,
    pub step: usize,
    /// Its index in a parallel step; None for a step holding one item.
    pub item: Option<usize>,
    /// The lane it IS, of either kind.
    pub track: Option<String>,
    /// Set on a PICTURE lane: it holds no module, and its placements are not in `tracks/`.
    pub picture: bool,
    pub value: Item,
}

#[derive(Clone, Debug, Default)]
pub struct ChainProject {
    pub chains: IndexMap<String, Chain>,
    /// The chains marked `hidden`, set aside whole: no node of theirs is in `nodes`, and no writer rewrites them.
    pub hidden: IndexMap<String, Chain>,
    /// Each chain file's text as read, so a rewrite keeps its comments.
    pub texts: IndexMap<String, String>,
    pub nodes: IndexMap<String, ChainNode>,
    pub issues: Vec<String>,
    /// The same chains typed by song's decoder, what `to_segments` reads.
    pub typed: IndexMap<String, Arc<TypedChain>>,
}

pub(crate) fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Whether `tracks/<lane>.k7x` is a PICTURE lane: it extends `@meta/picture-track`.
fn is_picture_lane_file(fs: &dyn Fs, dir: &str, lane: &str) -> bool {
    let file = join(&[dir, TRACKS_DIR, &format!("{lane}.k7x")]);
    // A lane file that will not read is reported where it is loaded.
    fs.exists(&file)
        && ws_data(fs, &file).is_ok_and(|d| matches!(d.get("extends"), Some(Value::Str(s)) if s == PICTURE_TRACK_REF))
}

fn chain_names(fs: &dyn Fs, folder: &str) -> Res<Vec<String>> {
    let mut names: HashSet<String> = HashSet::new();
    for e in fs.list(folder).map_err(|e| io_error(&e, "scandir", folder))? {
        let name = strip_doc_ext(&e.name);
        if !name.is_empty() && is_doc_file(&e.name) {
            names.insert(name.to_owned());
        }
    }
    // `polysynth-copy` sorts before `polysynth` by file name but not by chain name, and lanes follow this order.
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort_by_key(|n| utf16(n));
    Ok(names)
}

/// Every chain file, decoded; a file that fails is reported and left out rather than half-read.
///
/// # Errors
/// A folder that does not list, or a chain file that cannot be read.
pub fn read_chain_project(fs: &dyn Fs, dir: &str) -> Res<ChainProject> {
    let mut out = ChainProject::default();
    let folder = join(&[dir, CHAINS_DIR]);
    if !fs.exists(&folder) {
        return Ok(out);
    }
    for name in chain_names(fs, &folder)? {
        let file = chain_file_path(dir, &name);
        let f = basename(&file).to_owned();
        let text = ws_text(fs, &file)?;
        let typed = match song::routing::decode_chain(&format!("{CHAINS_DIR}/{f}"), &text, PathRule::HOST) {
            Ok(t) => t,
            Err(e) => {
                out.issues
                    .push(format!("{}:{}:{}: {}", e.file, e.line, e.col, e.message));
                continue;
            }
        };
        let chain = Chain::from_value(&Document::new(&text).to_js()?);
        // A hidden chain is not part of the project as compiled: its lane, its nodes and its links are all left out.
        if typed.hidden == Some(true) {
            out.hidden.insert(name, chain);
            continue;
        }
        for (sub, steps) in chain.step_lists() {
            for (s, step) in steps.iter().enumerate() {
                for (k, it) in step.items().iter().enumerate() {
                    let n = name_of(it).to_owned();
                    if let Some(first) = out.nodes.get(&n) {
                        out.issues.push(format!(
                            "{CHAINS_DIR}/{f}: \"{n}\" is already a node of chain \"{}\"",
                            first.chain
                        ));
                    }
                    let picture = is_video_item(it)
                        || (is_track_item(it) && lane_of(it).is_some_and(|l| is_picture_lane_file(fs, dir, l)));
                    let node = ChainNode {
                        name: n.clone(),
                        id: crate::chain_data::id_of(it).to_owned(),
                        chain: name.clone(),
                        sub: sub.map(str::to_owned),
                        step: s,
                        item: matches!(step, Step::Many(_)).then_some(k),
                        track: lane_of(it).map(str::to_owned),
                        picture,
                        value: it.clone(),
                    };
                    out.nodes.insert(n, node);
                }
            }
        }
        out.typed.insert(name.clone(), Arc::new(typed));
        out.texts.insert(name.clone(), text);
        out.chains.insert(name, chain);
    }
    Ok(out)
}

/// The pins as the file wrote them, `{}` when there is none.
///
/// # Errors
/// A layout file that does not read.
pub fn read_layout(fs: &dyn Fs, dir: &str) -> Res<Value> {
    let f = join(&[dir, LAYOUT_FILE]);
    if !fs.exists(&f) {
        return Ok(Value::Map(vec![]));
    }
    Ok(match ws_data(fs, &f)?.get("positions") {
        Some(m @ Value::Map(_)) => m.clone(),
        _ => Value::Map(vec![]),
    })
}

fn pin(p: &Value, k: &str) -> String {
    match p.get(k) {
        Some(Value::Num(n)) => js_number(*n),
        _ => "undefined".to_owned(),
    }
}

/// The pins, set by the fewest edits so a moved card is a one-value diff; no pin left removes the file.
///
/// # Errors
/// A file that does not read, or the write's.
pub fn write_layout(fs: &mut dyn Fs, dir: &str, positions: &Value) -> Res<()> {
    let f = join(&[dir, LAYOUT_FILE]);
    let pins = match positions {
        Value::Map(m) => m.as_slice(),
        _ => &[],
    };
    if pins.is_empty() {
        return fs.remove(&f).map_err(|e| io_error(&e, "rm", &f));
    }
    if fs.exists(&f) {
        let data = Value::Map(vec![("positions".to_owned(), positions.clone())]);
        ws_edit(fs, &f, EditOpts::default(), |doc| Ok(doc.apply_in(&[], &data)?))?;
        return Ok(());
    }
    // A new file puts one card a line, so every later move is a one-line diff.
    let cards: String = pins
        .iter()
        .map(|(k, p)| format!("  {}: {{ x: {}, y: {} }},\n", print_key(k), pin(p, "x"), pin(p, "y")))
        .collect();
    ws_write_text(fs, &f, &format!("positions({{\n{cards}}})\n"))?;
    Ok(())
}

/// Write each chain whose text differs from the file on disk, and remove the files of chains gone.
///
/// # Errors
/// A chain that does not print, or a read or write that fails.
pub fn write_chains(
    fs: &mut dyn Fs,
    dir: &str,
    chains: &IndexMap<String, Chain>,
    prev_texts: &IndexMap<String, String>,
    keep: &HashSet<String>,
) -> Res<Vec<String>> {
    let folder = join(&[dir, CHAINS_DIR]);
    fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
    let mut out = vec![];
    let on_disk = |fs: &dyn Fs, name: &str| -> Res<Option<String>> {
        let f = chain_file_path(dir, name);
        if fs.exists(&f) {
            ws_text(fs, &f).map(Some)
        } else {
            Ok(None)
        }
    };
    for (name, c) in chains {
        let disk = on_disk(fs, name)?;
        let prev = prev_texts
            .get(name)
            .cloned()
            .or_else(|| disk.clone())
            .unwrap_or_default();
        let text = format_chain(c, &prev)?;
        if disk.as_deref() == Some(text.as_str()) {
            continue;
        }
        ws_write_text(fs, &chain_file_path(dir, name), &text)?;
        out.push(format!("{CHAINS_DIR}/{name}.k7x"));
    }
    for name in read_chain_project(fs, dir)?.chains.keys() {
        if chains.contains_key(name) || keep.contains(name) {
            continue;
        }
        let f = chain_file_path(dir, name);
        if fs.exists(&f) {
            fs.remove(&f).map_err(|e| io_error(&e, "rm", &f))?;
            out.push(format!("{CHAINS_DIR}/{name}.k7x"));
        }
    }
    Ok(out)
}

/// The project re-keyed for a lane rename, so the chain and the item follow the name.
#[must_use]
pub fn renamed(cp: &ChainProject, from: &str, to: &str) -> ChainProject {
    let mut out = cp.clone();
    let n = out.nodes.shift_remove(from);
    if let Some(mut n) = n.clone() {
        n.name = to.to_owned();
        n.track = Some(to.to_owned());
        // Always `track`, as the TypeScript spreads it: `raw_items` is what a picture lane's item is written from.
        crate::chain_data::set_key(&mut n.value, "track", Value::Str(to.to_owned()));
        out.nodes.insert(to.to_owned(), n);
    }
    if n.is_some_and(|n| n.chain == from) && !out.chains.contains_key(to) {
        if let Some(c) = out.chains.shift_remove(from) {
            out.chains.insert(to.to_owned(), c);
        }
        if let Some(t) = out.texts.shift_remove(from) {
            out.texts.insert(to.to_owned(), t);
        }
        for n in out.nodes.values_mut().filter(|n| n.chain == from) {
            n.chain = to.to_owned();
        }
    }
    out
}

/// Every item as WRITTEN, by id: the parsed value lists the schema's keys first, and a rewrite must not reorder.
///
/// # Errors
/// A chain text that does not parse.
pub fn raw_items(cp: &ChainProject) -> Res<IndexMap<String, Item>> {
    let mut out = IndexMap::new();
    for text in cp.texts.values() {
        let file = Document::new(text).to_js()?;
        let mut lists: Vec<&Value> = vec![];
        lists.extend(file.get("steps"));
        if let Some(Value::List(subs)) = file.get("subchains") {
            lists.extend(subs.iter().filter_map(|sc| sc.get("steps")));
        }
        for steps in lists {
            let Value::List(steps) = steps else { continue };
            for s in steps {
                let items: &[Value] = match s {
                    Value::List(l) => l,
                    other => std::slice::from_ref(other),
                };
                for i in items {
                    if let Some(Value::Str(id)) = i.get("id") {
                        out.insert(id.clone(), i.clone());
                    }
                }
            }
        }
    }
    Ok(out)
}

/// A node a chain project holds, or the sentence for one it does not.
///
/// # Errors
/// A name no chain holds.
pub fn node_of<'a>(cp: &'a ChainProject, node: &str) -> Res<&'a ChainNode> {
    match cp.nodes.get(node) {
        Some(n) => Ok(n),
        None => fail(format!("no node '{node}' in this project")),
    }
}
