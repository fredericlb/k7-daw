// curve_placement.rs — a curve card sits at the left of the node it drives, under its top, whether the layout is fresh or the other cards keep their place.
// A reader would get wrong that these answer positions only: the page spreads them over its own node objects, which carry far more than a box.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The gap a curve keeps from its target, and from any card it is tried against.
const CURVE_GAP: f64 = 24.0;
/// How far below its target's top a curve card starts: under the event plugs on that top edge.
const CURVE_DROP: f64 = 24.0;

/// The part of a card the placement reads.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Box2 {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Where a card goes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Moved {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

/// One cable as the placement reads it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct PlaceEdge {
    pub source: String,
    pub target: String,
    pub automation: Option<bool>,
}

/// Each curve card's target, read off its automation cable: one binding, one card, one target.
pub fn curve_targets(edges: &[PlaceEdge], is_curve: impl Fn(&str) -> bool) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for e in edges {
        if e.automation == Some(true) && is_curve(&e.source) && !out.contains_key(&e.source) {
            out.insert(e.source.clone(), e.target.clone());
        }
    }
    out
}

fn by_id(nodes: &[Box2]) -> HashMap<&str, &Box2> {
    nodes.iter().map(|n| (n.id.as_str(), n)).collect()
}

/// The x a row sweep sorts each node by: a curve just before its target, so nothing comes between them.
pub fn row_sort_keys(nodes: &[Box2], targets: &IndexMap<String, String>) -> Vec<f64> {
    let ids = by_id(nodes);
    nodes
        .iter()
        .map(|n| match targets.get(&n.id).and_then(|t| ids.get(t.as_str())) {
            Some(t) => t.x - 1e-3,
            None => n.x,
        })
        .collect()
}

/// A fresh curve put left of its target on the target's line, ahead of the row sweep that parts collisions.
pub fn curves_left_of_targets(nodes: &[Box2], targets: &IndexMap<String, String>) -> Vec<Moved> {
    let ids = by_id(nodes);
    nodes
        .iter()
        .filter_map(|n| {
            let t = ids.get(targets.get(&n.id)?.as_str())?;
            Some(Moved { id: n.id.clone(), x: t.x - n.w - CURVE_GAP, y: t.y })
        })
        .collect()
}

/// Every curve card to the left of its target and under its top, several stacked down: forced, whatever was remembered.
pub fn curves_under_left(nodes: &[Box2], targets: &IndexMap<String, String>) -> Vec<Moved> {
    let ids = by_id(nodes);
    let mut below: HashMap<&str, f64> = HashMap::new();
    let mut placed = Vec::new();
    for n in nodes {
        let Some(tid) = targets.get(&n.id) else { continue };
        let Some(t) = ids.get(tid.as_str()) else { continue };
        let y = below.get(tid.as_str()).copied().unwrap_or(t.y + CURVE_DROP);
        placed.push(Moved { id: n.id.clone(), x: t.x - n.w - CURVE_GAP, y });
        below.insert(tid.as_str(), y + n.h + CURVE_GAP);
    }
    placed
}

/// The indexes of the nodes and cables the zoomed-out view keeps: no curve card and no cable touching one.
pub fn without_curves(kinds: &[(String, String)], edges: &[(String, String)]) -> Option<(Vec<usize>, Vec<usize>)> {
    let curves: Vec<&str> = kinds.iter().filter(|(_, k)| k == "curve").map(|(id, _)| id.as_str()).collect();
    if curves.is_empty() {
        return None;
    }
    let nodes = kinds.iter().enumerate().filter(|(_, (id, _))| !curves.contains(&id.as_str())).map(|(i, _)| i);
    let kept = edges
        .iter()
        .enumerate()
        .filter(|(_, (s, t))| !curves.contains(&s.as_str()) && !curves.contains(&t.as_str()))
        .map(|(i, _)| i);
    Some((nodes.collect(), kept.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, x: f64, y: f64) -> Box2 {
        Box2 { id: id.into(), x, y, w: 200.0, h: 120.0 }
    }

    fn edge(source: &str, target: &str, automation: bool) -> PlaceEdge {
        PlaceEdge {
            source: source.into(),
            target: target.into(),
            automation: automation.then_some(true),
        }
    }

    #[test]
    fn a_curve_follows_its_first_binding_only() {
        let edges = [edge("t", "a", false), edge("crv", "c", true), edge("crv", "a", true)];
        let targets = curve_targets(&edges, |id| id == "crv");
        assert_eq!(targets.get("crv").map(String::as_str), Some("c"));
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn a_fresh_curve_sits_left_of_its_target_on_its_line() {
        let nodes = [card("c", 2000.0, 900.0), card("crv", 0.0, 0.0)];
        let targets = IndexMap::from([("crv".to_owned(), "c".to_owned())]);
        let moved = curves_left_of_targets(&nodes, &targets);
        assert_eq!(moved, vec![Moved { id: "crv".into(), x: 2000.0 - 200.0 - 24.0, y: 900.0 }]);
        assert_eq!(row_sort_keys(&nodes, &targets), vec![2000.0, 2000.0 - 1e-3]);
    }

    #[test]
    fn curves_under_left_stack_down_whatever_was_remembered() {
        let nodes = [card("c", 2000.0, 900.0), card("one", 5.0, 5.0), card("two", 9.0, 9.0)];
        let targets = IndexMap::from([("one".to_owned(), "c".to_owned()), ("two".to_owned(), "c".to_owned())]);
        let moved = curves_under_left(&nodes, &targets);
        assert_eq!(moved[0], Moved { id: "one".into(), x: 1776.0, y: 924.0 });
        assert_eq!(moved[1], Moved { id: "two".into(), x: 1776.0, y: 924.0 + 120.0 + 24.0 });
    }

    #[test]
    fn the_zoomed_out_graph_drops_every_curve_and_every_cable_touching_one() {
        let kinds: Vec<(String, String)> =
            [("t", "track"), ("a", "module"), ("crv", "curve")].map(|(a, b)| (a.into(), b.into())).to_vec();
        let edges: Vec<(String, String)> = [("t", "a"), ("crv", "a"), ("a", "crv")].map(|(a, b)| (a.into(), b.into())).to_vec();
        assert_eq!(without_curves(&kinds, &edges), Some((vec![0, 1], vec![0])));
        assert_eq!(without_curves(&kinds[..2], &edges[..1]), None);
    }
}
