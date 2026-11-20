// graph_edit.rs — the graph verbs: each a `graph.k7x` writer run on the file, or on a chain project's chains read as segments.
// A reader would get wrong that renaming a node in a chain project is a no-op for anything but a lane: its name is its id.

use crate::chain_graph_edit::{delete_chain_item, edit_project_graph, node_arg, project_graph_text, rename_chain_lane};
use crate::chain_item_edit::is_chain_project;
use crate::chain_project::read_chain_project;
use crate::confine::Env;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::graph_file::{
    add_node, clear_node_positions, connect_nodes, disconnect_nodes, insert_between, remove_node, rename_node,
    set_module_enabled, set_node_position, set_node_positions, unlink_node,
};
use crate::graph_schema::{GRAPH_FILE, parse_graph};
use crate::loader::NODES_DIR;
use crate::module_file::text_of;
use crate::module_io::entity_file_for;
use crate::node_files::{move_node_files, remove_node_files};
use crate::paths::{extname, join};
use crate::workspace_file::ws_exists;
use k7x::edit::{PathSeg, Value, WriteOptions};
use song::routing::decode_graph;
use song::routing::derive_chains;

/// The graph a writer reads, parsed, or thrown as unreadable.
///
/// # Errors
/// A graph that does not read.
pub fn read_graph_value(fs: &dyn Fs, dir: &str) -> Res<Value> {
    let r = parse_graph(&project_graph_text(fs, dir)?);
    match r.value {
        Some(v) => Ok(v),
        None => fail(format!(
            "{GRAPH_FILE}: {}",
            r.issues.first().map_or("unreadable", String::as_str)
        )),
    }
}

/// Bypass (or re-enable) a module.
///
/// # Errors
/// A graph that does not read, or a module it does not name.
pub fn set_module_bypass(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, bypassed: bool) -> Res<String> {
    let node = node_arg(fs, dir, module)?;
    edit_project_graph(fs, env, dir, |doc| set_module_enabled(doc, &node, !bypassed))
}

/// Mute (or unmute) a lane through its bypass, giving the graph an entry for a lane it does not name yet.
///
/// # Errors
/// As `set_module_bypass`.
pub fn set_lane_bypass(fs: &mut dyn Fs, env: &Env, dir: &str, lane: &str, bypassed: bool) -> Res<String> {
    let node = node_arg(fs, dir, lane)?;
    edit_project_graph(fs, env, dir, |doc| {
        if bypassed {
            add_node(doc, &node)?;
        } else {
            let listed = doc.get_in(&[PathSeg::Key("bypassed".to_owned())])?;
            // Unmuting a lane nothing bypasses is already done.
            let held =
                matches!(&listed, Some(Value::List(l)) if l.iter().any(|x| matches!(x, Value::Str(s) if *s == node)));
            if !held {
                return Ok(());
            }
        }
        set_module_enabled(doc, &node, !bypassed)
    })
}

/// Set (or clear) one `bypassed:` entry the graph need not hold as a node.
///
/// # Errors
/// What the document's writers fail with.
pub fn set_bypass_entry(fs: &mut dyn Fs, env: &Env, dir: &str, name: &str, bypassed: bool) -> Res<String> {
    edit_project_graph(fs, env, dir, |doc| {
        let key = [PathSeg::Key("bypassed".to_owned())];
        let list: Vec<String> = match doc.get_in(&key)? {
            Some(Value::List(l)) => l.iter().map(|x| text_of(Some(x))).collect(),
            _ => vec![],
        };
        let next: Vec<String> = if bypassed {
            let mut n = list.clone();
            if !n.iter().any(|x| x == name) {
                n.push(name.to_owned());
            }
            n
        } else {
            list.iter().filter(|x| *x != name).cloned().collect()
        };
        if next == list {
            return Ok(());
        }
        if next.is_empty() {
            doc.delete_in(&key, false)?;
        } else {
            let v = Value::List(next.into_iter().map(Value::Str).collect());
            doc.set_in(&key, &v, WriteOptions { inline: true })?;
        }
        Ok(())
    })
}

/// Take a node out of the signal path, joining what fed it to what it fed; the nodes it rewired.
///
/// # Errors
/// A node the chains do not hold.
pub fn unlink_chain_node(fs: &mut dyn Fs, env: &Env, dir: &str, name: &str) -> Res<Vec<String>> {
    let node = node_arg(fs, dir, name)?;
    let value = read_graph_value(fs, dir)?;
    let modules = match value.get("modules") {
        Some(Value::Map(m)) => m.clone(),
        _ => vec![],
    };
    let is_dest = modules
        .iter()
        .any(|(_, d)| matches!(d, Value::List(l) if l.iter().any(|x| matches!(x, Value::Str(s) if *s == node))));
    if !modules.iter().any(|(k, _)| *k == node) && !is_dest {
        return fail(format!("no '{node}' in the chains"));
    }
    let mut rewired = vec![];
    edit_project_graph(fs, env, dir, |doc| {
        rewired = unlink_node(doc, &node)?;
        Ok(())
    })?;
    Ok(rewired)
}

/// The lane whose chain plays `node`, or None: the first when several reach it.
#[must_use]
pub fn lane_of_node(fs: &dyn Fs, dir: &str, node: &str) -> Option<String> {
    let node = node_arg(fs, dir, node).ok()?;
    let text = project_graph_text(fs, dir).ok()?;
    let segments = decode_graph(GRAPH_FILE, &text).ok()?;
    derive_chains(&segments, &|_| false)
        .into_iter()
        .find(|(name, c)| {
            *name != node && (c.use_steps.iter().any(|s| s.module == node) || c.out.iter().any(|o| o.to == node))
        })
        .map(|(name, _)| name)
}

fn module_used_as_node(fs: &dyn Fs, dir: &str, name: &str) -> bool {
    if is_chain_project(fs, dir) || entity_file_for(fs, &join(&[dir, NODES_DIR]), NODES_DIR, name).is_some() {
        return false;
    }
    let Ok(text) = project_graph_text(fs, dir) else {
        return false;
    };
    let Some(Value::Map(modules)) = parse_graph(&text).value.and_then(|g| g.get("modules").cloned()) else {
        return false;
    };
    modules.iter().any(|(k, _)| k == name)
        || modules
            .iter()
            .any(|(_, d)| matches!(d, Value::List(l) if l.iter().any(|x| matches!(x, Value::Str(s) if s == name))))
}

/// A chain project renames LANES only: any other node is known by its id, so renaming it changes nothing.
fn rename_in_chains(fs: &mut dyn Fs, env: &Env, dir: &str, from: &str, to: &str) -> Res<(Vec<String>, Vec<String>)> {
    let cp = read_chain_project(fs, dir)?;
    let Some(n) = cp.nodes.get(from) else {
        return fail(format!("no node '{from}' in this project"));
    };
    if n.track.is_none() {
        return Ok((vec![], vec![]));
    }
    if to.is_empty()
        || !to
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return fail(format!(
            "invalid track name '{to}' — letters, digits, '.', '_' and '-' only"
        ));
    }
    if read_chain_project(fs, dir)?.nodes.contains_key(to) {
        return fail(format!("'{to}' is already a node of this project"));
    }
    let mut rewired = vec![];
    let moved = rename_chain_lane(fs, env, dir, from, to, |doc| {
        rewired = rename_node(doc, from, to)?;
        Ok(())
    })?;
    Ok((moved, rewired))
}

/// Rename a node: its file, its preset, and every trace of it in the chains.
///
/// # Errors
/// A node the project does not hold, or a name already taken.
pub fn rename_chain_node(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    from: &str,
    to: &str,
) -> Res<(Vec<String>, Vec<String>)> {
    if is_chain_project(fs, dir) {
        return rename_in_chains(fs, env, dir, from, to);
    }
    let Some(found) = entity_file_for(fs, &join(&[dir, NODES_DIR]), NODES_DIR, from) else {
        if module_used_as_node(fs, dir, from) {
            return fail(format!(
                "'{from}' is a module used directly on the graph, not a node of one — rename the module, or give it a node first (`node new`) and rename that"
            ));
        }
        return fail(format!("no node '{from}' in this project"));
    };
    let ext = extname(&found.path).to_owned();
    if ws_exists(fs, &join(&[dir, NODES_DIR, &format!("{to}{ext}")])) {
        return fail(format!("nodes/{to}{ext} already exists — refusing to overwrite"));
    }
    let moved = move_node_files(fs, dir, &found, from, to)?;
    let mut rewired = vec![];
    edit_project_graph(fs, env, dir, |doc| {
        rewired = rename_node(doc, from, to)?;
        Ok(())
    })?;
    Ok((moved, rewired))
}

/// Delete a node: its file, its preset, and every trace of it in the chains; files first.
///
/// # Errors
/// A node the project does not hold.
pub fn delete_chain_node(fs: &mut dyn Fs, env: &Env, dir: &str, name: &str) -> Res<(Vec<String>, Vec<String>)> {
    if is_chain_project(fs, dir) {
        let node = node_arg(fs, dir, name)?;
        let mut rewired = vec![];
        let removed = delete_chain_item(fs, env, dir, &node, |doc| {
            rewired = remove_node(doc, &node)?;
            Ok(())
        })?;
        return Ok((rewired, removed));
    }
    let Some(found) = entity_file_for(fs, &join(&[dir, NODES_DIR]), NODES_DIR, name) else {
        if module_used_as_node(fs, dir, name) {
            let mut rewired = vec![];
            edit_project_graph(fs, env, dir, |doc| {
                rewired = remove_node(doc, name)?;
                Ok(())
            })?;
            return Ok((rewired, vec![]));
        }
        return fail(format!("no node '{name}' in this project"));
    };
    let removed = remove_node_files(fs, dir, &found, name)?;
    let mut rewired = vec![];
    edit_project_graph(fs, env, dir, |doc| {
        rewired = remove_node(doc, name)?;
        Ok(())
    })?;
    Ok((rewired, removed))
}

/// Put a module in the graph wired to nothing.
///
/// # Errors
/// What the graph writer fails with.
pub fn place_chain_node(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str) -> Res<String> {
    let node = node_arg(fs, dir, module)?;
    edit_project_graph(fs, env, dir, |doc| add_node(doc, &node))
}

/// Pin every node in `at` that has no pin yet, in one write.
///
/// # Errors
/// What the graph writer fails with.
pub fn set_chain_node_positions(fs: &mut dyn Fs, env: &Env, dir: &str, at: &[(String, (f64, f64))]) -> Res<String> {
    edit_project_graph(fs, env, dir, |doc| set_node_positions(doc, at))
}

/// Hand every node back to the layout.
///
/// # Errors
/// What the graph writer fails with.
pub fn clear_chain_node_positions(fs: &mut dyn Fs, env: &Env, dir: &str) -> Res<String> {
    edit_project_graph(fs, env, dir, clear_node_positions)
}

/// Pin a node's place on the graph, or with `None` hand it back to the layout.
///
/// # Errors
/// What the graph writer fails with.
pub fn set_chain_node_position(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    name: &str,
    at: Option<(f64, f64)>,
) -> Res<String> {
    let node = node_arg(fs, dir, name)?;
    edit_project_graph(fs, env, dir, |doc| set_node_position(doc, &node, at))
}

/// Draw a new arrow from `from` to `to`, each optionally `name#port`.
///
/// # Errors
/// What the graph writer fails with.
pub fn connect_chain_nodes(fs: &mut dyn Fs, env: &Env, dir: &str, from: &str, to: &str) -> Res<String> {
    let (a, b) = (node_arg(fs, dir, from)?, node_arg(fs, dir, to)?);
    edit_project_graph(fs, env, dir, |doc| connect_nodes(doc, &a, &b))
}

/// Take the arrow from `from` to `to` out.
///
/// # Errors
/// What the graph writer fails with.
pub fn disconnect_chain_nodes(fs: &mut dyn Fs, env: &Env, dir: &str, from: &str, to: &str) -> Res<String> {
    let (a, b) = (node_arg(fs, dir, from)?, node_arg(fs, dir, to)?);
    edit_project_graph(fs, env, dir, |doc| disconnect_nodes(doc, &a, &b))
}

/// Move one end of an existing arrow, in one read and one write.
///
/// # Errors
/// What the graph writer fails with.
pub fn replug_chain_nodes(fs: &mut dyn Fs, env: &Env, dir: &str, ends: [&str; 4]) -> Res<String> {
    let [a, b, c, d] = [0, 1, 2, 3].map(|i| node_arg(fs, dir, ends[i]));
    let (a, b, c, d) = (a?, b?, c?, d?);
    edit_project_graph(fs, env, dir, |doc| {
        disconnect_nodes(doc, &a, &b)?;
        connect_nodes(doc, &c, &d)
    })
}

/// Insert a module into the arrow from `before` to `after`.
///
/// # Errors
/// What the graph writer fails with.
pub fn insert_chain_step(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    before: &str,
    after: Option<&str>,
    module: &str,
) -> Res<String> {
    let b = node_arg(fs, dir, before)?;
    let a = after.map(|x| node_arg(fs, dir, x)).transpose()?;
    let m = node_arg(fs, dir, module)?;
    edit_project_graph(fs, env, dir, |doc| insert_between(doc, &b, a.as_deref(), &m))
}
