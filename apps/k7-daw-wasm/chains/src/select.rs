// select.rs — several nodes selected at once (Shift+click), which of them a control LINKS, and a Shift+drag's catch.
// A control moved on one node of a selection of one ref on one engine moves on all of them by the same TRAVEL.
// A reader would get wrong that a mixed selection links NOTHING: no partial fan-out (docs/graphBuild.md).

use serde::{Deserialize, Serialize};

/// A card as the selection reads it: its kind and name, the ref and engine it uses, its box.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SelectNode {
    pub kind: String,
    pub module: Option<String>,
    pub uses: Option<String>,
    pub engine: Option<String>,
    pub frozen: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub h: f64,
}

/// A rectangle dragged over the graph, in world coordinates, from any corner to the opposite one.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DragBox {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// Shift+click on `name`: the selection with it toggled, the single selection it grew from counted in.
pub fn toggle_selection(current: &[String], primary: Option<&str>, name: &str) -> Vec<String> {
    let mut next: Vec<String> = if current.is_empty() {
        primary.map(str::to_owned).into_iter().collect()
    } else {
        current.to_vec()
    };
    match next.iter().position(|n| n == name) {
        Some(at) => {
            next.remove(at);
        }
        None => next.push(name.to_owned()),
    }
    next
}

fn is_card(n: &SelectNode) -> bool {
    n.kind == "module" && n.module.as_deref().is_some_and(|m| !m.is_empty())
}

/// The names of the OTHER selected cards a control on `of` drives too: none unless all share one ref and one engine.
pub fn linked_nodes(nodes: &[SelectNode], selection: &[String], of: &SelectNode) -> Vec<String> {
    let Some(name) = of.module.as_deref().filter(|m| !m.is_empty()) else {
        return Vec::new();
    };
    if selection.len() < 2 || !selection.iter().any(|s| s == name) {
        return Vec::new();
    }
    let picked: Vec<&SelectNode> = nodes
        .iter()
        .filter(|m| is_card(m) && m.module.as_deref().is_some_and(|n| selection.iter().any(|s| s == n)))
        .collect();
    // Every selected name must be a node here: one that is not (a chain, a lane) makes the selection mixed.
    if picked.len() != selection.len() {
        return Vec::new();
    }
    let same = picked
        .iter()
        .all(|m| m.uses.as_deref().is_some_and(|u| !u.is_empty()) && m.uses == of.uses && m.engine == of.engine);
    if !same {
        return Vec::new();
    }
    // A baked module has no plugin in the engine; it is left out rather than spoiling the rest.
    picked
        .into_iter()
        .filter(|m| m.module.as_deref() != Some(name) && m.frozen.as_deref().is_none_or(str::is_empty))
        .filter_map(|m| m.module.clone())
        .collect()
}

/// The module names whose cards a rectangle TOUCHES, in world coordinates.
pub fn nodes_in_box(nodes: &[SelectNode], b: DragBox) -> Vec<String> {
    let (left, right) = (b.x0.min(b.x1), b.x0.max(b.x1));
    let (top, bottom) = (b.y0.min(b.y1), b.y0.max(b.y1));
    nodes
        .iter()
        .filter(|n| is_card(n) && n.x < right && n.x + n.w > left && n.y < bottom && n.y + n.h > top)
        .filter_map(|n| n.module.clone())
        .collect()
}

#[cfg(test)]
mod tests;
