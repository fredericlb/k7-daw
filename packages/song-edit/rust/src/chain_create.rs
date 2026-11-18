// chain_create.rs — creating nodes and lanes in a chain project: a node is one item in a chain of its own, a lane its `tracks/` file plus a chain.
// A reader would get wrong that the name asked for names the CHAIN: the node is its fresh id, and nothing here wires anything.

use crate::chain_item_edit::{CHAINS_DIR, TRACKS_DIR, chain_file_path};
use crate::error::{Error, Res, fail, io_error};
use crate::fs::Fs;
use crate::ids::NewId;
use crate::paths::{join, relative};
use crate::workspace_doc::{ws_text, ws_write_text};
use k7x::edit::{Document, PathSeg, StringifyOptions, Tree, Value, WriteOptions, stringify};
use song::{Cache, PathRule, Source};

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

/// Whether `chains/<name>` is taken by a chain file.
#[must_use]
pub fn chain_taken(fs: &dyn Fs, dir: &str, name: &str) -> bool {
    fs.exists(&chain_file_path(dir, name))
}

/// A node file's k7x text as an item's data after a fresh id, and the `//` lines it opens with.
fn item_of(body: &str, new_id: NewId) -> Res<(String, Value, Vec<String>)> {
    let src = Document::new(body);
    let id = new_id();
    let mut fields = vec![("id".to_owned(), Value::Str(id.clone()))];
    if let Value::Map(m) = src.to_js()? {
        fields.extend(m);
    }
    Ok((id, Value::object(fields), src.header()))
}

fn write_text(fs: &mut dyn Fs, path: &str, text: &str) -> Res<()> {
    ws_write_text(fs, path, text)?;
    Ok(())
}

fn mkdirs(fs: &mut dyn Fs, dir: &str) -> Res<()> {
    fs.mkdir_all(dir).map_err(|e| io_error(&e, "mkdir", dir))
}

/// Write a node, given as a node file's k7x text, as the one item of a new loose chain `chain`; its id and file.
///
/// # Errors
/// A chain already there, a body that does not read, or the file system's.
pub fn create_chain_node(fs: &mut dyn Fs, dir: &str, chain: &str, body: &str, new_id: NewId) -> Res<(String, String)> {
    let rel = format!("{CHAINS_DIR}/{chain}.k7x");
    if chain_taken(fs, dir, chain) {
        return fail(format!("{rel} already exists — refusing to overwrite"));
    }
    let (id, item, header) = item_of(body, new_id)?;
    mkdirs(fs, &join(&[dir, CHAINS_DIR]))?;
    let text = stringify(
        &Value::object(vec![("steps".to_owned(), Value::List(vec![item]))]),
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )?;
    write_text(fs, &join(&[dir, &rel]), &text)?;
    Ok((id, rel))
}

/// A new lane: `tracks/<lane>.k7x` holding `body` (k7x text), and the `start: true` chain that plays it; the two files.
///
/// # Errors
/// A lane or chain already there, a body that does not read, or the file system's.
pub fn create_chain_lane(fs: &mut dyn Fs, dir: &str, lane: &str, body: &str, new_id: NewId) -> Res<Vec<String>> {
    let track = format!("{TRACKS_DIR}/{lane}.k7x");
    if fs.exists(&join(&[dir, &track])) {
        return fail(format!("track '{lane}' already exists ({track})"));
    }
    let chain = format!("{CHAINS_DIR}/{lane}.k7x");
    if chain_taken(fs, dir, lane) {
        return fail(format!("{chain} already exists — a lane's chain is named after it"));
    }
    mkdirs(fs, &join(&[dir, TRACKS_DIR]))?;
    mkdirs(fs, &join(&[dir, CHAINS_DIR]))?;
    write_text(fs, &join(&[dir, &track]), body)?;
    let step = Value::object(vec![
        ("id".to_owned(), Value::Str(new_id())),
        ("track".to_owned(), Value::Str(lane.to_owned())),
    ]);
    let text = stringify(
        &Value::object(vec![
            ("start".to_owned(), Value::Bool(true)),
            ("steps".to_owned(), Value::List(vec![step])),
        ]),
        &StringifyOptions::default(),
    )?;
    write_text(fs, &join(&[dir, &chain]), &text)?;
    Ok(vec![track, chain])
}

/// Put an input, given as a node file's k7x text, in the lane's chain just ahead of the lane; its id and file.
///
/// # Errors
/// A lane that is not alone in a chain step, a chain that would no longer read, or the file system's.
pub fn insert_chain_input(fs: &mut dyn Fs, dir: &str, lane: &str, body: &str, new_id: NewId) -> Res<(String, String)> {
    let source: &dyn Source = fs;
    let cp =
        song::routing::read_chain_project(source, &mut Cache::new(), dir).map_err(|e| io_error(&e, "scandir", dir))?;
    let Some(at) = cp.nodes.get(lane).filter(|n| n.track.is_some()) else {
        return fail(format!("no lane '{lane}' in a chain"));
    };
    if at.sub.is_some() || at.item.is_some() {
        return fail(format!(
            "'{lane}' sits inside a subchain or a parallel step — wire its input by hand"
        ));
    }
    let (chain, step) = (at.chain.clone(), at.step);
    let path = chain_file_path(dir, &chain);
    let mut doc = Document::new(&ws_text(fs, &path)?);
    let steps_start = doc.node_at(&[key("steps")], |n| match n {
        Some(Tree::List { .. }) => n.map(Tree::start),
        _ => None,
    })?;
    let Some(steps_start) = steps_start else {
        return fail(format!("{chain} has no steps"));
    };
    let (_, item, header) = item_of(body, new_id)?;
    let id = match item.get("id") {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    doc.insert_in(&[key("steps")], step, &item, WriteOptions::default())?;
    let at_step = [key("steps"), PathSeg::Index(step)];
    let added = doc.node_at(&at_step, |n| n.map(Tree::start))?;
    // The body's own description goes above the item, when the item has a line of its own to sit under it.
    let own = added.is_some_and(|s| doc.text()[..s].rfind('\n').is_some_and(|nl| nl > steps_start));
    if !header.is_empty() && own {
        let col = doc.position_of(&at_step)?.map_or(1, |(_, c)| c);
        let pad = " ".repeat(col - 1);
        let lead = header
            .iter()
            .map(|l| format!("// {l}").trim_end().to_owned())
            .collect::<Vec<_>>()
            .join(&format!("\n{pad}"));
        let src = doc.get_source(&at_step)?.unwrap_or_default();
        doc.replace_source(&at_step, &format!("{lead}\n{pad}{src}"))?;
    }
    let out = doc.text().to_owned();
    if let Err(f) = song::routing::decode_chain("chain", &out, PathRule::HOST) {
        return Err(Error::Msg(
            format!("the input would make {chain} unreadable — {}", f.message)
                .trim()
                .to_owned(),
        ));
    }
    write_text(fs, &path, &out)?;
    Ok((id, relative(dir, &path)))
}
