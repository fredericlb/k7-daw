// chain_stack.rs — STACK a loose node under another in a chain project: the two become a subchain run in place of the one above.
// A reader would get wrong that the new node must be LOOSE, alone in its chain and wired to nothing: its chain file goes, its item moves.

use crate::chain_data::{Chain, Step, Subchain, id_of};
use crate::chain_item_edit::chain_file_path;
use crate::chain_project::{LAYOUT_FILE, read_chain_project, read_layout, write_layout};
use crate::error::{Error, Res, fail, io_error};
use crate::format_chain::format_chain;
use crate::fs::Fs;
use crate::paths::relative;
use crate::workspace_doc::ws_write_text;
use k7x::edit::Value;
use song::PathRule;
use song::routing::chain_file::CHAIN_SINKS;
use std::collections::HashSet;

/// A free subchain id inside `c`, after what the node above extends.
fn free_id(c: &Chain, item: &Value) -> String {
    let base = match item.get("extends") {
        Some(Value::Str(e)) => e.rsplit('/').next().unwrap_or("").to_owned(),
        _ => id_of(item).to_owned(),
    };
    let taken: HashSet<&str> = c.subchains.iter().map(|s| s.id.as_str()).collect();
    let mut id = base.clone();
    let mut i = 2;
    while taken.contains(id.as_str()) || CHAIN_SINKS.contains(&id.as_str()) {
        id = format!("{base}-{i}");
        i += 1;
    }
    id
}

/// Put loose node `node` right under `above`, in one subchain with it; the project-relative files written.
///
/// # Errors
/// Chains that do not read, a node that is not where it must be, or the file system's.
pub fn stack_chain_node(fs: &mut dyn Fs, dir: &str, above: &str, node: &str) -> Res<Vec<String>> {
    let cp = read_chain_project(fs, dir)?;
    if !cp.issues.is_empty() {
        return fail(format!(
            "the chains do not read — fix them first:\n  {}",
            cp.issues.join("\n  ")
        ));
    }
    let Some(at) = cp.nodes.get(above) else {
        return fail(format!("no node '{above}' in this project"));
    };
    if at.track.is_some() {
        return fail(format!("'{above}' is a lane, a source: nothing stacks under it"));
    }
    if at.item.is_some() {
        return fail(format!(
            "'{above}' sits in a side-by-side step — stack under a node standing alone"
        ));
    }
    let Some(born) = cp.nodes.get(node) else {
        return fail(format!("no node '{node}' in this project"));
    };
    let Some(own) = cp.chains.get(&born.chain) else {
        return fail(format!("no node '{node}' in this project"));
    };
    let loose = born.sub.is_none()
        && born.chain != at.chain
        && own.subchains.is_empty()
        && own.steps.len() == 1
        && own.steps[0].items().len() == 1;
    if !loose {
        return fail(format!(
            "'{node}' is already wired — only a node `node new` just made can be stacked"
        ));
    }
    let Some(mut parent) = cp.chains.get(&at.chain).cloned() else {
        return fail(format!("no node '{above}' in this project"));
    };
    let item = born.value.clone();
    if let Some(sub) = &at.sub {
        // Already in a subchain: the new node joins it, right under the one above.
        if let Some(sc) = parent.subchains.iter_mut().find(|s| s.id == *sub) {
            sc.steps.insert((at.step + 1).min(sc.steps.len()), Step::One(item));
        }
    } else {
        let id = free_id(&parent, &at.value);
        parent.steps[at.step] = Step::Call(id.clone());
        parent.subchains.push(Subchain {
            id,
            steps: vec![Step::One(at.value.clone()), Step::One(item)],
        });
    }
    let text = format_chain(&parent, cp.texts.get(&at.chain).map_or("", String::as_str))?;
    if let Err(f) = song::routing::decode_chain("chain", &text, PathRule::HOST) {
        return Err(Error::Msg(format!(
            "stacking would make {} unreadable — {}",
            at.chain, f.message
        )));
    }
    let at_file = chain_file_path(dir, &at.chain);
    let born_file = chain_file_path(dir, &born.chain);
    ws_write_text(fs, &at_file, &text)?;
    fs.remove(&born_file).map_err(|e| io_error(&e, "rm", &born_file))?;
    let mut written = vec![
        relative(dir, &at_file),
        format!("{} (removed)", relative(dir, &born_file)),
    ];
    // Its pin placed a loose card; in a chain it is laid out with the chain, so the pin goes.
    let mut pins = read_layout(fs, dir)?;
    if let Value::Map(m) = &mut pins
        && m.iter().any(|(k, _)| k == node)
    {
        m.retain(|(k, _)| k != node);
        write_layout(fs, dir, &pins)?;
        written.push(LAYOUT_FILE.to_owned());
    }
    Ok(written)
}
