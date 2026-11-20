// routing/derive.rs — graph.k7x declares SEGMENTS (who feeds whom); this derives the chains: maximal linear runs.
// A reader would get wrong that a run breaks where the signal splits and where two signals meet; a lane is never decided here, its caller says.

use super::types::{Chain, Out, Segments, UseStep};
use super::{MIX_NODE, RENDER_NODE, split_port};
use crate::modules::ModuleKind;
use crate::resolve::Resolved;
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

/// The source a chain TEMPLATE starts at, and where it ends.
pub const CHAIN_IN: &str = "@in";
pub const CHAIN_OUT: &str = "@out";

struct Dest<'a> {
    to: &'a str,
    from_port: Option<&'a str>,
}

/// One module wired to nothing at either end: placed but not connected.
pub fn is_parked(c: &Chain) -> bool {
    c.from.is_none() && c.use_steps.len() == 1 && c.out.is_empty()
}

/// The nodes of kind `track` among resolved modules: the lanes a caller derives with.
pub fn lane_names(resolved: &IndexMap<String, Resolved>) -> HashSet<String> {
    resolved
        .iter()
        .filter(|(_, m)| m.kind == Some(ModuleKind::Track))
        .map(|(k, _)| k.clone())
        .collect()
}

/// The chains a segment graph describes, keyed by name; `is_lane` says which nodes are lanes.
pub fn derive_chains(segments: &Segments, is_lane: &dyn Fn(&str) -> bool) -> IndexMap<String, Chain> {
    let Some(modules) = segments.modules.as_ref() else {
        return IndexMap::new();
    };
    let bypassed: HashSet<&str> = segments.bypassed_of().collect();
    let node_of = |r: &str| -> String { split_port(r).0.to_owned() };
    let lane = |n: &str| -> bool {
        let node = split_port(n).0;
        node == CHAIN_IN || node == MIX_NODE || node == RENDER_NODE || is_lane(node)
    };
    // A LANE keyed on more than one output is more than one SOURCE; a lane keyed once keeps that key's port.
    let mut per_lane: IndexMap<String, Vec<&str>> = IndexMap::new();
    for key in modules.keys().filter(|k| lane(k)) {
        per_lane.entry(node_of(key)).or_default().push(key);
    }
    let mut split_lanes: HashSet<&str> = HashSet::new();
    let mut sole_port: HashMap<&str, Option<&str>> = HashMap::new();
    for (node, keys) in &per_lane {
        if keys.len() > 1 {
            split_lanes.insert(node);
        } else {
            sole_port.insert(node, split_port(keys[0]).1);
        }
    }
    let src_id = |key: &str| -> String {
        let node = node_of(key);
        if split_lanes.contains(node.as_str()) {
            key.to_owned()
        } else {
            node
        }
    };
    // Indexes built once: destinations by source id, in key order.
    let mut dests: HashMap<String, Vec<Dest>> = HashMap::new();
    let mut sources: IndexSet<String> = IndexSet::new();
    for (key, ds) in modules {
        let id = src_id(key);
        sources.insert(id.clone());
        let port = split_port(key).1;
        let list = dests.entry(id).or_default();
        list.extend(ds.iter().map(|to| Dest { to, from_port: port }));
    }
    let degree = |n: &str| dests.get(n).map_or(0, Vec::len);
    let mut feeders: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    for (key, ds) in modules {
        let id = src_id(key);
        let deg = degree(&id);
        for d in ds {
            match split_port(d) {
                (_, None) => feeders.entry(d).or_default().push(deg),
                (node, Some(_)) => named.entry(node.to_owned()).or_default().push(id.clone()),
            }
        }
    }
    let absorbed =
        |n: &str| -> bool { !lane(n) && feeders.get(n).is_some_and(|f| f.len() == 1 && f[0] == 1) && degree(n) > 0 };
    let step = |module: &str| UseStep {
        module: module.to_owned(),
        inputs: named.get(module).filter(|e| !e.is_empty()).cloned(),
        enabled: bypassed.contains(module).then_some(false),
    };
    let outs = |ds: &[Dest]| -> Vec<Out> {
        ds.iter()
            .map(|d| Out {
                to: d.to.to_owned(),
                gain: 1.0,
                from_port: d.from_port.map(str::to_owned),
            })
            .collect()
    };
    let follow = |placed: &mut HashSet<String>, start: &[Dest]| -> (Vec<UseStep>, Vec<Out>) {
        let mut used = Vec::new();
        let mut at: &[Dest] = start;
        while at.len() == 1 && split_port(at[0].to).1.is_none() && absorbed(at[0].to) && !placed.contains(at[0].to) {
            placed.insert(at[0].to.to_owned());
            used.push(step(at[0].to));
            at = dests.get(at[0].to).map_or(&[], Vec::as_slice);
        }
        (used, outs(at))
    };
    let mut placed: HashSet<String> = HashSet::new();
    let mut chains: IndexMap<String, Chain> = IndexMap::new();
    let none: Vec<Dest> = Vec::new();
    let of = |n: &str| dests.get(n).unwrap_or(&none);
    // The LANES first, claimed whether or not they are wired, named after the SOURCE.
    for n in &sources {
        if !lane(n) {
            continue;
        }
        placed.insert(n.clone());
        let ds = of(n);
        if ds.is_empty() {
            continue;
        }
        let (node, own_port) = split_port(n);
        let port = if split_lanes.contains(node) {
            own_port
        } else {
            sole_port.get(node).copied().flatten()
        };
        let (used, out) = follow(&mut placed, ds);
        chains.insert(
            n.clone(),
            Chain {
                from: Some(node.to_owned()),
                from_port: port.map(str::to_owned),
                use_steps: used,
                out,
            },
        );
    }
    // Then every module starting a run of its own, then a cycle, which has no root and is broken at its first module.
    for pass in 0..2 {
        for m in &sources {
            if (pass == 0 && absorbed(m)) || placed.contains(m) {
                continue;
            }
            placed.insert(m.clone());
            let (used, out) = follow(&mut placed, of(m));
            let mut all = vec![step(m)];
            all.extend(used);
            chains.insert(
                m.clone(),
                Chain {
                    from: None,
                    from_port: None,
                    use_steps: all,
                    out,
                },
            );
        }
    }
    chains
}
