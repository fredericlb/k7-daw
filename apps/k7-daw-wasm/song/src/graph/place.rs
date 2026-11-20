// graph/place.rs — where a chain project's unnamed node SITS, as a person reads it: `chain · step n`.
// A reader would get wrong that this is only what is SHOWN beside a node's uuid: every command and the wire carry the uuid.

use crate::routing::{ChainNode, ChainProject, Item};
use std::collections::HashMap;

/// What an unnamed node is shown as: where it sits, and the module it extends when it extends one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodePlace<'a> {
    pub place: String,
    pub uses: Option<&'a str>,
}

/// `chain · step n` (`chain › sub` inside a subchain), steps from 1, and `n.k` for the k-th of a side-by-side step (435).
pub fn place_of(n: &ChainNode) -> String {
    let chain = match &n.sub {
        Some(sub) => format!("{} › {sub}", n.chain),
        None => n.chain.clone(),
    };
    let item = n.item.map_or_else(String::new, |k| format!(".{}", k + 1));
    format!("{chain} · step {}{item}", n.step + 1)
}

/// Every unnamed node by its name (its id); a lane has a name of its own and none here.
pub fn node_places(p: &ChainProject) -> HashMap<&str, NodePlace<'_>> {
    p.nodes
        .values()
        .filter(|n| n.track.is_none())
        .map(|n| {
            let uses = match &n.value {
                Item::Node(item) => item.module.extends.as_deref(),
                _ => None,
            };
            (
                n.name.as_str(),
                NodePlace {
                    place: place_of(n),
                    uses,
                },
            )
        })
        .collect()
}

/// Each subchain RUN in place, its nodes in order: what a row stacks into one column (435).
pub fn node_stacks(p: &ChainProject) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for c in p.chains.values() {
        let called = c.called_subchains();
        for sc in c.subchains.iter().flatten() {
            if called.contains(sc.id.as_str()) {
                out.push(
                    sc.steps
                        .iter()
                        .flat_map(|s| s.items())
                        .map(|it| it.name().to_owned())
                        .collect(),
                );
            }
        }
    }
    out
}
