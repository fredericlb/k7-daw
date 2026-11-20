// chain_item_edit.rs — editing ONE node of a chain project, whose text is an item inside a chain file, with a node file's writers.
// A reader would get wrong that a writer sees the whole item, but `id`, `bypass`, `vstpreset` and `mapping` are the USE's: they come back.

use crate::error::{Error, Res, fail, io_error};
use crate::fs::Fs;
use crate::module_io::{ModuleFile, entity_file_for};
use crate::node_files::NODES_DIR;
use crate::paths::join;
use crate::sugar::is_doc_file;
use crate::workspace_doc::{ws_data, ws_text, ws_write_text};
use k7x::edit::{Document, PathSeg, Scope, StringifyOptions, Tree, Value, WriteOptions, same, stringify};
use song::Cache;
use song::{Kind, PathRule, Source};

pub const CHAINS_DIR: &str = "chains";
pub const TRACKS_DIR: &str = "tracks";
const NODE_ITEM_KEYS: [&str; 4] = ["id", "bypass", "vstpreset", "mapping"];

/// Its presence IS the format: a project with `chains/` is read from it, and never from graph.k7x.
#[must_use]
pub fn is_chain_project(fs: &dyn Fs, dir: &str) -> bool {
    fs.stat(&join(&[dir, CHAINS_DIR])).is_ok_and(|s| s.kind == Kind::Dir)
}

/// The file chain `chain` is read from and written to.
#[must_use]
pub fn chain_file_path(dir: &str, chain: &str) -> String {
    join(&[dir, CHAINS_DIR, &format!("{chain}.k7x")])
}

/// The folder a project keeps its LANES' files in: `tracks/` in a chain project, `nodes/` otherwise.
#[must_use]
pub fn lane_folder(fs: &dyn Fs, dir: &str) -> &'static str {
    if is_chain_project(fs, dir) {
        TRACKS_DIR
    } else {
        NODES_DIR
    }
}

/// A lane's own file, the one place its events, takes and alternatives are written, or None.
#[must_use]
pub fn lane_file_for(fs: &dyn Fs, dir: &str, lane: &str) -> Option<ModuleFile> {
    let sub = lane_folder(fs, dir);
    entity_file_for(fs, &join(&[dir, sub]), sub, lane)
}

/// Where lane `id`'s `events` live: the file, and the path inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct EventsAt {
    pub path: String,
    pub base: Vec<PathSeg>,
}

/// The timeline and lane an address `<timeline>><lane>` names, or None for a project lane.
#[must_use]
pub fn timeline_lane_of(id: &str) -> Option<(&str, &str)> {
    let at = id.find('>')?;
    (at > 0 && at < id.len() - 1).then(|| (&id[..at], &id[at + 1..]))
}

/// Where lane `id`'s `events` live: its own file at the root, or a timeline's lane inside the timeline's file.
///
/// # Errors
/// A timeline file that does not read.
pub fn lane_events_at(fs: &dyn Fs, dir: &str, id: &str) -> Res<Option<EventsAt>> {
    let Some((timeline, lane)) = timeline_lane_of(id) else {
        return Ok(lane_file_for(fs, dir, id).map(|own| EventsAt {
            path: own.path,
            base: vec![],
        }));
    };
    let Some(file) = entity_file_for(fs, &join(&[dir, "timelines"]), "timelines", timeline) else {
        return Ok(None);
    };
    let data = ws_data(fs, &file.path)?;
    let at = match data.get("lanes") {
        Some(Value::List(l)) => l
            .iter()
            .position(|x| matches!(x.get("lane"), Some(Value::Str(s)) if s == lane)),
        _ => None,
    };
    Ok(at.map(|i| EventsAt {
        path: file.path,
        base: vec![PathSeg::Key("lanes".to_owned()), PathSeg::Index(i)],
    }))
}

/// Where a chain node's item sits: its chain file, and the item's id inside that file.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainItemRef {
    pub chain: String,
    pub path: String,
    pub id: String,
}

/// The chain item a non-lane node `name` is, or None: a lane is a file of its own (`lane_file_for`).
///
/// # Errors
/// A folder that does not list.
pub fn chain_item_of(fs: &dyn Fs, dir: &str, name: &str) -> Res<Option<ChainItemRef>> {
    if !is_chain_project(fs, dir) {
        return Ok(None);
    }
    let source: &dyn Source = fs;
    let cp =
        song::routing::read_chain_project(source, &mut Cache::new(), dir).map_err(|e| io_error(&e, "scandir", dir))?;
    Ok(cp.nodes.get(name).filter(|n| n.track.is_none()).map(|n| ChainItemRef {
        chain: n.chain.clone(),
        path: chain_file_path(dir, &n.chain),
        id: n.id.clone(),
    }))
}

fn step_seqs(doc: &Document) -> Res<Vec<Vec<PathSeg>>> {
    let key = |s: &str| PathSeg::Key(s.to_owned());
    let is_list = |p: &[PathSeg]| doc.node_at(p, |n| matches!(n, Some(Tree::List { .. })));
    let mut out = vec![];
    if is_list(&[key("steps")])? {
        out.push(vec![key("steps")]);
    }
    let subs = doc.node_at(&[key("subchains")], |n| match n {
        Some(Tree::List { items, .. }) => items.len(),
        _ => 0,
    })?;
    for j in 0..subs {
        let at = vec![key("subchains"), PathSeg::Index(j), key("steps")];
        if is_list(&at)? {
            out.push(at);
        }
    }
    Ok(out)
}

/// Every item of a chain document that carries an `id`, with its path: a step's own item, or one of a parallel step's.
///
/// # Errors
/// A document that does not parse.
pub fn chain_item_paths(doc: &Document) -> Res<Vec<(String, Vec<PathSeg>)>> {
    let mut out = vec![];
    for steps in step_seqs(doc)? {
        let slots = doc.node_at(&steps, |n| match n {
            Some(Tree::List { items, .. }) => items
                .iter()
                .enumerate()
                .flat_map(|(i, s)| match s {
                    Tree::List { items: inner, .. } => inner
                        .iter()
                        .enumerate()
                        .map(|(k, m)| (vec![i, k], m.clone()))
                        .collect::<Vec<_>>(),
                    other => vec![(vec![i], other.clone())],
                })
                .collect::<Vec<_>>(),
            _ => vec![],
        })?;
        for (idx, node) in slots {
            let Tree::Map(m) = node else { continue };
            let id = m.entries.iter().find(|e| e.key == "id").map(|e| &e.value);
            if let Some(Tree::Scalar {
                value: Value::Str(id), ..
            }) = id
            {
                let mut path = steps.clone();
                path.extend(idx.into_iter().map(PathSeg::Index));
                out.push((id.clone(), path));
            }
        }
    }
    Ok(out)
}

/// The path of the item `id` inside a chain document, or None.
///
/// # Errors
/// A document that does not parse.
pub fn chain_item_path(doc: &Document, id: &str) -> Res<Option<Vec<PathSeg>>> {
    Ok(chain_item_paths(doc)?
        .into_iter()
        .find(|(i, _)| i == id)
        .map(|(_, p)| p))
}

/// The k7x text a node file would hold for this chain node: what a reader of the module's text wants.
///
/// # Errors
/// An unknown node, an item the file no longer holds, or a file that does not read.
pub fn chain_item_text(fs: &dyn Fs, dir: &str, name: &str) -> Res<String> {
    let Some(r) = chain_item_of(fs, dir, name)? else {
        return fail(format!("no chain node '{name}' in this project"));
    };
    let doc = Document::new(&ws_text(fs, &r.path)?);
    let Some(at) = chain_item_path(&doc, &r.id)? else {
        let rel = r.path.strip_prefix(&format!("{dir}/")).unwrap_or(&r.path);
        return fail(format!("{rel} holds no item {}", r.id));
    };
    Ok(stringify(
        &doc.get_in(&at)?.unwrap_or(Value::Null),
        &StringifyOptions::default(),
    )?)
}

/// Run a node-file writer over a chain node's item and write the chain file back; the chain file's path.
///
/// # Errors
/// An unknown node, what `edit` fails with, or the write's.
pub fn edit_chain_item(
    fs: &mut dyn Fs,
    dir: &str,
    name: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
) -> Res<String> {
    let Some(r) = chain_item_of(fs, dir, name)? else {
        return fail(format!("no chain node '{name}' in this project"));
    };
    let before = ws_text(fs, &r.path)?;
    let after = splice_chain_item(&before, &r.id, edit)?;
    if after != before {
        ws_write_text(fs, &r.path, &after)?;
    }
    Ok(r.path)
}

fn unreadable(id: &str, text: &str) -> Option<Error> {
    let f = song::routing::decode_chain("chain", text, PathRule::HOST).err()?;
    Some(Error::Msg(
        format!("node {id}: the edit would make its chain unreadable — {}", f.message)
            .trim()
            .to_owned(),
    ))
}

/// The same edit on a chain document held in memory, refused when the chain would no longer read.
///
/// # Errors
/// An item the chain does not hold, what `edit` fails with, or a chain the edit would make unreadable.
pub fn edit_chain_item_in(doc: &mut Document, id: &str, edit: impl FnOnce(&mut Scope) -> Res<()>) -> Res<()> {
    let Some(at) = chain_item_path(doc, id)? else {
        return fail(format!("no item {id} in this chain"));
    };
    let under = |k: &str| {
        let mut p = at.clone();
        p.push(PathSeg::Key(k.to_owned()));
        p
    };
    let mut kept = vec![];
    for k in NODE_ITEM_KEYS {
        kept.push((k, doc.has_in(&under(k))?, doc.get_in(&under(k))?));
    }
    let before = doc.text().to_owned();
    edit(&mut doc.at(&at))?;
    if doc.text() == before {
        return Ok(());
    }
    // The use's own keys are put back as they were: a module edit never moves, bypasses or re-ids a node.
    for (k, has, value) in kept {
        let now = doc.get_in(&under(k))?;
        match (has, value) {
            (false, _) => {
                doc.delete_in(&under(k), false)?;
            }
            (true, Some(v)) if now.as_ref().is_none_or(|n| !same(n, &v)) => {
                doc.set_in(&under(k), &v, WriteOptions::default())?
            }
            _ => {}
        }
    }
    match unreadable(id, doc.text()) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// The pure half of `edit_chain_item`: chain file text in, chain file text out.
///
/// # Errors
/// What `edit_chain_item_in` fails with.
pub fn splice_chain_item(chain_text: &str, id: &str, edit: impl FnOnce(&mut Scope) -> Res<()>) -> Res<String> {
    let mut doc = Document::new(chain_text);
    edit_chain_item_in(&mut doc, id, edit)?;
    Ok(doc.text().to_owned())
}

/// The ids of a chain file's node items that hold events of their own: what a time edit has to reach.
#[must_use]
pub fn event_item_ids(chain_text: &str) -> Vec<String> {
    if song::routing::decode_chain("chain", chain_text, PathRule::HOST).is_err() {
        return vec![];
    }
    let doc = Document::new(chain_text);
    let Ok(items) = chain_item_paths(&doc) else {
        return vec![];
    };
    items
        .into_iter()
        .filter(|(_, path)| {
            let has = |k: &str| {
                let mut p = path.clone();
                p.push(PathSeg::Key(k.to_owned()));
                doc.has_in(&p).unwrap_or(false)
            };
            !has("track") && ["events", "alternatives", "takes"].iter().any(|k| has(k))
        })
        .map(|(id, _)| id)
        .collect()
}

/// Every chain file of a project, project-relative: none for a project of segments.
///
/// # Errors
/// A chains folder that does not list.
pub fn chain_files(fs: &dyn Fs, dir: &str) -> Res<Vec<String>> {
    if !is_chain_project(fs, dir) {
        return Ok(vec![]);
    }
    let d = join(&[dir, CHAINS_DIR]);
    let mut names: Vec<String> = fs
        .list(&d)
        .map_err(|e| io_error(&e, "scandir", &d))?
        .into_iter()
        .map(|e| e.name)
        .filter(|f| is_doc_file(f))
        .collect();
    names.sort_by_key(|n| n.encode_utf16().collect::<Vec<u16>>());
    Ok(names.into_iter().map(|f| format!("{CHAINS_DIR}/{f}")).collect())
}
