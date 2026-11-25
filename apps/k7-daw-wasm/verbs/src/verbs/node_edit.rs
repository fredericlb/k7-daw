// node_edit.rs — `node delete`, `node replace`, `node stack` and `chain save`, after verbs/node.ts, nodeDeleteGuard.ts, nodeStack.ts and chainSave.ts.
// A reader would get wrong a replace's rename: only a name the `<owner>_<module>_<n>` convention made follows the new module.

use super::graph_read::read_graph;
use super::shared::{Done, split_words};
use crate::host::{Host, Thrown};
use k7x::edit::Value;
use song::Cache;
use song::routing::split_port;
use song::views::surfaces::project_surfaces;
use song_edit::assembly_edit::{drop_node_from_assemblies, rename_node_in_assemblies};
use song_edit::chain_graph_edit::chain_node_name;
use song_edit::chain_item_edit::is_chain_project;
use song_edit::chain_stack::stack_chain_node;
use song_edit::graph_edit::{delete_chain_node, lane_of_node, read_graph_value, rename_chain_node};
use song_edit::instance_name::{instance_name, name_part};
use song_edit::loader::load_modules;
use song_edit::project_edit::set_module_extends;
use song_edit::project_files::node_name_taken;

/// The graph's segments: each node and where its signal goes, the ports dropped.
fn segments(h: &Host) -> Option<Vec<(String, Vec<String>)>> {
    let v = read_graph_value(&*h.fs, &h.song_path).ok()?;
    let Some(Value::Map(m)) = v.get("modules") else {
        return Some(Vec::new());
    };
    Some(
        m.iter()
            .map(|(k, v)| {
                let dests = match v {
                    Value::List(l) => l
                        .iter()
                        .filter_map(|x| {
                            if let Value::Str(s) = x {
                                Some(split_port(s).0.to_owned())
                            } else {
                                None
                            }
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                (k.clone(), dests)
            })
            .collect(),
    )
}

/// `starvedByDelete`: the nodes `node` is the last feed of while their module requires one; empty to go ahead.
fn starved_by_delete(h: &Host, node: &str) -> Vec<String> {
    let Some(graph) = segments(h) else {
        return Vec::new();
    };
    let feeders = |target: &str| -> Vec<&str> {
        graph
            .iter()
            .filter(|(_, d)| d.iter().any(|x| x == target))
            .map(|(k, _)| k.as_str())
            .collect()
    };
    if !feeders(node).is_empty() {
        return Vec::new();
    }
    let mut fed: Vec<String> = Vec::new();
    for d in graph.iter().filter(|(k, _)| k == node).flat_map(|(_, d)| d) {
        if !fed.contains(d) {
            fed.push(d.clone());
        }
    }
    if fed.is_empty() {
        return Vec::new();
    }
    let Ok(g) = read_graph(h) else {
        return Vec::new();
    };
    fed.into_iter()
        .filter(|d| {
            let mandatory = g.mandatory.get(d).copied().unwrap_or(0);
            feeders(d).iter().filter(|f| **f != node).count() < mandatory
        })
        .collect()
}

/// `node delete <node>` — spliced out of its chain, its feeders joined to what it fed, out of every box.
pub fn node_delete(a: &str, h: &mut Host) -> Done {
    let Some(node) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: node delete <node>".to_owned()]);
        return Ok(());
    };
    let starved = starved_by_delete(h, &node);
    if !starved.is_empty() {
        h.refuse([format!(
            "node delete failed: {node} is the only signal reaching {}, whose input is mandatory — delete {} first, or connect another source",
            starved.join(", "),
            if starved.len() > 1 { "those" } else { "it" }
        )]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let done = delete_chain_node(h.fs, &h.env, &dir, &node)
        .and_then(|r| drop_node_from_assemblies(h.fs, &dir, &node).map(|_| r));
    match done {
        Ok((rewired, removed)) => {
            let joined = if rewired.is_empty() {
                String::new()
            } else {
                format!(" — rejoined what fed it: {}", rewired.join(", "))
            };
            h.log([format!("deleted {}{joined}", removed.join(", "))]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node delete failed: {e}")]),
    }
    Ok(())
}

/// `madeFrom`: is `name` `<stem>_<digits>`?
fn made_from(name: &str, stem: &str) -> bool {
    name.strip_prefix(stem)
        .and_then(|r| r.strip_prefix('_'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// `renamedInstance`: what a use is called once the module under it changes, or None for a name a person chose.
fn renamed_instance(
    name: &str,
    from: &str,
    to: &str,
    owner: Option<&str>,
    taken: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let what = name_part(from);
    if what == name_part(to) {
        return None;
    }
    if let Some(o) = owner
        && made_from(name, &format!("{}_{what}", name_part(o)))
    {
        return Some(instance_name(to, Some(o), taken));
    }
    made_from(name, &what).then(|| instance_name(to, None, taken))
}

/// `surfacesNaming`: the surfaces whose mapping names `module`, which nothing writes back.
fn surfaces_naming(h: &Host, module: &str) -> Vec<String> {
    let Ok(s) = project_surfaces(&*h.fs, &mut Cache::new(), &h.song_path) else {
        return Vec::new();
    };
    s.surfaces
        .into_iter()
        .filter(|p| {
            p.surface
                .mapping
                .as_ref()
                .is_some_and(|m| m.values().any(|b| b.0.as_ref().is_some_and(|b| b.module == module)))
        })
        .map(|p| p.id)
        .collect()
}

/// The last segment of a module ref: the namespace is not part of what a use is named after.
fn leaf_of(r: &str) -> Option<&str> {
    r.rsplit('/').next().filter(|s| !s.is_empty())
}

/// `renameForNewModule`: the node's name follows the module it now extends, or the line says why not.
fn rename_for_new_module(h: &mut Host, node: &str, was: &str, r: &str) -> Option<String> {
    let to = leaf_of(r)?;
    let dir = h.song_path.clone();
    let owner = lane_of_node(&*h.fs, &dir, node);
    let next = renamed_instance(node, was, to, owner.as_deref(), &|n| {
        node_name_taken(&*h.fs, &h.env, &dir, n)
    })?;
    let mapped = surfaces_naming(h, node);
    if !mapped.is_empty() {
        let what = if mapped.len() > 1 { "surfaces" } else { "surface" };
        return Some(format!(
            "{node}: kept its name — {what} {} name it, and a mapping is not rewritten",
            mapped.join(", ")
        ));
    }
    let done = rename_chain_node(h.fs, &h.env, &dir, node, &next)
        .and_then(|(_, rewired)| rename_node_in_assemblies(h.fs, &dir, node, &next).map(|b| (rewired, b)));
    Some(match done {
        Ok((rewired, boxes)) => {
            let rw = if rewired.is_empty() {
                String::new()
            } else {
                format!(" (rewired {})", rewired.join(", "))
            };
            let bx = if boxes.is_empty() {
                String::new()
            } else {
                format!(" (followed in {})", boxes.join(", "))
            };
            format!("{node} → {next}, following the module it now extends{rw}{bx}")
        }
        Err(e) => format!("{node}: kept its name — {e}"),
    })
}

/// `node replace <node> <extends-ref>` — one edit to the node's `extends`, then its name follows.
pub fn node_replace(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(key), Some(r)) = (w.first().map(|s| (*s).to_owned()), w.get(1).map(|s| (*s).to_owned())) else {
        h.refuse(["usage: node replace <node> <extends-ref>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let was = load_modules(&*h.fs, &h.env, &dir)
        .ok()
        .and_then(|m| m.get(&key).and_then(|m| m.authored.extends.clone()))
        .and_then(|e| leaf_of(&e).map(str::to_owned));
    let (rel, dropped, alternative) = match set_module_extends(h.fs, &h.env, &dir, &key, &r) {
        Ok(x) => x,
        Err(e) => {
            h.refuse([format!("node replace failed: {e}")]);
            return Ok(());
        }
    };
    let alt = alternative
        .as_ref()
        .map_or_else(String::new, |a| format!(", alternative '{a}'"));
    let own = if dropped.is_empty() {
        String::new()
    } else {
        format!(" — dropped its own {}", dropped.join(", "))
    };
    h.log([format!("{key}{alt}: extends {r} ({rel}){own}")]);
    let renamed = match (&was, &alternative) {
        (Some(was), None) => rename_for_new_module(h, &key, was, &r),
        _ => None,
    };
    h.reload();
    if let Some(line) = renamed {
        h.log([line]);
    }
    Ok(())
}

/// `node stack <node> <new node>` — the new node right under the first, both in one subchain.
pub fn node_stack(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 2 {
        h.refuse([
            "usage: node stack <node> <new node> (the new node goes right under the first, in one subchain)".to_owned(),
        ]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    if !is_chain_project(&*h.fs, &dir) {
        h.refuse(["node stack: a stack is a subchain, and only a chain project has them (apps/k7-daw-frontend/packages/song/docs/chainFiles.md)".to_owned()]);
        return Ok(());
    }
    let name = |p: &str| chain_node_name(&*h.fs, &dir, p).unwrap_or_else(|_| p.to_owned());
    let (above, node) = (name(w[0]), name(w[1]));
    match stack_chain_node(h.fs, &dir, &above, &node) {
        Ok(written) => {
            h.reload();
            h.log([format!("stacked {node} under {above} ({})", written.join(", "))]);
        }
        Err(e) => h.refuse([format!("node stack: {e}")]),
    }
    Ok(())
}

/// `chain save [name]` — the bench's run written as one of the account's chains by the bench's own route.
pub fn chain_save(a: &str, h: &mut Host) -> Result<(), Thrown> {
    use serde_json::Value as Json;
    let name = split_words(a).first().map(|s| (*s).to_owned());
    if h.song_path.is_empty() {
        h.refuse(["chain save: no project is open".to_owned()]);
        return Ok(());
    }
    let args = name.map_or_else(|| serde_json::json!({}), |n| serde_json::json!({ "name": n }));
    let saved = match h.call(super::routes::BENCH_SAVE, args)? {
        Ok(v) => v,
        Err(f) => {
            h.refuse([format!("chain save: {}", f.error)]);
            return Ok(());
        }
    };
    let r = saved.get("ref").and_then(Json::as_str).unwrap_or_default();
    let steps = saved.get("steps").and_then(Json::as_u64).unwrap_or(0);
    let mut lines = vec![if steps > 0 {
        let s = if steps == 1 { "" } else { "s" };
        format!("saved {r} ({steps} step{s}) — clip process <clip> {r}")
    } else {
        format!("saved {r}: its input alone")
    }];
    let outside: Vec<&str> = saved
        .get("outside")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .collect();
    // A module the person sees on the bench and the chain leaves out must be said, never dropped quietly.
    if !outside.is_empty() {
        lines.push(format!(
            "not in it: {} — only what sits between IN and fader is the chain",
            outside.join(", ")
        ));
    }
    h.log(lines);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_conventional_name_follows_its_module() {
        let free = |_: &str| false;
        assert_eq!(
            renamed_instance("drums_comp_2", "comp", "eq", Some("drums"), &free).as_deref(),
            Some("drums_eq_1")
        );
        assert_eq!(
            renamed_instance("comp_3", "comp", "eq", None, &free).as_deref(),
            Some("eq_1")
        );
        assert_eq!(renamed_instance("my-comp", "comp", "eq", None, &free), None);
        assert_eq!(renamed_instance("comp_1", "comp", "comp", None, &free), None);
    }
}
