// row_seams.rs — where a glued module row offers its `+`: on each seam between two columns a cable joins, once past the last column, and a branch.
// A reader would get wrong that a seam is offered only where an arrow exists: two neighbours with no cable between them have nothing to splice.

use crate::graph::Graph;
use crate::node::{Edge, EdgeInsert, Kind, Laid, Node};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The room the tail `+` takes past the last card.
pub const SEAM_END_ROOM: f64 = 40.0;

/// One `+` of a glued row, in the row's own coordinates: a vertical seam `h` tall at `x`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct RowSeam {
    pub insert: EdgeInsert,
    pub x: f64,
    pub y: f64,
    pub h: f64,
    /// The row's tail: drawn past the last card and always shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<bool>,
    /// The seam's second button: a stack under its left card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<EdgeInsert>,
}

/// The seams of a row, which cables are glued, and the width the row now needs.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SeamsAnswer {
    pub seams: Vec<RowSeam>,
    /// The ids of the cables between two touching cards: never drawn, but the signal still flows through them.
    pub glued: Vec<String>,
    pub width: f64,
}

/// A closed box's only plug on that side, when it has exactly one: what an arrow's end must resolve to.
fn port_on(n: &Node, outgoing: bool) -> Option<String> {
    n.assembly_of.as_ref()?;
    let jacks = n.jacks.as_ref()?;
    let side = if outgoing { &jacks.outputs } else { &jacks.inputs };
    let named: Vec<&str> = side
        .iter()
        .filter_map(|j| j.name.as_deref().filter(|n| !n.is_empty()))
        .collect();
    (named.len() == 1).then(|| named[0].to_owned())
}

/// The chains.k7x name a jack click has in hand, plus `#port`; an assembly answers with the member behind the plug.
pub fn jack_ref(n: &Node, port: Option<&str>) -> String {
    if let (Some(a), Some(port)) = (&n.assembly_of, port.filter(|p| !p.is_empty()))
        && let Some(plug) = a.plugs.get(port)
    {
        return match plug.port.as_deref().filter(|p| !p.is_empty()) {
            Some(p) => format!("{}#{p}", plug.node),
            None => plug.node.clone(),
        };
    }
    let name = if matches!(n.kind, Kind::Module | Kind::Assembly) {
        n.module.as_deref().filter(|m| !m.is_empty()).unwrap_or(&n.label)
    } else {
        &n.label
    };
    match port.filter(|p| !p.is_empty()) {
        Some(p) => format!("{name}#{p}"),
        None => name.to_owned(),
    }
}

/// Whether an arrow maps to an insert: at least one end a module, or the two built-in ends of a master run.
pub fn insert_for(source: &str, target: &str, by_id: &HashMap<&str, &Node>) -> Option<EdgeInsert> {
    let (s, t) = (by_id.get(source)?, by_id.get(target)?);
    // Sink to sink is the empty master run: the one line every template writes to be dropped on.
    let master_run = s.kind == Kind::Sink && t.kind == Kind::Sink;
    let spliceable = |n: &Node| n.kind == Kind::Module || n.assembly_of.is_some();
    if !spliceable(s) && !spliceable(t) && !master_run {
        return None;
    }
    // The box's own name is never written into a chain: the arrow's real end is the member behind its plug.
    Some(EdgeInsert {
        before: jack_ref(s, port_on(s, true).as_deref()),
        after: jack_ref(t, port_on(t, false).as_deref()),
        stack: None,
    })
}

/// Every arrow's insert in one call, in the order given.
pub fn inserts_for(nodes: &[Node], pairs: &[(String, String)]) -> Vec<Option<EdgeInsert>> {
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    pairs.iter().map(|(s, t)| insert_for(s, t, &by_id)).collect()
}

/// Two cards meet when one ends where the next starts, or one pixel past it: a glued row shares a border.
fn meets(end: f64, start: f64) -> bool {
    end == start || end == start + 1.0
}

/// A column's box: as wide as its widest card, from its top card's top to its bottom card's bottom.
fn extent(col: &[&Node]) -> (f64, f64, f64, f64) {
    let y = col.iter().map(|n| n.y.unwrap_or(0.0)).fold(f64::INFINITY, f64::min);
    let bottom = col
        .iter()
        .map(|n| n.y.unwrap_or(0.0) + n.h)
        .fold(f64::NEG_INFINITY, f64::max);
    let w = col.iter().map(|n| n.w).fold(f64::NEG_INFINITY, f64::max);
    (col[0].x.unwrap_or(0.0), y, w, bottom - y)
}

/// The seams of a row, its tail when `exit` names the arrow out of the last card, and with `branch` a branch button on each.
pub fn row_seams(row: &Laid, exit: Option<EdgeInsert>, branch: bool) -> SeamsAnswer {
    let by_id: HashMap<&str, &Node> = row.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let linked: HashSet<String> = row
        .edges
        .iter()
        .filter(|e| e.automation.is_none())
        .map(|e| format!("{}>{}", e.source, e.target))
        .collect();
    // Columns by x, left to right, each top to bottom.
    let mut by_x: Vec<(f64, Vec<&Node>)> = Vec::new();
    for n in &row.nodes {
        let x = n.x.unwrap_or(0.0);
        match by_x.iter_mut().find(|(k, _)| *k == x) {
            Some((_, c)) => c.push(n),
            None => by_x.push((x, vec![n])),
        }
    }
    by_x.sort_by(|a, b| a.0.total_cmp(&b.0));
    let columns: Vec<Vec<&Node>> = by_x
        .into_iter()
        .map(|(_, mut c)| {
            c.sort_by(|p, q| p.y.unwrap_or(0.0).total_cmp(&q.y.unwrap_or(0.0)));
            c
        })
        .collect();
    let mut seams: Vec<RowSeam> = Vec::new();
    let mut touching: HashSet<String> = HashSet::new();
    let mut touch = |a: &Node, b: &Node| -> bool {
        let key = format!("{}>{}", a.id, b.id);
        if !linked.contains(&key) {
            return false;
        }
        touching.insert(key);
        true
    };
    for col in &columns {
        for i in 1..col.len() {
            if meets(col[i - 1].y.unwrap_or(0.0) + col[i - 1].h, col[i].y.unwrap_or(0.0)) {
                touch(col[i - 1], col[i]);
            }
        }
    }
    for i in 1..columns.len() {
        let (a, b) = (extent(&columns[i - 1]), extent(&columns[i]));
        if !meets(a.0 + a.2, b.0) {
            continue;
        }
        // The arrow across the seam, from whichever card of the left column to whichever of the right.
        for p in &columns[i - 1] {
            for q in &columns[i] {
                if !touch(p, q) {
                    continue;
                }
                let insert = insert_for(&p.id, &q.id, &by_id);
                let y = a.1.max(b.1);
                let h = (a.1 + a.3).min(b.1 + b.3) - y;
                // A branch stacks under a module: a lane is a source, and nothing goes under it, nor under an input.
                let input = p.monitor.is_some() || p.device.is_some();
                let under = match &p.module {
                    Some(m) if branch && p.kind == Kind::Module && !m.is_empty() && !input => Some(EdgeInsert {
                        before: m.clone(),
                        after: m.clone(),
                        stack: Some(true),
                    }),
                    _ => None,
                };
                if let Some(insert) = insert
                    && h > 0.0
                {
                    seams.push(RowSeam {
                        insert,
                        x: b.0,
                        y,
                        h,
                        end: None,
                        branch: under,
                    });
                }
            }
        }
    }
    let glued: Vec<String> = row
        .edges
        .iter()
        .filter(|e| touching.contains(&format!("{}>{}", e.source, e.target)))
        .map(|e: &Edge| e.id.clone())
        .collect();
    let mut width = row.width;
    if let (Some(last), Some(exit)) = (columns.last(), exit) {
        let tail = extent(last);
        seams.push(RowSeam {
            insert: exit,
            x: tail.0 + tail.2 + SEAM_END_ROOM / 2.0,
            y: tail.1,
            h: tail.3,
            end: Some(true),
            branch: None,
        });
        width += SEAM_END_ROOM;
    }
    SeamsAnswer { seams, glued, width }
}

/// The arrow out of `module` when it ends a chain with one way out: what the row's tail splices into.
pub fn chain_exit(graph: &Graph, module: &str) -> Option<EdgeInsert> {
    for chain in graph.chains.values() {
        if chain.steps.last().map(|s| s.module.as_str()) != Some(module) {
            continue;
        }
        // Two ways out is a fork: the tail cannot pick one for you.
        let [out] = chain.out.as_slice() else { return None };
        // A chain target is entered at its first module; an empty one is drawn as its own node, under its name.
        let after = graph
            .chains
            .get(&out.to)
            .and_then(|c| c.steps.first())
            .map_or(out.to.as_str(), |s| s.module.as_str());
        let before = match out.from_port.as_deref().filter(|p| !p.is_empty()) {
            Some(p) => format!("{module}#{p}"),
            None => module.to_owned(),
        };
        return Some(EdgeInsert {
            before,
            after: after.to_owned(),
            stack: None,
        });
    }
    None
}

/// The tail's arrow: into a card lifted off the row's end when `full` still holds one, else out of the last card's chain.
pub fn row_exit(full: &Laid, shown: &Laid, graph: &Graph) -> Option<EdgeInsert> {
    let last = shown.nodes.last()?;
    let by_id: HashMap<&str, &Node> = full.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let at = full.nodes.iter().position(|n| n.id == last.id).map_or(0, |i| i + 1);
    if let Some(next) = full.nodes.get(at)
        && full.edges.iter().any(|e| e.source == last.id && e.target == next.id)
    {
        return insert_for(&last.id, &next.id, &by_id);
    }
    last.module.as_deref().and_then(|m| chain_exit(graph, m))
}

#[cfg(test)]
mod tests;
