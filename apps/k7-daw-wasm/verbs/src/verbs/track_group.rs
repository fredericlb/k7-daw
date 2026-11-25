// track_group.rs — `track group`, `track hide|unhide` and `track input`, after trackGroup.ts, trackHide.ts and trackInput.ts.
// A reader would get wrong group membership: the group's `tracks:` is its one record, so a track joining one leaves the other.

use super::shared::{Done, free_name, join, split_words};
use crate::host::{Host, Thrown};
use k7x::edit::{PathSeg, StringifyOptions, Value, stringify};
use song::Cache;
use song::graph::load_graph;
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::modules::{IoKind, ModuleKind};
use song::routing::split_port;
use song_edit::chain_create::{create_chain_lane, insert_chain_input};
use song_edit::chain_data::is_track_item;
use song_edit::chain_item_edit::{chain_file_path, is_chain_project};
use song_edit::chain_project::read_chain_project;
use song_edit::graph_edit::{connect_chain_nodes, place_chain_node};
use song_edit::js_text::js_string;
use song_edit::loader::{load_modules, roots_for};
use song_edit::module_io::entity_file_for;
use song_edit::project_files::{create_node, free_node_name};
use song_edit::project_root::load_project_root;
use song_edit::workspace_doc::ws_write_text;
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::{delete_file_value, set_file_value};
use std::collections::HashMap;

const GROUP_TRACK_REF: &str = "@meta/group-track";

fn key(k: &str) -> Vec<PathSeg> {
    vec![PathSeg::Key(k.to_owned())]
}

fn str_list(items: &[String]) -> Value {
    Value::List(items.iter().cloned().map(Value::Str).collect())
}

/// A group lane's text: the module it is and its members, on one line.
fn group_body(members: &[String]) -> Result<String, Thrown> {
    let body = Value::Map(vec![
        ("extends".to_owned(), Value::Str(GROUP_TRACK_REF.to_owned())),
        ("tracks".to_owned(), str_list(members)),
    ]);
    let inline = |p: &[PathSeg], _: &Value| matches!(p.first(), Some(PathSeg::Key(k)) if k == "tracks");
    stringify(
        &body,
        &StringifyOptions {
            header: &[],
            inline: Some(&inline),
        },
    )
    .map_err(|e| Thrown(e.to_string()))
}

/// `createGroupFile`: the group's file where a lane's goes, its own chain in a chain project.
fn create_group_file(h: &mut Host, id: &str, members: &[String]) -> Result<Vec<String>, String> {
    if !super::words::is_name(id) {
        return Err(format!(
            "invalid group name '{id}' — letters, digits, '.', '_' and '-' only"
        ));
    }
    let dir = h.song_path.clone();
    let body = group_body(members).map_err(|e| e.0)?;
    if is_chain_project(&*h.fs, &dir) {
        return create_chain_lane(h.fs, &dir, id, &body, h.new_id).map_err(|e| e.to_string());
    }
    let rel = format!("nodes/{id}.k7x");
    if ws_exists(&*h.fs, &join(&dir, &rel)) {
        return Err(format!("a lane '{id}' already exists ({rel})"));
    }
    h.fs.mkdir_all(&join(&dir, "nodes")).map_err(|e| e.to_string())?;
    ws_write_text(h.fs, &join(&dir, &rel), &body).map_err(|e| e.to_string())?;
    Ok(vec![rel])
}

/// `setGroupMembers`: an existing group's members stated whole.
fn set_group_members(h: &mut Host, id: &str, members: &[String]) -> Result<String, String> {
    let dir = h.song_path.clone();
    let file = entity_file_for(&*h.fs, &join(&dir, "tracks"), "tracks", id)
        .or_else(|| entity_file_for(&*h.fs, &join(&dir, "nodes"), "nodes", id))
        .ok_or_else(|| format!("no file for group '{id}'"))?;
    set_file_value(h.fs, &file.path, &key("tracks"), &str_list(members)).map_err(|e| e.to_string())?;
    Ok(file.rel)
}

/// `track group <tracks, comma-joined> [name]` — a new group lane holding them.
pub fn track_group(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let Some(list) = w.first().copied() else {
        h.refuse(["usage: track group <tracks, comma-joined> [name]".to_owned()]);
        return Ok(());
    };
    let p = h.project()?;
    let mut members: Vec<String> = Vec::new();
    for want in list
        .split(',')
        .map(song_edit::js_text::js_trim)
        .filter(|s| !s.is_empty())
    {
        if p.groups.iter().any(|g| g.id == want) {
            h.refuse([format!("'{want}' is a group — a group holds tracks, not groups")]);
            return Ok(());
        }
        let Some(t) = p
            .tracks
            .iter()
            .find(|x| x.id == want || x.name == want || split_port(&x.id).0 == want)
        else {
            h.refuse([format!("no track '{want}'")]);
            return Ok(());
        };
        let lane = split_port(&t.id).0.to_owned();
        if !members.contains(&lane) {
            members.push(lane);
        }
    }
    let id = match w.get(1) {
        Some(n) => (*n).to_owned(),
        None => {
            let taken: Vec<String> = p
                .tracks
                .iter()
                .map(|t| split_port(&t.id).0.to_owned())
                .chain(p.groups.iter().map(|g| g.id.clone()))
                .collect();
            free_name("group", &|n| taken.iter().any(|t| t == n), false, 1)
        }
    };
    let mut left: Vec<(String, Vec<String>)> = Vec::new();
    for t in p.tracks.iter().filter(|t| !t.group.is_empty() && !t.generated) {
        let lane = split_port(&t.id).0.to_owned();
        let at = match left.iter().position(|(g, _)| *g == t.group) {
            Some(i) => i,
            None => {
                left.push((t.group.clone(), Vec::new()));
                left.len() - 1
            }
        };
        if !members.contains(&lane) && !left[at].1.contains(&lane) {
            left[at].1.push(lane);
        }
    }
    let mut touched: Vec<String> = Vec::new();
    for t in p.tracks.iter().filter(|t| !t.group.is_empty()) {
        if members.iter().any(|m| m == split_port(&t.id).0) && !touched.contains(&t.group) {
            touched.push(t.group.clone());
        }
    }
    let mut edited = Vec::new();
    for g in &touched {
        let rest = left
            .iter()
            .find(|(x, _)| x == g)
            .map(|(_, r)| r.clone())
            .unwrap_or_default();
        match set_group_members(h, g, &rest) {
            Ok(rel) => edited.push(rel),
            Err(e) => {
                h.refuse([format!("track group: {e}")]);
                return Ok(());
            }
        }
    }
    let written = match create_group_file(h, &id, &members) {
        Ok(w) => w,
        Err(e) => {
            h.refuse([format!("track group: {e}")]);
            return Ok(());
        }
    };
    h.reload();
    let mut lines = vec![format!("+ group {id}: {} ({})", members.join(", "), written.join(", "))];
    if !edited.is_empty() {
        lines.push(format!("  out of {} ({})", touched.join(", "), edited.join(", ")));
    }
    h.log(lines);
    Ok(())
}

fn hide(a: &str, h: &mut Host, on: bool) -> Done {
    let verb = if on { "track hide" } else { "track unhide" };
    let Some(id_arg) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse([format!("usage: {verb} <track>")]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    if !is_chain_project(&*h.fs, &dir) {
        h.refuse([format!(
            "{verb}: a track is hidden by its chain, and only a chain project has chain files"
        )]);
        return Ok(());
    }
    let lane = split_port(&id_arg).0.to_owned();
    let cp = read_chain_project(&*h.fs, &dir)?;
    let holds = |c: &song_edit::chain_data::Chain| {
        c.step_lists().any(|(_, steps)| {
            steps.iter().any(|s| {
                s.items()
                    .iter()
                    .any(|i| is_track_item(i) && matches!(i.get("track"), Some(Value::Str(t)) if *t == lane))
            })
        })
    };
    let (here, there) = if on {
        (&cp.chains, &cp.hidden)
    } else {
        (&cp.hidden, &cp.chains)
    };
    let Some(chain) = here.iter().find(|(_, c)| holds(c)).map(|(n, _)| n.clone()) else {
        let elsewhere = there.values().any(holds);
        h.refuse([if elsewhere {
            format!("\"{lane}\" is already {}", if on { "hidden" } else { "shown" })
        } else {
            format!("no track \"{id_arg}\" in this project")
        }]);
        return Ok(());
    };
    if let Err(e) = write_hidden(h, &chain, &lane, on) {
        h.refuse([format!("{verb}: {e}")]);
    }
    Ok(())
}

fn write_hidden(h: &mut Host, chain: &str, lane: &str, on: bool) -> Result<(), song_edit::Error> {
    let dir = h.song_path.clone();
    let path = chain_file_path(&dir, chain);
    if on {
        set_file_value(h.fs, &path, &key("hidden"), &Value::Bool(true))?;
    } else {
        delete_file_value(h.fs, &path, &key("hidden"))?;
    }
    let mut notes = vec![format!("chains/{chain}.k7x")];
    if on {
        let root = load_project_root(&*h.fs, &dir, false)?;
        let soloed: Vec<String> = match root.data.get("soloed") {
            Some(Value::List(l)) => l.iter().map(js_string).collect(),
            _ => Vec::new(),
        };
        if let Some(project) = entity_file_for(&*h.fs, &dir, "", "project")
            && soloed.iter().any(|s| s == lane)
        {
            let next: Vec<String> = soloed.into_iter().filter(|s| s != lane).collect();
            if next.is_empty() {
                delete_file_value(h.fs, &project.path, &key("soloed"))?;
            } else {
                set_file_value(h.fs, &project.path, &key("soloed"), &str_list(&next))?;
            }
            notes.push(format!("unsoloed in {}", project.rel.trim_start_matches('/')));
        }
    }
    h.reload();
    h.log([if on {
        format!(
            "{lane} hidden ({}) — `track unhide {lane}` brings it back",
            notes.join(", ")
        )
    } else {
        format!("{lane} shown again ({})", notes.join(", "))
    }]);
    Ok(())
}

/// `track hide <track>` — the lane's chain set aside, files kept; a hidden lane leaves `soloed`.
pub fn track_hide(a: &str, h: &mut Host) -> Done {
    hide(a, h, true)
}

/// `track unhide <track>` — the chain shown again.
pub fn track_unhide(a: &str, h: &mut Host) -> Done {
    hide(a, h, false)
}

/// The lanes and modules `runCarriesInput` reads: who feeds whom, and which nodes are live inputs.
pub(super) struct Feeds {
    lanes: Vec<String>,
    live: Vec<String>,
    feeders: HashMap<String, Vec<String>>,
}

pub(super) fn feeds(h: &Host) -> Result<Feeds, Thrown> {
    let roots = roots_for(&*h.fs, &h.env, &h.song_path);
    let mut cache = Cache::new();
    let inputs = load_inputs(&*h.fs, &mut cache, &roots, Rig::default()).map_err(|e| Thrown(e.to_string()))?;
    let g = load_graph(&*h.fs, &mut cache, &roots, &inputs).map_err(|e| match e {
        song::graph::GraphError::Source(s) => Thrown(s.to_string()),
        song::graph::GraphError::Refused(r) => Thrown(
            r.findings
                .first()
                .map_or_else(|| "the graph does not load".to_owned(), |f| f.message.clone()),
        ),
    })?;
    let bare = |s: &str| s.split('#').next().unwrap_or("").to_owned();
    let mut feeders: HashMap<String, Vec<String>> = HashMap::new();
    let mut link = |from: &str, to: &str| feeders.entry(bare(to)).or_default().push(bare(from));
    for c in g.chains.values() {
        let run: Vec<&str> = c
            .from
            .iter()
            .map(String::as_str)
            .chain(c.use_steps.iter().map(|s| s.module.as_str()))
            .collect();
        for pair in run.windows(2) {
            link(pair[0], pair[1]);
        }
        if let Some(last) = run.last() {
            for o in &c.out {
                link(last, &o.to);
            }
        }
    }
    let live = g
        .modules
        .iter()
        .filter(|(_, m)| m.kind == ModuleKind::Io && matches!(m.io, Some(IoKind::In | IoKind::Midi)))
        .map(|(k, _)| (*k).to_owned())
        .collect();
    Ok(Feeds {
        lanes: g.tracks.keys().map(|k| (*k).to_owned()).collect(),
        live,
        feeders,
    })
}

/// `runCarriesInput`: does the signal arriving at `node` come off a live input? Walks upstream, cycles allowed.
pub(super) fn carries_input(f: &Feeds, node: &str) -> bool {
    let mut seen: Vec<String> = Vec::new();
    let mut stack = vec![node.split('#').next().unwrap_or("").to_owned()];
    while let Some(n) = stack.pop() {
        if seen.contains(&n) {
            continue;
        }
        if f.live.contains(&n) {
            return true;
        }
        stack.extend(f.feeders.get(&n).cloned().unwrap_or_default());
        seen.push(n);
    }
    false
}

/// `track input <track> audio|midi` — an input node feeding the lane.
pub fn track_input(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(lane), Some(kind)) = (
        w.first().copied(),
        w.get(1).copied().filter(|k| matches!(*k, "audio" | "midi")),
    ) else {
        h.refuse(["usage: track input <track> audio|midi".to_owned()]);
        return Ok(());
    };
    let f = match feeds(h) {
        Ok(f) => f,
        Err(e) => {
            h.refuse([format!("track input: {}", e.0)]);
            return Ok(());
        }
    };
    if !f.lanes.iter().any(|l| l == lane) {
        h.refuse([format!("no track \"{lane}\" in this project")]);
        return Ok(());
    }
    if carries_input(&f, lane) {
        h.refuse([format!("{lane} already has an input")]);
        return Ok(());
    }
    if let Err(e) = add_input(h, lane, kind) {
        h.refuse([format!("track input: {e}")]);
        return Ok(());
    }
    h.reload();
    Ok(())
}

fn add_input(h: &mut Host, lane: &str, kind: &str) -> Result<(), song_edit::Error> {
    let dir = h.song_path.clone();
    let midi = kind == "midi";
    if is_chain_project(&*h.fs, &dir) {
        let mut body = vec![(
            "extends".to_owned(),
            Value::Str(if midi { "@meta/MIDI" } else { "@meta/IN" }.to_owned()),
        )];
        if !midi {
            body.push(("monitor".to_owned(), Value::Str("auto".to_owned())));
        }
        let text = stringify(&Value::Map(body), &StringifyOptions::default())?;
        let (_, rel) = insert_chain_input(h.fs, &dir, lane, &text, h.new_id)?;
        h.log([format!("{lane}: {kind} input added ahead of it ({rel})")]);
        return Ok(());
    }
    let node = free_node_name(
        &*h.fs,
        &h.env,
        &dir,
        if midi { "midi_in" } else { "audio_in" },
        Some(lane),
    );
    let catalogue = load_modules(&*h.fs, &h.env, &dir)?;
    create_node(
        h.fs,
        &h.env,
        &dir,
        "io",
        if midi { "MIDI" } else { "IN" },
        &node,
        &catalogue,
        h.new_id,
    )?;
    place_chain_node(h.fs, &h.env, &dir, &node)?;
    connect_chain_nodes(h.fs, &h.env, &dir, &node, lane)?;
    h.log([format!("{lane}: {kind} input {node} wired to it")]);
    Ok(())
}
