// graph_wire.rs — the graph's wiring, layout and boxes: pin, unlink, organize, move, chain connect/disconnect/replug, assemblies, after verbs/graph.ts.
// A reader would get wrong where a pin lands: graph.k7x's `positions:`, or a chain project's `layout.k7x`, the same writer either way.

use super::graph_read::{GraphView, read_graph, take_spawn_at};
use super::shared::{Done, join, split_words};
use super::track_group::{carries_input, feeds};
use crate::host::Host;
use k7x::edit::js_number;
use song::lanes::is_builtin_output;
use song_edit::assembly_edit::{assembly_members, create_assembly, dissolve_assembly};
use song_edit::chain_graph_edit::chain_node_name;
use song_edit::graph_edit::{
    clear_chain_node_positions, connect_chain_nodes, delete_chain_node, disconnect_chain_nodes, replug_chain_nodes,
    set_chain_node_position, set_chain_node_positions, unlink_chain_node,
};
use song_edit::js_text::{js_trim, str_to_number};
use song_edit::paths::relative;
use song_edit::start_time::js_round;

const OUT_NODE: &str = "OUT";

fn words(a: &str) -> Vec<String> {
    split_words(a).into_iter().map(str::to_owned).collect()
}

fn base(s: &str) -> &str {
    s.split('#').next().unwrap_or("")
}

/// `chainNodeName` over each word: a chain project's node answers to the name `node new` gave it.
fn node_names(h: &Host, parts: &[String]) -> Vec<String> {
    parts
        .iter()
        .map(|p| chain_node_name(&*h.fs, &h.song_path, p).unwrap_or_else(|_| p.clone()))
        .collect()
}

/// The file a writer touched, as the log names it.
fn rel(h: &Host, path: &str) -> String {
    relative(&h.song_path, path)
}

/// `graph pin <JSON {node: {x,y}}>` — where the graph drew itself, in one write.
pub fn graph_pin(a: &str, h: &mut Host) -> Done {
    let payload = js_trim(a);
    if payload.is_empty() {
        h.refuse([
            r#"usage: graph pin <JSON {node: {x,y}}>  (e.g. graph pin {"reverb":{"x":420,"y":180}})"#.to_owned(),
        ]);
        return Ok(());
    }
    let raw: serde_json::Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad positions JSON: {e}")]);
            return Ok(());
        }
    };
    let Some(map) = raw.as_object() else {
        h.refuse(["positions payload must be a JSON object of node -> {x,y}".to_owned()]);
        return Ok(());
    };
    let num = |v: Option<&serde_json::Value>| match v {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) => Some(str_to_number(s)).filter(|n| n.is_finite()),
        _ => None,
    };
    let at: Vec<(String, (f64, f64))> = map
        .iter()
        .filter(|(node, _)| !node.is_empty())
        .filter_map(|(node, v)| Some((node.clone(), (num(v.get("x"))?, num(v.get("y"))?))))
        .collect();
    if at.is_empty() {
        h.refuse(["no usable {x,y} in the payload".to_owned()]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match set_chain_node_positions(h.fs, &h.env, &dir, &at) {
        Ok(path) => h.log([format!("pinned {} node(s) ({})", at.len(), rel(h, &path))]),
        Err(e) => h.refuse([format!("graph pin: {e}")]),
    }
    Ok(())
}

/// `assembly new <template-ref> <name> [x,y]` — a templated box of nodes, wiring and all.
pub fn assembly_new(a: &str, h: &mut Host) -> Done {
    let mut parts = words(a);
    let at = take_spawn_at(&mut parts);
    let [r, name] = parts.as_slice() else {
        h.refuse(["usage: assembly new <template-ref> <name> [x,y]".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match create_assembly(h.fs, &h.env, &dir, r, name, at) {
        Ok((file, members)) => {
            h.log([format!("created {file} — {} nodes, drawn as one box", members.len())]);
            h.reload();
        }
        Err(e) => h.refuse([format!("assembly new failed: {e}")]),
    }
    Ok(())
}

/// `assembly delete <name>` — the record AND every node it holds, each spliced out like `node delete`.
pub fn assembly_delete(a: &str, h: &mut Host) -> Done {
    let Some(name) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: assembly delete <name>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let members = match assembly_members(&*h.fs, &dir, &name) {
        Ok(m) => m,
        Err(e) => {
            h.refuse([format!("assembly delete failed: {e}")]);
            return Ok(());
        }
    };
    let file = match dissolve_assembly(h.fs, &dir, &name) {
        Ok(f) => f,
        Err(e) => {
            h.refuse([format!("assembly delete failed: {e}")]);
            return Ok(());
        }
    };
    let mut removed = 0;
    for node in &members {
        // A member with no file left is one somebody already deleted: not a reason to stop halfway.
        if let Ok((_, gone)) = delete_chain_node(h.fs, &h.env, &dir, node) {
            removed += gone.len();
        }
    }
    h.log([format!("deleted {file} and {removed} of its files")]);
    h.reload();
    Ok(())
}

/// `assembly dissolve <name>` — the record only; every node stays on the graph, wired as it was.
pub fn assembly_dissolve(a: &str, h: &mut Host) -> Done {
    let Some(name) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: assembly dissolve <name>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let done =
        assembly_members(&*h.fs, &dir, &name).and_then(|m| dissolve_assembly(h.fs, &dir, &name).map(|f| (m.len(), f)));
    match done {
        Ok((n, file)) => {
            h.log([format!("dropped {file} — its {n} nodes are on the graph, untouched")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("assembly dissolve failed: {e}")]),
    }
    Ok(())
}

/// `graph unlink <node>` — out of the signal path, files untouched, so `node insert` puts it back.
pub fn graph_unlink(a: &str, h: &mut Host) -> Done {
    let Some(node) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: graph unlink <node>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match unlink_chain_node(h.fs, &h.env, &dir, &node) {
        Ok(rewired) => {
            let how = if rewired.is_empty() {
                " — nothing fed it, so there was nothing to rejoin".to_owned()
            } else {
                format!(" and rejoined what fed it: {}", rewired.join(", "))
            };
            h.log([format!("unlinked {node}{how}; it stays in the graph, wired to nothing")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("graph unlink failed: {e}")]),
    }
    Ok(())
}

/// `graph organize` — every node handed back to the layout.
pub fn graph_organize(_a: &str, h: &mut Host) -> Done {
    let dir = h.song_path.clone();
    match clear_chain_node_positions(h.fs, &h.env, &dir) {
        Ok(_) => {
            h.log(["graph reorganized — every node handed back to the layout".to_owned()]);
            h.reload();
        }
        Err(e) => h.refuse([format!("graph organize failed: {e}")]),
    }
    Ok(())
}

/// The curves' ids, which are cards on the graph too; a folder that does not list is none.
fn curve_ids(h: &Host) -> Vec<String> {
    let dir = join(&h.song_path, "curves");
    h.fs.list(&dir)
        .map(|l| {
            l.into_iter()
                .filter_map(|e| e.name.strip_suffix(".k7x").map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// `graph move <node> <x> <y>`, or `graph move <node> auto` to hand it back to the layout.
pub fn graph_move(a: &str, h: &mut Host) -> Done {
    let usage = "usage: graph move <node> <x> <y> (world px), or `graph move <node> auto`";
    let parts = words(a);
    if parts.len() < 2 {
        h.refuse([usage.to_owned()]);
        return Ok(());
    }
    let g = match read_graph(h) {
        Ok(g) => g,
        Err(e) => {
            h.refuse([format!("graph move: {}", e.0)]);
            return Ok(());
        }
    };
    let name = &parts[0];
    let known = is_builtin_output(name)
        || g.is_module(name)
        || g.is_track(name)
        || g.is_video_track(name)
        || g.is_assembly(name)
        || curve_ids(h).contains(name);
    if !known {
        h.refuse([format!("no node \"{name}\" in this project ({})", g.known_nodes())]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let line = if parts[1].eq_ignore_ascii_case("auto") {
        set_chain_node_position(h.fs, &h.env, &dir, name, None).map(|_| format!("{name}: back to the layout"))
    } else {
        let x = str_to_number(&parts[1]);
        let y = parts.get(2).map_or(f64::NAN, |y| str_to_number(y));
        if !x.is_finite() || !y.is_finite() {
            h.refuse([usage.to_owned()]);
            return Ok(());
        }
        set_chain_node_position(h.fs, &h.env, &dir, name, Some((x, y))).map(|_| {
            format!(
                "{name}: pinned at {},{}",
                js_number(js_round(x)),
                js_number(js_round(y))
            )
        })
    };
    match line {
        Ok(l) => {
            h.log([l]);
            h.reload();
        }
        Err(e) => h.refuse([format!("graph move failed: {e}")]),
    }
    Ok(())
}

/// The refusal for an end the graph does not name, or None when both are nodes.
fn unknown_end(g: &GraphView, ends: &[&String]) -> Option<String> {
    ends.iter()
        .map(|e| base(e))
        .find(|e| !g.names_a_node(e))
        .map(|e| format!("no node \"{e}\" in this project ({})", g.known_nodes()))
}

/// `graph chain connect <from> <to>` — the patch-cable gesture; either end may carry `#port`.
pub fn chain_connect(a: &str, h: &mut Host) -> Done {
    let parts = words(a);
    if parts.len() != 2 {
        h.refuse(["usage: graph chain connect <from> <to> — each a node, optionally name#port (e.g. graph chain connect gtr reverb)".to_owned()]);
        return Ok(());
    }
    let ends = node_names(h, &parts);
    let (from, to) = (&ends[0], &ends[1]);
    if base(from) == base(to) {
        h.refuse([format!("can't connect '{}' to itself", base(from))]);
        return Ok(());
    }
    let g = match read_graph(h) {
        Ok(g) => g,
        Err(e) => {
            h.refuse([format!("graph chain connect: {}", e.0)]);
            return Ok(());
        }
    };
    if base(from) == OUT_NODE {
        h.refuse(["OUT is the sink — it has no output to connect from".to_owned()]);
        return Ok(());
    }
    if g.is_track(base(to)) && !feeds(h).is_ok_and(|f| carries_input(&f, base(from))) {
        h.refuse([format!(
            "'{}' is a track — only a live input's signal lands on a lane",
            base(to)
        )]);
        return Ok(());
    }
    if let Some(why) = unknown_end(&g, &[from, to]) {
        h.refuse([why]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match connect_chain_nodes(h.fs, &h.env, &dir, from, to) {
        Ok(_) => {
            h.log([format!("connected {from} → {to}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("graph chain connect failed: {e}")]),
    }
    Ok(())
}

/// `graph chain disconnect <from> <to>` — one cable out; the writer's own refusal is the check.
pub fn chain_disconnect(a: &str, h: &mut Host) -> Done {
    let parts = words(a);
    if parts.len() != 2 {
        h.refuse(["usage: graph chain disconnect <from> <to> — each a node, optionally name#port".to_owned()]);
        return Ok(());
    }
    let ends = node_names(h, &parts);
    let dir = h.song_path.clone();
    match disconnect_chain_nodes(h.fs, &h.env, &dir, &ends[0], &ends[1]) {
        Ok(_) => {
            h.log([format!("disconnected {} → {}", ends[0], ends[1])]);
            h.reload();
        }
        Err(e) => h.refuse([format!("graph chain disconnect failed: {e}")]),
    }
    Ok(())
}

/// `graph chain replug <from> <to> <newFrom> <newTo>` — one end of an arrow moved in one write.
pub fn chain_replug(a: &str, h: &mut Host) -> Done {
    let parts = words(a);
    let [from, to, new_from, new_to] = parts.as_slice() else {
        h.refuse([
            "usage: graph chain replug <from> <to> <newFrom> <newTo> — the arrow from→to becomes newFrom→newTo"
                .to_owned(),
        ]);
        return Ok(());
    };
    if from == new_from && to == new_to {
        h.refuse(["nothing moved — the cable is where it was".to_owned()]);
        return Ok(());
    }
    if base(new_from) == base(new_to) {
        h.refuse([format!("can't connect '{}' to itself", base(new_from))]);
        return Ok(());
    }
    let g = match read_graph(h) {
        Ok(g) => g,
        Err(e) => {
            h.refuse([format!("graph chain replug: {}", e.0)]);
            return Ok(());
        }
    };
    if base(new_from) == OUT_NODE {
        h.refuse(["OUT is the sink — it has no output to connect from".to_owned()]);
        return Ok(());
    }
    if g.is_track(base(new_to)) {
        h.refuse([format!("'{}' is a track — a source; nothing can feed it", base(new_to))]);
        return Ok(());
    }
    if let Some(why) = unknown_end(&g, &[new_from, new_to]) {
        h.refuse([why]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match replug_chain_nodes(h.fs, &h.env, &dir, [from, to, new_from, new_to]) {
        Ok(_) => {
            h.log([format!("replugged: {from} → {to} is now {new_from} → {new_to}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("graph chain replug failed: {e}")]),
    }
    Ok(())
}
