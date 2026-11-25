// node_make.rs — making nodes and modules: `module new|own|fork|clone`, `node new` and `node insert` on an arrow, after verbs/nodeNew.ts.
// A reader would get wrong where a new node sits: pinned where the cursor was, else above the origin, never left to the layout.

use super::graph_read::{read_graph, take_spawn_at};
use super::module_files::{
    canonical_target, create_module_file, create_node_on, existing_extends, local_module_folder, vocabulary,
};
use super::shared::{Done, split_words};
use crate::host::Host;
use indexmap::IndexMap;
use k7x::edit::Value;
use song::graph::place::{NodePlace, node_places};
use song::modules::LoadedModule;
use song::routing::read_chain_project;
use song_edit::account_fork::fork_into_account;
use song_edit::chain_graph_edit::chain_node_name;
use song_edit::chain_item_edit::is_chain_project;
use song_edit::graph_edit::{
    connect_chain_nodes, insert_chain_step, place_chain_node, read_graph_value, set_chain_node_position,
};
use song_edit::module_fork::materialise_engine_module;
use song_edit::module_io::is_project_folder;
use song_edit::project_files::{clone_module_file, create_node, free_node_name};
use song_edit::workspace_file::ws_exists;

/// Where a node wired to nothing is born when no cursor said: above the origin, clear of the first lane.
const ORPHAN_AT: (f64, f64) = (0.0, -30.0);

/// `module new <category>/<name> [extends-ref]` — a module scaffolded, and only that.
pub fn module_create(a: &str, h: &mut Host) -> Done {
    let usage = "usage: module new <category>/<name> [extends-ref]";
    let w = split_words(a);
    if w.is_empty() || w.len() > 2 {
        h.refuse([usage.to_owned()]);
        return Ok(());
    }
    let (target, extends) = (w[0], w.get(1).copied());
    let Some((category, name)) = target
        .split_once('/')
        .filter(|(c, n)| !c.is_empty() && !n.is_empty() && !n.contains('/'))
    else {
        h.refuse([
            format!("module new failed: '{target}' is not <category>/<name>"),
            usage.to_owned(),
        ]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let exists = ws_exists(&*h.fs, &local_module_folder(&*h.fs, &dir, category, name));
    let own = format!(
        "{}/{category}/{name}",
        if is_project_folder(&*h.fs, &dir) {
            "@workspace"
        } else {
            "@project"
        }
    );
    let next = format!("use it with `node new {target} <node-name>`");
    if exists {
        let inherits = existing_extends(&*h.fs, &dir, category, name);
        if let Some(r) = extends.filter(|r| *r != own && !inherits.is_empty() && inherits != *r) {
            h.refuse([
                format!("module new failed: module '{name}' extends '{inherits}', not '{r}' — drop the extends-ref to use it as it is"),
                usage.to_owned(),
            ]);
            return Ok(());
        }
        h.refuse([format!("{target} is already here"), next]);
        return Ok(());
    }
    match create_module_file(h, category, name, extends) {
        Ok(rel) => {
            h.log([
                format!("created {rel} — add an inline plugin/rack (or an `extends`) to give it a sound"),
                next,
            ]);
            h.reload();
        }
        Err(e) => h.refuse([
            format!("module new failed: {e} — nothing was changed"),
            usage.to_owned(),
        ]),
    }
    Ok(())
}

/// `shownNode`: a created node as a person reads it, its module and place in a chain project, its path otherwise.
fn shown_node(h: &Host, rel: &str) -> String {
    if !is_chain_project(&*h.fs, &h.song_path) {
        return rel.to_owned();
    }
    let Ok(cp) = read_chain_project(&*h.fs, &mut song::Cache::new(), &h.song_path) else {
        return rel.to_owned();
    };
    let places = node_places(&cp);
    match places.get(rel) {
        Some(NodePlace { place, uses: Some(u) }) => format!("{u} · {place} ({rel})"),
        Some(NodePlace { place, uses: None }) => format!("{place} ({rel})"),
        None => rel.to_owned(),
    }
}

/// The new node placed, pinned and logged.
fn placed(
    h: &mut Host,
    rel: &str,
    node: &str,
    at: Option<(f64, f64)>,
    on: Option<&str>,
) -> Result<(), song_edit::Error> {
    let dir = h.song_path.clone();
    place_chain_node(h.fs, &h.env, &dir, node)?;
    set_chain_node_position(h.fs, &h.env, &dir, node, Some(at.unwrap_or(ORPHAN_AT)))?;
    let on = on.map_or_else(String::new, |r| format!(" on {r}"));
    h.log([format!(
        "created {}{on} — in the graph, wired to nothing yet",
        shown_node(h, rel)
    )]);
    h.reload();
    Ok(())
}

/// `node new <category>/<name>|<@ref> [<node-name>] [<extends-ref>] [x,y]` — one use of a module, and its place in the graph.
pub fn node_new(a: &str, h: &mut Host) -> Done {
    let usage = "usage: node new <category>/<name>|<@module-ref> [<node-name>] [<extends-ref>] [x,y]";
    let mut parts: Vec<String> = split_words(a).into_iter().map(str::to_owned).collect();
    let at = take_spawn_at(&mut parts);
    if parts.is_empty() || parts.len() > 3 {
        h.refuse([usage.to_owned()]);
        return Ok(());
    }
    let known = vocabulary(h)?;
    let parts: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 || p.contains('/') {
                canonical_target(h, p, &known)
            } else {
                p.clone()
            }
        })
        .collect();
    let (target, rest) = (&parts[0], &parts[1..]);
    let dir = h.song_path.clone();
    if target.starts_with('@') {
        if rest.len() > 1 || rest.iter().any(|p| p.contains('/')) {
            h.refuse([usage.to_owned()]);
            return Ok(());
        }
        let leaf = target.rsplit('/').next().unwrap_or("");
        let node = rest
            .first()
            .cloned()
            .unwrap_or_else(|| free_node_name(&*h.fs, &h.env, &dir, leaf, None));
        let done = create_node_on(h, target, &node, &known)
            .and_then(|rel| placed(h, &rel, &node, at, Some(target)).map_err(|e| e.to_string()));
        if let Err(e) = done {
            h.refuse([format!("node new failed: {e}")]);
        }
        return Ok(());
    }
    let Some((category, module)) = target
        .split_once('/')
        .filter(|(c, n)| !c.is_empty() && !n.is_empty() && !n.contains('/'))
    else {
        h.refuse([format!("'{target}' is not <category>/<name>"), usage.to_owned()]);
        return Ok(());
    };
    let r = rest.iter().find(|p| p.contains('/'));
    let named = rest.iter().find(|p| !p.contains('/'));
    if rest.len() > if r.is_some() && named.is_some() { 2 } else { 1 } {
        h.refuse([usage.to_owned()]);
        return Ok(());
    }
    let node = named
        .cloned()
        .unwrap_or_else(|| free_node_name(&*h.fs, &h.env, &dir, module, None));
    let made = match r {
        Some(r) => create_node_on(h, r, &node, &known),
        None => {
            let catalogue: IndexMap<String, LoadedModule> = known
                .iter()
                .filter(|(k, _)| k.starts_with("@engine/"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            create_node(h.fs, &h.env, &dir, category, module, &node, &catalogue, h.new_id).map_err(|e| e.to_string())
        }
    };
    let done = made.and_then(|rel| placed(h, &rel, &node, at, r.map(String::as_str)).map_err(|e| e.to_string()));
    if let Err(e) = done {
        h.refuse([format!("node new failed: {e}")]);
    }
    Ok(())
}

/// `module own <@engine/…>` — an engine module given a folder here, the chains repointed at it.
pub fn module_own(a: &str, h: &mut Host) -> Done {
    let Some(r) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: module own <@engine/…>".to_owned()]);
        return Ok(());
    };
    let known = vocabulary(h)?;
    let dir = h.song_path.clone();
    match materialise_engine_module(h.fs, &h.env, &dir, &r, &known, None) {
        Ok(f) => {
            let by = if f.chains.is_empty() {
                String::new()
            } else {
                format!(", played by {}", f.chains.join(", "))
            };
            h.log([format!("{} is now {}{by}", f.reference, f.path)]);
            h.reload();
        }
        Err(e) => h.refuse([format!("module own failed: {e}")]),
    }
    Ok(())
}

/// `module fork <@library/path>` — the library module made the account's own, the project pointed at it.
pub fn module_fork(a: &str, h: &mut Host) -> Done {
    let Some(r) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: module fork <@library/path>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match fork_into_account(h.fs, &h.env, &dir, &r) {
        Ok(f) => {
            let n = f.repointed.len();
            let files = if n == 0 {
                String::new()
            } else {
                format!(
                    " — {n} file{} of this project now name it",
                    if n > 1 { "s" } else { "" }
                )
            };
            let how = if f.created { "made, extending" } else { "already forks" };
            h.log([format!("{} {how} {r}{files}", f.reference)]);
            h.reload();
        }
        Err(e) => h.refuse([format!("module fork: {e}")]),
    }
    Ok(())
}

/// `module clone <category>/<name> <newName>` — the module's folder copied under a new name.
pub fn module_clone(a: &str, h: &mut Host) -> Done {
    let usage = "usage: clone module <category>/<name> <newName>";
    let w = split_words(a);
    let [target, new_name] = w.as_slice() else {
        h.refuse([usage.to_owned()]);
        return Ok(());
    };
    let Some((category, name)) = target
        .split_once('/')
        .filter(|(c, n)| !c.is_empty() && !n.is_empty() && !n.contains('/'))
    else {
        h.refuse([format!("'{target}' is not <category>/<name>"), usage.to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match clone_module_file(h.fs, &dir, category, name, new_name) {
        Ok(rel) => {
            h.log([format!("cloned {target} → {rel}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("clone module failed: {e}")]),
    }
    Ok(())
}

/// `arrowState`: whether `before → after` is an arrow (`Ok(false)`), a master run to write first (`Ok(true)`), or why neither.
fn arrow_state(h: &Host, before: &str, after: &str) -> Result<bool, String> {
    let master = (before == "MIX" && after == "OUT") || (before == "RENDER" && after == "VIDEO_OUT");
    let graph = read_graph_value(&*h.fs, &h.song_path).map_err(|e| e.to_string())?;
    let modules = match graph.get("modules") {
        Some(Value::Map(m)) => m.clone(),
        _ => Vec::new(),
    };
    let dests: Option<Vec<String>> = modules.iter().find(|(k, _)| k == before).map(|(_, v)| match v {
        Value::List(l) => l
            .iter()
            .filter_map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None })
            .collect(),
        _ => Vec::new(),
    });
    if master && dests.as_ref().is_none_or(Vec::is_empty) {
        return Ok(true);
    }
    let Some(dests) = dests else {
        let names: Vec<&str> = modules.iter().map(|(k, _)| k.as_str()).collect();
        let list = if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(", ")
        };
        return Err(format!("no node \"{before}\" in graph.k7x (nodes: {list})"));
    };
    if !dests.iter().any(|d| d == after) {
        let list = if dests.is_empty() {
            "nothing".to_owned()
        } else {
            dests.join(", ")
        };
        return Err(format!("'{before}' does not feed '{after}' (it feeds: {list})"));
    }
    Ok(false)
}

/// `node insert <before> <after> <node> [x,y]` — a node that exists spliced into an arrow.
pub fn node_insert(a: &str, h: &mut Host) -> Done {
    let mut parts: Vec<String> = split_words(a).into_iter().map(str::to_owned).collect();
    let at = take_spawn_at(&mut parts);
    if parts.len() < 3 {
        h.refuse(["usage: node insert <before> <after> <node> [x,y] (the two ends of one graph arrow, then the node to splice between them)".to_owned()]);
        return Ok(());
    }
    let n: Vec<String> = parts
        .iter()
        .map(|p| chain_node_name(&*h.fs, &h.song_path, p).unwrap_or_else(|_| p.clone()))
        .collect();
    let (before, after, node) = (&n[0], &n[1], &n[2]);
    let g = match read_graph(h) {
        Ok(g) => g,
        Err(e) => {
            h.refuse([format!("node insert: {}", e.0)]);
            return Ok(());
        }
    };
    let write = match arrow_state(h, before, after) {
        Ok(w) => w,
        Err(e) => {
            h.refuse([e]);
            return Ok(());
        }
    };
    if !g.is_module(node) && !g.is_ref(node) {
        let list = if g.modules.is_empty() {
            "none".to_owned()
        } else {
            g.modules.join(", ")
        };
        h.refuse([format!(
            "no node \"{node}\" in this project (nodes: {list}) — create one with `node new`"
        )]);
        return Ok(());
    }
    if let Some(cold) = g.cold_placement_error(node, before) {
        h.refuse([cold]);
        return Ok(());
    }
    let mid = || {
        let a = g.positions.get(before.split('#').next().unwrap_or(""))?;
        let b = g.positions.get(after.split('#').next().unwrap_or(""))?;
        Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))
    };
    let born = at.or_else(mid);
    let dir = h.song_path.clone();
    let done = (|| -> Result<(), song_edit::Error> {
        if write {
            connect_chain_nodes(h.fs, &h.env, &dir, before, after)?;
        }
        insert_chain_step(h.fs, &h.env, &dir, before, Some(after), node)?;
        if let Some(p) = born {
            set_chain_node_position(h.fs, &h.env, &dir, node, Some(p))?;
        }
        Ok(())
    })();
    match done {
        Ok(()) => {
            h.log([format!("inserted {node} between {before} and {after}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node insert failed: {e}")]),
    }
    Ok(())
}
