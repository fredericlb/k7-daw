// chain_nest.rs — derived chains back into their files: a `<parent>/<id>` chain returns to its parent while its scope holds.
// A reader would get wrong that a chain is lifted out for one reason only: something outside its parent links to it or it links out.

use crate::chain_data::{Chain, Item, Step, Subchain, id_of, lane_of};
use crate::chain_project::ChainNode;
use indexmap::{IndexMap, IndexSet};
use song::routing::chain_file::CHAIN_SINKS;
use song::routing::split_key;
use std::collections::{HashMap, HashSet};

fn is_sink(name: &str) -> bool {
    CHAIN_SINKS.contains(&name)
}

fn end_links(steps: &[Step]) -> &[crate::chain_data::Link] {
    Chain::links(steps)
}

fn relink(steps: &[Step], rename: &HashMap<String, String>) -> Vec<Step> {
    steps
        .iter()
        .map(|s| match s {
            Step::End(to) => Step::End(
                to.iter()
                    .map(|l| {
                        let mut l = l.clone();
                        if let Some(n) = rename.get(&l.chain) {
                            l.chain.clone_from(n);
                        }
                        l
                    })
                    .collect(),
            ),
            other => other.clone(),
        })
        .collect()
}

/// Each derived `<parent>/<id>` chain back into its parent while its scope holds, else renamed free.
#[must_use]
pub fn nest_subchains(out: IndexMap<String, Chain>) -> IndexMap<String, Chain> {
    let keys: Vec<String> = out.keys().filter(|k| split_key(k).1.is_some()).cloned().collect();
    let mut feeders: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (name, c) in &out {
        for l in end_links(&c.steps) {
            feeders.entry(&l.chain).or_default().insert(name);
        }
    }
    // Lifted out one at a time, since a sibling lifted out is "outside" for the ones linking to it.
    let mut nested: IndexSet<String> = keys.iter().cloned().collect();
    let in_scope = |nested: &IndexSet<String>, k: &str, parent: &str| {
        k == parent || (nested.contains(k) && split_key(k).0 == parent)
    };
    let mut changed = true;
    while changed {
        changed = false;
        for k in nested.iter().cloned().collect::<Vec<_>>() {
            let (parent, _) = split_key(&k);
            let Some(c) = out.get(&k) else { continue };
            let ok = out.contains_key(parent)
                && split_key(parent).1.is_none()
                && c.start.is_none()
                && c.ports.is_empty()
                && !c.bus_only
                && feeders
                    .get(k.as_str())
                    .is_none_or(|fs| fs.iter().all(|f| in_scope(&nested, f, parent)))
                && end_links(&c.steps)
                    .iter()
                    .all(|l| is_sink(&l.chain) || in_scope(&nested, &l.chain, parent));
            if !ok {
                nested.shift_remove(&k);
                changed = true;
            }
        }
    }
    // A chain lifted out takes a free top-level name: `<parent>-<id>`, numbered past any taken.
    let mut rename: HashMap<String, String> = HashMap::new();
    let mut taken: HashSet<String> = out.keys().filter(|k| split_key(k).1.is_none()).cloned().collect();
    for k in &keys {
        let (parent, sub) = split_key(k);
        let sub = sub.unwrap_or("");
        if nested.contains(k) {
            rename.insert(k.clone(), sub.to_owned());
        } else {
            let mut n = format!("{parent}-{sub}");
            let mut i = 2;
            while taken.contains(&n) {
                n = format!("{parent}-{sub}-{i}");
                i += 1;
            }
            taken.insert(n.clone());
            rename.insert(k.clone(), n);
        }
    }
    let mut result: IndexMap<String, Chain> = IndexMap::new();
    for (name, c) in &out {
        if split_key(name).1.is_some() {
            if !nested.contains(name)
                && let Some(to) = rename.get(name)
            {
                let mut moved = c.clone();
                moved.steps = relink(&c.steps, &rename);
                result.insert(to.clone(), moved);
            }
            continue;
        }
        let subs: Vec<Subchain> = keys
            .iter()
            .filter(|k| nested.contains(*k) && split_key(k).0 == name)
            .filter_map(|k| {
                Some(Subchain {
                    id: rename.get(k)?.clone(),
                    steps: relink(&out.get(k)?.steps, &rename),
                })
            })
            .collect();
        let mut kept = c.clone();
        kept.steps = relink(&c.steps, &rename);
        kept.subchains = subs;
        result.insert(name.clone(), kept);
    }
    result
}

/// What `regroup_calls` needs of the project as it was: where each node sat, and each file's chain.
pub struct PrevPlaces<'a> {
    pub nodes: &'a IndexMap<String, ChainNode>,
    pub chains: &'a IndexMap<String, Chain>,
}

fn name_of(i: &Item) -> &str {
    lane_of(i).unwrap_or_else(|| id_of(i))
}

/// Each stretch of a run whose nodes sat in a subchain RUN in place goes back into it, as a `{ sub }` step.
#[must_use]
pub fn regroup_calls(out: IndexMap<String, Chain>, prev: Option<&PrevPlaces>) -> IndexMap<String, Chain> {
    let Some(prev) = prev else { return out };
    let call_of = |name: &str| -> Option<String> {
        let n = prev.nodes.get(name)?;
        let sub = n.sub.as_ref()?;
        let file = prev.chains.get(&n.chain)?;
        file.called_subchains().contains(sub.as_str()).then(|| sub.clone())
    };
    // The subchain a step's nodes ALL sat in; a step mixing two places belongs to none.
    let group_of = |s: &Step| -> Option<String> {
        let subs: Vec<Option<String>> = s.items().iter().map(|i| call_of(name_of(i))).collect();
        let first = subs.first()?.clone()?;
        subs.iter()
            .all(|x| x.as_deref() == Some(first.as_str()))
            .then_some(first)
    };
    let is_new = |s: &Step| {
        let items = s.items();
        !items.is_empty() && items.iter().all(|i| !prev.nodes.contains_key(name_of(i)))
    };
    let mut result = IndexMap::new();
    for (name, c) in out {
        let mut subs: Vec<Subchain> = c.subchains.clone();
        let mut used: HashSet<String> = subs.iter().map(|s| s.id.clone()).collect();
        let mut steps: Vec<Step> = vec![];
        let mut changed = false;
        let mut i = 0;
        while i < c.steps.len() {
            let id = group_of(&c.steps[i]);
            // Never the first step: that one is a node, its ports what the chain is linked into.
            let Some(id) = id.filter(|id| i != 0 && !used.contains(id)) else {
                steps.push(c.steps[i].clone());
                i += 1;
                continue;
            };
            // A node inserted between two of its members joins the stretch, as one put inside it would.
            let mut group: Vec<Step> = vec![];
            while i < c.steps.len() {
                let s = &c.steps[i];
                let inside = group_of(s).as_deref() == Some(id.as_str())
                    || (is_new(s)
                        && c.steps
                            .get(i + 1)
                            .is_some_and(|n| group_of(n).as_deref() == Some(id.as_str())));
                if !inside {
                    break;
                }
                group.push(s.clone());
                i += 1;
            }
            steps.push(Step::Call(id.clone()));
            subs.push(Subchain {
                id: id.clone(),
                steps: group,
            });
            used.insert(id);
            changed = true;
        }
        let kept = if changed {
            Chain {
                steps,
                subchains: subs,
                ..c
            }
        } else {
            c
        };
        result.insert(name, kept);
    }
    result
}
