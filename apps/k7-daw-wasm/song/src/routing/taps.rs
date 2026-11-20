// routing/taps.rs — a rack node's taps routed to subchains: which output PAIR each takes, and the note it plays.
// A reader would get wrong that pair 0 is the instrument's main output: routed taps take 1, 2, … in the order of their names.

use super::types::TapFeed;
use indexmap::IndexMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutedTap {
    pub node: String,
    pub tap: String,
    pub to: String,
    pub pair: u32,
}

/// The `{ note, pair }` a plugin carries for a routed tap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TapOut {
    pub note: u8,
    pub pair: u32,
}

/// Each node's routed taps, ordered by code point of the tap name and numbered from pair 1.
pub fn tap_pairs(feeds: &[TapFeed]) -> IndexMap<String, Vec<RoutedTap>> {
    let mut by_node: IndexMap<String, Vec<RoutedTap>> = IndexMap::new();
    for f in feeds {
        by_node.entry(f.node.clone()).or_default().push(RoutedTap {
            node: f.node.clone(),
            tap: f.tap.clone(),
            to: f.to.clone(),
            pair: 0,
        });
    }
    for list in by_node.values_mut() {
        // UTF-8 bytewise order is code point order.
        list.sort_by(|a, b| a.tap.cmp(&b.tap));
        for (i, t) in list.iter_mut().enumerate() {
            t.pair = u32::try_from(i + 1).unwrap_or(u32::MAX);
        }
    }
    by_node
}

/// The list a plugin carries, or a refusal naming the tap the instrument does not have.
pub fn tap_outs_for(
    node: &str,
    routed: &[RoutedTap],
    note_of: &dyn Fn(&str) -> Option<u8>,
) -> Result<Vec<TapOut>, String> {
    routed
        .iter()
        .map(|t| {
            note_of(&t.tap)
                .map(|note| TapOut { note, pair: t.pair })
                .ok_or_else(|| {
                    format!(
                        "node '{node}': tap '{}' is routed to a subchain, but the node has no tap of that name",
                        t.tap
                    )
                })
        })
        .collect()
}
