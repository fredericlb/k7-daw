// tap_chain.rs — a rack tap's SUBCHAIN, edited from the tap: add a module, remove one, move one, written into the node's chain file.
// A reader would get wrong that the last node removed takes the subchain with it and puts the tap back on the main output.

use crate::chain_data::{Chain, Item, Link, Step, Subchain, id_of, name_of, remove_key, set_key};
use crate::chain_item_edit::chain_file_path;
use crate::chain_project::{ChainNode, ChainProject, read_chain_project};
use crate::confine::Env;
use crate::error::{Res, fail};
use crate::format_chain::format_chain;
use crate::fs::Fs;
use crate::loader::load_module_refs;
use crate::paths::relative;
use crate::rack_taps::taps_of;
use crate::workspace_doc::ws_write_text;
use k7x::edit::Value;
use song::PathRule;
use song::modules::ModuleKind;
use song::routing::chain_file::CHAIN_SINKS;
use std::collections::HashSet;

/// Where the edit lands: the node's chain file, cloned, and the node's own place inside the clone.
struct Open {
    cp: ChainProject,
    at: ChainNode,
    file: Chain,
}

impl Open {
    fn steps(&mut self) -> Option<&mut Vec<Step>> {
        match &self.at.sub {
            Some(sub) => self
                .file
                .subchains
                .iter_mut()
                .find(|s| s.id == *sub)
                .map(|s| &mut s.steps),
            None => Some(&mut self.file.steps),
        }
    }

    fn item(&mut self) -> Option<&mut Item> {
        let (step, k) = (self.at.step, self.at.item.unwrap_or(0));
        self.steps()?.get_mut(step)?.items_mut().get_mut(k)
    }

    /// The index of the subchain the item routes `tap` to.
    fn sub_of(&mut self, tap: &str) -> Option<usize> {
        let to = match self.item()?.get("tap_outs")?.get(tap)? {
            Value::Str(s) => s.clone(),
            _ => return None,
        };
        self.file.subchains.iter().position(|s| s.id == to)
    }
}

fn open(fs: &dyn Fs, env: &Env, dir: &str, node: &str, tap: &str) -> Res<(Open, Option<usize>)> {
    let cp = read_chain_project(fs, dir)?;
    if !cp.issues.is_empty() {
        return fail(format!(
            "the chains do not read — fix them first:\n  {}",
            cp.issues.join("\n  ")
        ));
    }
    let Some(at) = cp.nodes.get(node).cloned() else {
        return fail(format!("no node '{node}' in this project"));
    };
    if !taps_of(fs, env, dir, node)?.iter().any(|t| t == tap) {
        return fail(format!("'{node}' has no tap '{tap}'"));
    }
    let file = cp.chains.get(&at.chain).cloned().unwrap_or_default();
    let mut o = Open { cp, at, file };
    if o.item().is_none() {
        return fail(format!("'{node}' is not where its chain file says"));
    }
    let sub = o.sub_of(tap);
    Ok((o, sub))
}

/// Check the edited file still reads, then write it: the project-relative path.
fn write(fs: &mut dyn Fs, dir: &str, o: &Open) -> Res<String> {
    let chain = &o.at.chain;
    let text = format_chain(&o.file, o.cp.texts.get(chain).map_or("", String::as_str))?;
    if let Err(f) = song::routing::decode_chain("chain", &text, PathRule::HOST) {
        return fail(format!("the edit would make {chain} unreadable — {}", f.message));
    }
    let path = chain_file_path(dir, chain);
    ws_write_text(fs, &path, &text)?;
    Ok(relative(dir, &path))
}

/// The node steps of a subchain, its trailing `to:` left out.
fn node_steps(sub: &Subchain) -> Vec<Step> {
    sub.steps.iter().filter(|s| !s.is_end()).cloned().collect()
}

fn holds(s: &Step, id: &str) -> bool {
    s.items().iter().any(|i| name_of(i) == id)
}

/// A subchain id free in `file`, after the tap: `snare-fx`, then `snare-fx-2`.
fn free_id(file: &Chain, tap: &str) -> String {
    let mut base = String::new();
    for c in tap.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            base.push(c);
        } else {
            // A regex without the `u` flag replaces each UTF-16 unit.
            base.extend(std::iter::repeat_n('-', c.len_utf16()));
        }
    }
    base.push_str("-fx");
    let taken: HashSet<&str> = file.subchains.iter().map(|s| s.id.as_str()).collect();
    let mut id = base.clone();
    let mut i = 2;
    while taken.contains(id.as_str()) || CHAIN_SINKS.contains(&id.as_str()) {
        id = format!("{base}-{i}");
        i += 1;
    }
    id
}

fn set_outs(item: &mut Item, tap: &str, to: Option<&str>) {
    let mut outs = match item.get("tap_outs") {
        Some(Value::Map(m)) => m.clone(),
        _ => vec![],
    };
    match to {
        Some(id) => match outs.iter_mut().find(|(k, _)| k == tap) {
            Some(slot) => slot.1 = Value::Str(id.to_owned()),
            None => outs.push((tap.to_owned(), Value::Str(id.to_owned()))),
        },
        None => outs.retain(|(k, _)| k != tap),
    }
    if outs.is_empty() {
        remove_key(item, "tap_outs");
    } else {
        set_key(item, "tap_outs", Value::Map(outs));
    }
}

/// A node of module `module_ref` added at the end of the tap's subchain; the subchain made, and the tap routed to it, when there is none.
///
/// # Errors
/// A module the project cannot see or an io module, chains that do not read, or a node without that tap.
pub fn add_tap_chain_node(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    tap: &str,
    module_ref: &str,
    new_id: &mut dyn FnMut() -> String,
) -> Res<(String, String)> {
    let vocab = load_module_refs(fs, env, dir)?;
    match vocab.modules.get(module_ref) {
        Some(m) if m.node.is_none() && m.engine_of.is_none() => {
            if m.kind == Some(ModuleKind::Io) {
                return fail(format!("'{module_ref}' is an io module — a tap's chain holds effects"));
            }
        }
        _ => return fail(format!("'{module_ref}' is not a module this project can see")),
    }
    let (mut o, sub) = open(fs, env, dir, node, tap)?;
    let added = Value::Map(vec![
        ("id".to_owned(), Value::Str(new_id())),
        ("extends".to_owned(), Value::Str(module_ref.to_owned())),
    ]);
    let id = id_of(&added).to_owned();
    if let Some(si) = sub {
        // Before the trailing link, so the new node is the last one the tap goes through.
        let sc = &mut o.file.subchains[si];
        let end = sc.steps.iter().position(Step::is_end).unwrap_or(sc.steps.len());
        sc.steps.insert(end, Step::One(added));
        let sub_id = sc.id.clone();
        write(fs, dir, &o)?;
        return Ok((sub_id, id));
    }
    let sub_id = free_id(&o.file, tap);
    o.file.subchains.push(Subchain {
        id: sub_id.clone(),
        steps: vec![
            Step::One(added),
            Step::End(vec![Link {
                chain: "MIX".to_owned(),
                level: None,
                port: None,
                port_first: false,
            }]),
        ],
    });
    if let Some(item) = o.item() {
        set_outs(item, tap, Some(&sub_id));
    }
    write(fs, dir, &o)?;
    Ok((sub_id, id))
}

/// Remove one node of the tap's subchain; the last one takes the subchain with it and the tap back to the main output.
///
/// # Errors
/// Chains that do not read, a tap with no subchain, or a node the subchain lacks.
pub fn remove_tap_chain_node(fs: &mut dyn Fs, env: &Env, dir: &str, node: &str, tap: &str, id: &str) -> Res<bool> {
    let (mut o, sub) = open(fs, env, dir, node, tap)?;
    let Some(si) = sub else {
        return fail(format!("'{node}': tap '{tap}' is not routed to a subchain"));
    };
    let sc = &mut o.file.subchains[si];
    let Some(at) = sc.steps.iter().position(|s| holds(s, id)) else {
        return fail(format!("no node '{id}' in subchain '{}'", sc.id));
    };
    match &mut sc.steps[at] {
        Step::Many(items) if items.len() > 1 => items.retain(|i| name_of(i) != id),
        _ => {
            sc.steps.remove(at);
        }
    }
    let emptied = node_steps(sc).is_empty();
    if emptied {
        let gone = sc.id.clone();
        o.file.subchains.retain(|s| s.id != gone);
        if let Some(item) = o.item() {
            set_outs(item, tap, None);
        }
    }
    write(fs, dir, &o)?;
    Ok(emptied)
}

/// Move one node of the tap's subchain `by` places (-1 earlier, +1 later), kept inside its nodes.
///
/// # Errors
/// Chains that do not read, a tap with no subchain, or a node the subchain lacks.
pub fn move_tap_chain_node(fs: &mut dyn Fs, env: &Env, dir: &str, node: &str, tap: &str, id: &str, by: i64) -> Res<()> {
    let (mut o, sub) = open(fs, env, dir, node, tap)?;
    let Some(si) = sub else {
        return fail(format!("'{node}': tap '{tap}' is not routed to a subchain"));
    };
    let sc = &mut o.file.subchains[si];
    let mut nodes = node_steps(sc);
    let Some(from) = nodes.iter().position(|s| holds(s, id)) else {
        return fail(format!("no node '{id}' in subchain '{}'", sc.id));
    };
    let last = i64::try_from(nodes.len()).unwrap_or(i64::MAX) - 1;
    let to = usize::try_from((i64::try_from(from).unwrap_or(0) + by).clamp(0, last)).unwrap_or(0);
    if to == from {
        return Ok(());
    }
    let moved = nodes.remove(from);
    nodes.insert(to, moved);
    nodes.extend(sc.steps.iter().filter(|s| s.is_end()).cloned());
    sc.steps = nodes;
    write(fs, dir, &o)?;
    Ok(())
}
