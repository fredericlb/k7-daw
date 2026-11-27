// graph_layout.rs — the Chains graph's layout: rust-sugiyama places the cards, then the rows, the pins and the pushes shape them.
// A reader would get wrong that the engine's own ranks are not the drawn ones: each card's `rank` from the build sets its row (arrange.rs).

pub mod arrange;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_arrange;

use crate::curve_placement::{PlaceEdge, curve_targets, curves_left_of_targets, curves_under_left};
use crate::geom::{FRAME_HEAD, FRAME_PAD, NODE_H, NODE_W};
use crate::node::{Edge, Kind, Laid, Node, Pt};
use arrange::{
    NODE_SEP, RANK_SEP, boxes, master_run_aside, member_of, push_aside_frames, push_below_inputs, snap_to_rank_rows, x,
    y,
};
use indexmap::IndexMap;
use rust_sugiyama::{configure::Config, from_vertices_and_edges};
use std::collections::{HashMap, HashSet};

/// The breathing room around the drawing, and what the measured extent adds back.
const GRAPH_MARGIN: f64 = 16.0;

type Memory<'a> = Option<&'a HashMap<String, Pt>>;

/// Lay a built graph out; `positions` are the file's pins by name, `prev` where each card was drawn last.
pub fn layout(nodes: Vec<Node>, raw: Vec<Edge>, positions: Memory, prev: Memory) -> Laid {
    let mut inside: IndexMap<String, Vec<Node>> = IndexMap::new();
    for n in &nodes {
        if let Some(a) = member_of(n) {
            inside.entry(a.to_owned()).or_default().push(n.clone());
        }
    }
    if inside.is_empty() {
        layout_flat(nodes, raw, positions, prev)
    } else {
        layout_nested(nodes, raw, inside, positions, prev)
    }
}

/// An OPEN box's members laid out by a RECURSIVE pass, the outer graph seeing one card that size.
fn layout_nested(
    nodes: Vec<Node>,
    raw: Vec<Edge>,
    inside: IndexMap<String, Vec<Node>>,
    positions: Memory,
    prev: Memory,
) -> Laid {
    let mut outer_nodes: Vec<Node> = nodes.into_iter().filter(|n| member_of(n).is_none()).collect();
    let mut box_id_of: HashMap<String, String> = HashMap::new();
    let mut laid_inside: IndexMap<String, Laid> = IndexMap::new();
    for (name, members) in &inside {
        let ids: HashSet<&str> = members.iter().map(|m| m.id.as_str()).collect();
        let Some(bi) = outer_nodes
            .iter()
            .position(|n| n.kind == Kind::Assembly && n.module.as_deref() == Some(name))
        else {
            // A member whose box is not on the graph is nobody's; it lays out with everything else.
            outer_nodes.extend(members.iter().cloned());
            continue;
        };
        let box_id = outer_nodes[bi].id.clone();
        for m in members {
            box_id_of.insert(m.id.clone(), box_id.clone());
        }
        // `assembly` is stripped for the inner pass, or the recursion would look for a box inside itself.
        let stripped = members.iter().cloned().map(|mut m| {
            m.assembly = None;
            m
        });
        let inner_edges = raw
            .iter()
            .filter(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str()));
        let laid = layout_flat(stripped.collect(), inner_edges.cloned().collect(), None, None);
        outer_nodes[bi].w = NODE_W.max(laid.width - laid.left + FRAME_PAD * 2.0);
        outer_nodes[bi].h = NODE_H.max(laid.height - laid.top + FRAME_PAD + FRAME_HEAD);
        laid_inside.insert(name.clone(), laid);
    }
    // The layout is shown the cables as they reach the BOX; the drawing gets them back pointing at the member.
    let mut seen: HashSet<String> = HashSet::new();
    let mut outer_edges: Vec<Edge> = Vec::new();
    for e in &raw {
        let source = box_id_of.get(&e.source).unwrap_or(&e.source).clone();
        let target = box_id_of.get(&e.target).unwrap_or(&e.target).clone();
        if source == target {
            continue;
        }
        let key = format!("{source}->{target}");
        if !seen.insert(key.clone()) {
            continue;
        }
        outer_edges.push(Edge {
            id: key,
            source,
            target,
            ..e.clone()
        });
    }
    // The push runs BEFORE the members are placed, so a frame that stepped aside takes them with it.
    let outer = push_aside_frames(layout_flat(outer_nodes, outer_edges, positions, prev).nodes);
    let mut members: Vec<Node> = Vec::new();
    for (name, laid) in &laid_inside {
        let Some(b) = outer
            .iter()
            .find(|n| n.kind == Kind::Assembly && n.module.as_deref() == Some(name))
        else {
            continue;
        };
        let dx = x(b) + FRAME_PAD - laid.left;
        let dy = y(b) + FRAME_HEAD - laid.top;
        for m in &laid.nodes {
            let mut m = m.clone();
            m.assembly = Some(name.clone());
            m.x = Some(x(&m) + dx);
            m.y = Some(y(&m) + dy);
            members.push(m);
        }
    }
    let mut out_nodes = outer;
    out_nodes.extend(members);
    let edges = edges_between(&out_nodes, &raw);
    finish(out_nodes, edges)
}

/// Every cable with the two centres of its cards, never zero points: the renderer skips an edge with fewer than two.
fn edges_between(nodes: &[Node], raw: &[Edge]) -> Vec<Edge> {
    let placed: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let centre = |n: &Node| Pt {
        x: x(n) + n.w / 2.0,
        y: y(n) + n.h / 2.0,
    };
    raw.iter()
        .map(|e| {
            let points = match (placed.get(e.source.as_str()), placed.get(e.target.as_str())) {
                (Some(s), Some(t)) => vec![centre(s), centre(t)],
                _ => Vec::new(),
            };
            Edge {
                weight: None,
                side: None,
                glued: None,
                points: Some(points),
                ..e.clone()
            }
        })
        .collect()
}

/// The extent is measured on the cards, 0 rather than infinity when empty, and `left`/`top` are never positive.
fn finish(nodes: Vec<Node>, edges: Vec<Edge>) -> Laid {
    let extent = |of: &dyn Fn(&Node) -> f64| {
        if nodes.is_empty() {
            0.0
        } else {
            nodes.iter().map(of).fold(f64::NEG_INFINITY, f64::max) + GRAPH_MARGIN
        }
    };
    let least = |of: &dyn Fn(&Node) -> f64| {
        if nodes.is_empty() {
            0.0
        } else {
            (nodes.iter().map(of).fold(f64::INFINITY, f64::min) - GRAPH_MARGIN).min(0.0)
        }
    };
    let (width, height) = (extent(&|n| x(n) + n.w), extent(&|n| y(n) + n.h));
    let (left, top) = (least(&|n| x(n)), least(&|n| y(n)));
    Laid {
        nodes,
        edges,
        width,
        height,
        left,
        top,
    }
}

/// The cables with the ones that close a cycle left out: the engine never ends on a dense cyclic graph, so it is only shown a DAG.
fn without_back_edges(n: usize, edges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut out: Vec<Vec<(u32, usize)>> = vec![Vec::new(); n];
    for (k, &(s, t)) in edges.iter().enumerate() {
        out[s as usize].push((t, k));
    }
    // 0 unseen, 1 on the walk, 2 done.
    let mut state = vec![0u8; n];
    let mut back: HashSet<usize> = HashSet::new();
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&mut (v, ref mut i)) = stack.last_mut() {
            if let Some(&(t, k)) = out[v].get(*i) {
                *i += 1;
                match state[t as usize] {
                    0 => {
                        state[t as usize] = 1;
                        stack.push((t as usize, 0));
                    }
                    1 => {
                        back.insert(k);
                    }
                    _ => {}
                }
            } else {
                state[v] = 2;
                stack.pop();
            }
        }
    }
    edges
        .iter()
        .enumerate()
        .filter(|(k, _)| !back.contains(k))
        .map(|(_, e)| *e)
        .collect()
}

/// The engine's top-left for each card: components side by side, each from the margin, cards centred in their layer.
fn engine_places(nodes: &[Node], raw: &[Edge]) -> Vec<(f64, f64)> {
    let index: HashMap<&str, u32> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i as u32))
        .collect();
    // Self-loops and repeated cables tell the engine nothing, and it takes neither.
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let mut edges: Vec<(u32, u32)> = Vec::new();
    for e in raw {
        if let (Some(&s), Some(&t)) = (index.get(e.source.as_str()), index.get(e.target.as_str()))
            && s != t
            && seen.insert((s, t))
        {
            edges.push((s, t));
        }
    }
    // The engine pads each card by one spacing; the rank gap is RANK_SEP, so the height gives back the difference.
    let edges = without_back_edges(nodes.len(), &edges);
    let sizes: Vec<(u32, (f64, f64))> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u32, (n.w, n.h + RANK_SEP - NODE_SEP)))
        .collect();
    let config = Config {
        vertex_spacing: NODE_SEP,
        ..Config::default()
    };
    let mut components = from_vertices_and_edges(&sizes, &edges, &config);
    components.sort_by_key(|(c, _, _)| c.iter().map(|(i, _)| *i).min().unwrap_or(usize::MAX));
    let mut out: Vec<Option<(f64, f64)>> = vec![None; nodes.len()];
    let mut cursor = GRAPH_MARGIN;
    for (comp, _, _) in &components {
        let left = comp
            .iter()
            .map(|(i, (cx, _))| cx - nodes[*i].w / 2.0)
            .fold(f64::INFINITY, f64::min);
        let top = comp
            .iter()
            .map(|(i, (_, cy))| cy - nodes[*i].h / 2.0)
            .fold(f64::INFINITY, f64::min);
        let mut right = cursor;
        for (i, (cx, cy)) in comp {
            let n = &nodes[*i];
            let px = cx - n.w / 2.0 - left + cursor;
            right = right.max(px + n.w);
            out[*i] = Some((px, cy - n.h / 2.0 - top + GRAPH_MARGIN));
        }
        cursor = right + NODE_SEP;
    }
    // A card the engine did not return stands on the first row, after everything it did place.
    let mut tail = (cursor, GRAPH_MARGIN);
    out.into_iter()
        .enumerate()
        .map(|(i, p)| {
            p.unwrap_or_else(|| {
                let at = tail;
                tail.0 += nodes[i].w + NODE_SEP;
                at
            })
        })
        .collect()
}

fn layout_flat(mut nodes: Vec<Node>, raw: Vec<Edge>, positions: Memory, prev: Memory) -> Laid {
    let places = engine_places(&nodes, &raw);
    for (n, (px, py)) in nodes.iter_mut().zip(places) {
        n.x = Some(px);
        n.y = Some(py);
    }
    let curve_ids: HashSet<&str> = nodes
        .iter()
        .filter(|n| n.kind == Kind::Curve)
        .map(|n| n.id.as_str())
        .collect();
    let place_edges: Vec<PlaceEdge> = raw
        .iter()
        .map(|e| PlaceEdge {
            source: e.source.clone(),
            target: e.target.clone(),
            automation: e.automation.is_some().then_some(true),
        })
        .collect();
    let curves = curve_targets(&place_edges, |id| curve_ids.contains(id));
    // A curve's engine spot is dropped: it goes beside its target, and the row sweep makes the room.
    let beside = curves_left_of_targets(&boxes(&nodes), &curves);
    apply(&mut nodes, beside);
    let rank_of: HashMap<String, f64> = nodes.iter().filter_map(|n| n.rank.map(|r| (n.id.clone(), r))).collect();
    let mut out = snap_to_rank_rows(nodes, &rank_of, &curves);
    // BEFORE the pins: where the master column sits is a preference, and a pin outranks a preference.
    out = master_run_aside(out, &raw);
    if positions.is_some() || prev.is_some() {
        for n in &mut out {
            let named = if matches!(n.kind, Kind::Module | Kind::Assembly) {
                n.module.as_ref().unwrap_or(&n.label)
            } else {
                &n.label
            };
            if let Some(p) = positions
                .and_then(|m| m.get(named))
                .or_else(|| prev.and_then(|m| m.get(&n.id)))
            {
                n.x = Some(p.x);
                n.y = Some(p.y);
            }
        }
    }
    // AFTER the pins and the memory: a curve card goes where its target IS, at its left.
    let moved = curves_under_left(&boxes(&out), &curves);
    apply(&mut out, moved);
    // AFTER the pins, because a pin is the only thing that can put a card above what feeds it.
    out = push_below_inputs(out, &raw);
    let edges = edges_between(&out, &raw);
    finish(out, edges)
}

fn apply(nodes: &mut [Node], moved: Vec<crate::curve_placement::Moved>) {
    let at: HashMap<&str, &crate::curve_placement::Moved> = moved.iter().map(|m| (m.id.as_str(), m)).collect();
    for n in nodes {
        if let Some(m) = at.get(n.id.as_str()) {
            n.x = Some(m.x);
            n.y = Some(m.y);
        }
    }
}
