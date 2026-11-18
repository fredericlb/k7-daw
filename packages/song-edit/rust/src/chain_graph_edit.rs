// chain_graph_edit.rs — a graph edit on a project: `graph.k7x` through its document, or a chain project's chains as segments.
// A reader would get wrong that a node the edit no longer names is KEPT as a loose chain, unless the caller says it is dropped.

use crate::chain_data::{Item, id_of, lane_of, name_of, set_key};
use crate::chain_item_edit::{CHAINS_DIR, TRACKS_DIR, is_chain_project};
use crate::chain_project::{
    ChainProject, LAYOUT_FILE, node_of, raw_items, read_chain_project, read_layout, renamed, write_chains, write_layout,
};
use crate::chain_segments::{chain_segments_text, to_segments};
use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::from_segments::{FromSegments, GraphIn, from_segments};
use crate::fs::Fs;
use crate::graph_schema::{GRAPH_BASE, GRAPH_FILE, parse_graph};
use crate::loader::NodeResolver;
use crate::module_io::entity_file_for;
use crate::paths::join;
use crate::workspace_doc::{EditOpts, ws_edit, ws_text};
use indexmap::IndexMap;
use k7x::edit::{Document, Scope, StringifyOptions, Value, json_text, stringify};
use song::modules::IoKind;
use song::routing::chain_file::CHAIN_SINKS;
use std::cell::OnceCell;
use std::collections::HashSet;

const SHAPE_CATEGORY: &str = "shapes";
const VSTPRESETS_DIR: &str = "vstpresets";

fn bare(d: &str) -> &str {
    d.split('#').next().unwrap_or(d)
}

/// What a chain graph edit adds to and takes from the project beside the writer's own change.
#[derive(Clone, Debug, Default)]
pub struct ChainGraphEdit {
    /// Items the transform brings in that no chain holds yet, keyed by their in-memory name.
    pub add: IndexMap<String, Item>,
    /// Nodes the transform takes out that are to be DELETED rather than kept loose.
    pub drop: Vec<String>,
    /// A lane renamed by the transform: its item, chain, link levels and layout key follow.
    pub rename: Option<(String, String)>,
}

/// The chain project as the segments k7x text a `graph.k7x` would hold: what every graph writer reads.
///
/// # Errors
/// A project that does not read, or whose segments are too large.
pub fn chain_graph_text(fs: &dyn Fs, dir: &str) -> Res<String> {
    chain_segments_text(&read_chain_project(fs, dir)?, &read_layout(fs, dir)?)
}

/// The graph file a folder holds: a project's, or a library chain's, which is the same file.
fn graph_file_of(fs: &dyn Fs, dir: &str) -> Res<String> {
    match entity_file_for(fs, dir, "", GRAPH_BASE) {
        Some(f) => Ok(f.path),
        None => fail(format!("{dir}: no {GRAPH_FILE}")),
    }
}

/// The segments text a graph writer reads: the file, or a chain project's chains as segments.
///
/// # Errors
/// A folder with no graph file.
pub fn project_graph_text(fs: &dyn Fs, dir: &str) -> Res<String> {
    if is_chain_project(fs, dir) {
        return chain_graph_text(fs, dir);
    }
    ws_text(fs, &graph_file_of(fs, dir)?)
}

/// Run a `graph.k7x` writer on the file, or through a chain project's files; what was written.
///
/// # Errors
/// What the writer, the read or the write fails with.
pub fn edit_project_graph(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
) -> Res<String> {
    if is_chain_project(fs, dir) {
        edit_chain_graph(fs, env, dir, edit, &ChainGraphEdit::default())?;
        return Ok(join(&[dir, CHAINS_DIR]));
    }
    let path = graph_file_of(fs, dir)?;
    // The graph writers expand `runs` and put it back themselves, so they see the document as written.
    ws_edit(
        fs,
        &path,
        EditOpts {
            create: false,
            raw: true,
        },
        |doc| edit(&mut doc.at(&[])),
    )?;
    Ok(path)
}

/// A name a caller may use for a node: the node itself, or a chain holding that one node alone.
///
/// # Errors
/// A chain project that does not read.
pub fn chain_node_name(fs: &dyn Fs, dir: &str, name: &str) -> Res<String> {
    Ok(chain_node_name_in(&read_chain_project(fs, dir)?, name))
}

/// `chain_node_name` over a project already read.
#[must_use]
pub fn chain_node_name_in(cp: &ChainProject, name: &str) -> String {
    if cp.nodes.contains_key(name) || CHAIN_SINKS.contains(&bare(name)) {
        return name.to_owned();
    }
    let items: Vec<&Item> = cp
        .chains
        .get(bare(name))
        .map(|c| c.steps.iter().flat_map(|s| s.items()).collect())
        .unwrap_or_default();
    let [one] = items.as_slice() else {
        return name.to_owned();
    };
    let port = name.find('#').map_or("", |i| &name[i..]);
    format!("{}{port}", name_of(one))
}

/// A name a caller may use for a node; a chain project's one-node chain stands for that node.
///
/// # Errors
/// A chain project that does not read.
pub fn node_arg(fs: &dyn Fs, dir: &str, name: &str) -> Res<String> {
    if is_chain_project(fs, dir) {
        return chain_node_name(fs, dir, name);
    }
    Ok(name.to_owned())
}

fn swap(s: &str, from: &str, to: &str) -> String {
    if s == from {
        to.to_owned()
    } else if let Some(rest) = s.strip_prefix(&format!("{from}#")) {
        format!("{to}#{rest}")
    } else {
        s.to_owned()
    }
}

/// A new chain's name: a lane's own, else the leaf of what the head extends, free among the chains.
fn name_for(item_of: &dyn Fn(&str) -> Res<Item>, head: &str, taken: &HashSet<String>) -> String {
    let ext = item_of(head)
        .ok()
        .filter(|it| lane_of(it).is_none())
        .and_then(|it| match it.get("extends") {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        });
    let base = ext.map_or_else(|| head.to_owned(), |e| e.rsplit('/').next().unwrap_or(head).to_owned());
    let mut n = base.clone();
    let mut i = 2;
    while taken.contains(&n) {
        n = format!("{base}-{i}");
        i += 1;
    }
    n
}

/// Run a `graph.k7x` writer over a chain project's segments and write the result back: the files changed.
///
/// # Errors
/// Chains that do not read, what the writer fails with, or a result the graph schema refuses.
pub fn edit_chain_graph(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
    opts: &ChainGraphEdit,
) -> Res<Vec<String>> {
    let read = read_chain_project(fs, dir)?;
    if !read.issues.is_empty() {
        return fail(format!(
            "the chains do not read — fix them first:\n  {}",
            read.issues.join("\n  ")
        ));
    }
    let positions = read_layout(fs, dir)?;
    let (graph, levels) = to_segments(&read, &positions)?;
    let text = stringify(&graph, &StringifyOptions::default())?;
    let mut doc = Document::new(&text);
    edit(&mut doc.at(&[]))?;
    if doc.text() == text {
        return Ok(vec![]);
    }
    let parsed = parse_graph(doc.text());
    let Some(result) = parsed.value else {
        return fail(format!(
            "a graph edit went wrong: {}",
            parsed.issues.first().map_or("unreadable", String::as_str)
        ));
    };
    let (chains, cp, asleep) = {
        let fsr: &dyn Fs = &*fs;
        rebuilt(fsr, env, dir, &read, &graph, &result, &levels, opts)?
    };
    let mut written = write_chains(fs, dir, &chains, &cp.texts, &asleep)?;
    let pos = result.get("positions").cloned().unwrap_or(Value::Map(vec![]));
    if json_text(&pos) != json_text(&positions) {
        write_layout(fs, dir, &pos)?;
        written.push(LAYOUT_FILE.to_owned());
    }
    Ok(written)
}

type Rebuilt = (
    IndexMap<String, crate::chain_data::Chain>,
    ChainProject,
    HashSet<String>,
);

/// The chain files the edited segments make, the project they were made against, and the chains left asleep.
#[allow(clippy::too_many_arguments)]
fn rebuilt(
    fs: &dyn Fs,
    env: &Env,
    dir: &str,
    read: &ChainProject,
    shown_graph: &Value,
    result: &Value,
    levels: &IndexMap<String, f64>,
    opts: &ChainGraphEdit,
) -> Res<Rebuilt> {
    let cp = match &opts.rename {
        Some((from, to)) => renamed(read, from, to),
        None => read.clone(),
    };
    let lv: IndexMap<String, f64> = match &opts.rename {
        Some((from, to)) => levels
            .iter()
            .map(|(k, v)| {
                let mut it = k.split('>');
                let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                (format!("{}>{}", swap(a, from, to), swap(b, from, to)), *v)
            })
            .collect(),
        None => levels.clone(),
    };
    let raw = raw_items(&cp)?;
    let item_of = |n: &str| -> Res<Item> {
        let was = cp.nodes.get(n);
        let it = opts.add.get(n).cloned().or_else(|| {
            let was = was?;
            let mut it = raw.get(&was.id).cloned().unwrap_or_else(|| was.value.clone());
            if let Some((_, to)) = opts.rename.as_ref().filter(|(_, to)| to == n) {
                set_key(
                    &mut it,
                    if was.picture { "video_track" } else { "track" },
                    Value::Str(to.clone()),
                );
            }
            Some(it)
        });
        match it {
            Some(it) => Ok(it),
            None => fail(format!("'{n}' is not a node of this project")),
        }
    };
    let graph = GraphIn::from_value(result);
    let mut named: HashSet<&str> = graph.modules.keys().map(String::as_str).collect();
    named.extend(graph.modules.values().flatten().map(|d| bare(d)));
    let dropped: HashSet<&str> = opts.drop.iter().map(String::as_str).collect();
    // A chain the graph did not show and the edit does not name is left as it is: rewritten, its nodes would come back one loose chain each.
    let shown_in = GraphIn::from_value(shown_graph);
    let mut shown: HashSet<&str> = shown_in.modules.keys().map(String::as_str).collect();
    shown.extend(shown_in.modules.values().flatten().map(|d| bare(d)));
    let asleep: HashSet<String> = cp
        .chains
        .iter()
        .filter(|(_, c)| {
            let ns: Vec<&str> = c
                .step_lists()
                .flat_map(|(_, steps)| steps.iter().flat_map(|s| s.items().iter().map(name_of)))
                .collect();
            !ns.is_empty()
                && ns
                    .iter()
                    .all(|n| !shown.contains(n) && !named.contains(n) && !dropped.contains(n))
        })
        .map(|(name, _)| name.clone())
        .collect();
    let asleep_nodes: HashSet<&str> = cp
        .nodes
        .values()
        .filter(|n| asleep.contains(&n.chain))
        .map(|n| n.name.as_str())
        .collect();
    let loose: Vec<String> = cp
        .nodes
        .keys()
        .chain(opts.add.keys())
        .filter(|n| !named.contains(n.as_str()) && !dropped.contains(n.as_str()) && !asleep_nodes.contains(n.as_str()))
        .cloned()
        .collect();
    let resolver: OnceCell<Res<NodeResolver>> = OnceCell::new();
    let resolved = || -> Res<&NodeResolver> {
        match resolver.get_or_init(|| NodeResolver::load(fs, env, dir)) {
            Ok(r) => Ok(r),
            Err(e) => Err(e.clone()),
        }
    };
    // Whether a node with nothing feeding it starts a chain: a lane, a node that already did, or an input.
    let is_source = |n: &str| -> Res<bool> {
        if lane_of(&item_of(n)?).is_some() {
            return Ok(true);
        }
        if let Some(was) = cp.nodes.get(n)
            && was.sub.is_none()
            && was.step == 0
            && cp
                .chains
                .get(&was.chain)
                .is_some_and(|c| c.start == Some(crate::chain_data::Start::Own))
        {
            return Ok(true);
        }
        let Some(m) = resolved()?.resolve(n)? else {
            return Ok(false);
        };
        let io = matches!(
            m.authored.io,
            Some(IoKind::In | IoKind::File | IoKind::Camera | IoKind::Midi)
        );
        // A SHAPE is a picture source too, drawn rather than filmed.
        Ok(io || m.category.as_deref() == Some(SHAPE_CATEGORY))
    };
    // Whether a node is a picture pass, resolved once and only if asked: what a fade's stretch is made of.
    let is_pass = |n: &str| -> Res<bool> {
        let category = resolved()?.resolve(n)?.and_then(|m| m.category.clone());
        Ok(matches!(category.as_deref(), Some("shaders" | "video_effects")))
    };
    let is_picture_lane = |n: &str| cp.nodes.get(n).is_some_and(|x| x.picture);
    let namer = |head: &str, taken: &HashSet<String>| name_for(&item_of, head, taken);
    // A new chain takes neither its name nor a hidden chain's, whose files this leaves alone too.
    let reserved: Vec<String> = asleep.iter().cloned().chain(cp.hidden.keys().cloned()).collect();
    let chains = from_segments(
        &graph,
        &FromSegments {
            item_of: &item_of,
            is_source: &is_source,
            prev: Some(&cp),
            levels: &lv,
            name_for: Some(&namer),
            loose: &loose,
            reserved: &reserved,
            is_picture_pass: Some(&is_pass),
            is_picture_lane: Some(&is_picture_lane),
        },
    )?;
    Ok((chains, cp, asleep))
}

/// Rename a LANE: its file in `tracks/`, its item's `track`, and every trace of it in the chains.
///
/// # Errors
/// A lane file already at the new name, or what the graph edit fails with.
pub fn rename_chain_lane(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    from: &str,
    to: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
) -> Res<Vec<String>> {
    let src = join(&[dir, TRACKS_DIR, &format!("{from}.k7x")]);
    let dest = join(&[dir, TRACKS_DIR, &format!("{to}.k7x")]);
    if fs.exists(&dest) {
        return fail(format!("{TRACKS_DIR}/{to}.k7x already exists — refusing to overwrite"));
    }
    let opts = ChainGraphEdit {
        rename: Some((from.to_owned(), to.to_owned())),
        ..ChainGraphEdit::default()
    };
    let written = edit_chain_graph(fs, env, dir, edit, &opts)?;
    if fs.exists(&src) {
        fs.rename(&src, &dest).map_err(|e| io_error(&e, "rename", &src))?;
    }
    let mut out = vec![format!("{TRACKS_DIR}/{from}.k7x → {TRACKS_DIR}/{to}.k7x")];
    out.extend(written);
    Ok(out)
}

fn remove_file(fs: &mut dyn Fs, f: &str) -> Res<bool> {
    if !fs.exists(f) {
        return Ok(false);
    }
    fs.remove(f).map_err(|e| io_error(&e, "rm", f))?;
    Ok(true)
}

/// Delete a node from a chain project: its item, a lane's file, and its preset when nobody else names it.
///
/// # Errors
/// A node the project does not hold, or what the graph edit fails with.
pub fn delete_chain_item(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
) -> Res<Vec<String>> {
    let cp = read_chain_project(fs, dir)?;
    let n = node_of(&cp, node)?.clone();
    let opts = ChainGraphEdit {
        drop: vec![node.to_owned()],
        ..ChainGraphEdit::default()
    };
    edit_chain_graph(fs, env, dir, edit, &opts)?;
    let mut removed = vec![format!("{CHAINS_DIR}/{}.k7x: {node}", n.chain)];
    if let Some(track) = &n.track {
        let f = join(&[dir, TRACKS_DIR, &format!("{track}.k7x")]);
        if remove_file(fs, &f)? {
            removed.push(format!("{TRACKS_DIR}/{track}.k7x"));
        }
        return Ok(removed);
    }
    // Only the default-named state goes: a named `vstpreset:` may be shared on purpose.
    let own = format!("{}.vstpreset", id_of(&n.value));
    let preset = |it: &Item| match it.get("vstpreset") {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let named = preset(&n.value);
    let others = cp
        .nodes
        .values()
        .any(|o| o.name != node && preset(&o.value).as_deref() == Some(own.as_str()));
    let f = join(&[dir, VSTPRESETS_DIR, &own]);
    let ours = named.as_deref().is_none_or(|s| s.is_empty() || s == own);
    if ours && !others && remove_file(fs, &f)? {
        removed.push(format!("{VSTPRESETS_DIR}/{own}"));
    }
    Ok(removed)
}
