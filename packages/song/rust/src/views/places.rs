// views/places.rs — where a chain project's unnamed node sits, as a person reads it: `chain · step n`.
// A reader would get wrong that this is a private copy: 2k's `node_places` is the same walk, to fold at the combine (decision 623).

use crate::routing::ChainProject;
use std::collections::HashMap;

/// `chain · step n` (`chain › sub` inside a subchain), steps from 1, and `n.k` for the k-th of a side-by-side step.
pub fn place_of(chain: &str, sub: Option<&str>, step: usize, item: Option<usize>) -> String {
    let at = match sub.filter(|s| !s.is_empty()) {
        Some(sub) => format!("{chain} › {sub}"),
        None => chain.to_owned(),
    };
    match item {
        Some(k) => format!("{at} · step {}.{}", step + 1, k + 1),
        None => format!("{at} · step {}", step + 1),
    }
}

/// Every unnamed node of a chain project by its name; a lane has a name of its own and none here.
pub fn node_places_of(project: &ChainProject) -> HashMap<&str, String> {
    project
        .nodes
        .values()
        .filter(|n| n.track.is_none())
        .map(|n| (n.name.as_str(), place_of(&n.chain, n.sub.as_deref(), n.step, n.item)))
        .collect()
}
