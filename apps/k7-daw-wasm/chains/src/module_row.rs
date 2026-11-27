// module_row.rs — named modules laid out side by side in the order given, no layout engine, and the two passes that line modules on one rank.
// A reader would get wrong that `align_ranks` and `space_modules` answer only the changes: the page already holds the nodes they read.

use crate::build::{BuildOptions, build_graph};
use crate::geom::{MIX_OUT, NODE_H, NODE_W, RENDER_OUT};
use crate::graph::Graph;
use crate::grid_fit::GridFit;
use crate::node::{Edge, Kind, Laid, Node, Pt};
use crate::node_ui::InputDevice;
use crate::widget_value::js_round;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// The room a row makes either side of a card's curve column, and the drop of its first curve under the card's top.
const CURVE_ROOM: f64 = 24.0;
const CURVE_DROP: f64 = 24.0;
/// The gap between two strips in a mixer row, wider than the layout's own spacing on purpose.
pub const STRIP_GAP: f64 = 24.0;
/// Matches the layout's margins, so the two frame alike.
const ROW_MARGIN: f64 = 16.0;

/// A node that only says where the signal comes from, drawn first and cabled into the first card.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct RowLead {
    pub id: String,
    pub label: String,
    pub sub: Option<String>,
}

/// What a host asks of a row beyond the modules: the shape, the cables, the lane it belongs to.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct RowOptions {
    /// Widen each card by this much: elbow room around a fixed-cell grid, not bigger controls.
    pub width_scale: Option<f64>,
    /// Carry the graph's own cables between these modules, and meet them on the sides.
    pub links: Option<bool>,
    /// The lane this row belongs to: its own card leads it.
    pub track: Option<String>,
    /// What feeds that lane, drawn before its card.
    pub inputs: Option<Vec<String>>,
    /// Every card at the width the full graph draws it, the lane's clips kept, all of them the tallest's height.
    pub as_drawn: Option<bool>,
    /// The air between two cards in px: 0 makes them touch, -1 glues them onto one shared border.
    pub gap: Option<f64>,
    /// Draw each of the graph's stacks as one column.
    pub stack: Option<bool>,
    /// Draw each card's curve cards in a column just before it.
    pub curves: Option<bool>,
    pub input_devices: Option<Vec<InputDevice>>,
    pub cameras: Option<Vec<String>>,
    pub midi_devices: Option<Vec<String>>,
    pub fit: Option<GridFit>,
    pub lead: Option<RowLead>,
    pub layout: Option<String>,
    /// The tab each card has open, by node id.
    pub tab_of: Option<HashMap<String, f64>>,
}

/// The nodes sized to the tab each has open, top edge kept.
pub fn with_open_tabs(nodes: &mut [Node], tab_of: &HashMap<String, f64>) {
    for n in nodes {
        let at = tab_of.get(&n.id).copied().unwrap_or(0.0) as usize;
        if let Some(h) = n.tab_h.as_ref().and_then(|t| t.get(at)).copied() {
            n.h = h;
        }
    }
}

/// The named modules laid out side by side in the order given, no layout engine.
pub fn module_row(graph: &Graph, modules: &[String], o: &RowOptions) -> Laid {
    let width_scale = o.width_scale.unwrap_or(1.0);
    let (links, as_drawn) = (o.links == Some(true), o.as_drawn == Some(true));
    let (curves, stack) = (o.curves == Some(true), o.stack == Some(true));
    let gap = o.gap.unwrap_or(STRIP_GAP);
    let tab_of = o.tab_of.clone().unwrap_or_default();
    // The same lists the full graph is built with, or its cards' dropdowns would offer nothing here.
    let opts = BuildOptions {
        input_devices: o.input_devices.clone(),
        cameras: o.cameras.clone(),
        midi_devices: o.midi_devices.clone(),
        fit: o.fit,
        layout: o.layout.clone(),
        ..BuildOptions::default()
    };
    let (mut nodes, edges) = build_graph(graph, &opts);
    // Each card as tall as the page it has open, before the row is laid.
    with_open_tabs(&mut nodes, &tab_of);
    let mut by_module: HashMap<&str, &Node> = HashMap::new();
    for n in &nodes {
        if n.kind == Kind::Module
            && let Some(m) = &n.module
        {
            by_module.entry(m).or_insert(n);
        }
    }
    let cards = |names: &[String]| -> Vec<&Node> {
        names
            .iter()
            .filter_map(|n| by_module.get(n.as_str()).copied())
            .collect()
    };
    let picked = cards(modules);
    let feeding = if o.track.is_some() {
        cards(o.inputs.as_deref().unwrap_or_default())
    } else {
        Vec::new()
    };
    // One height for the whole row, the tallest module card's: the lane's own is a clip list taller, and is dropped below.
    let tallest = feeding.iter().chain(&picked).fold(0.0_f64, |h, n| h.max(n.h));
    // Matched on the kind and not on the name: `trk:` exists so a lane and a module may share one.
    let lane: Option<&Node> = match &o.track {
        Some(t) if !picked.is_empty() || !feeding.is_empty() => nodes
            .iter()
            .find(|n| n.kind == Kind::Track && n.module.as_deref() == Some(t)),
        _ => None,
    };
    // The signal's origin, as a name and nothing else: a pass-through node, which no menu and no control acts on.
    let head: Option<Node> = match &o.lead {
        Some(l) if lane.is_none() && !picked.is_empty() => {
            let mut h = Node::plain(&l.id, &l.label, Kind::Chain);
            h.sub = l.sub.clone();
            (h.w, h.h) = (NODE_W, NODE_H);
            Some(h)
        }
        _ => None,
    };
    let head_id = head.as_ref().map(|h| h.id.as_str());
    let lane_id = lane.map(|l| l.id.as_str());
    let row: Vec<Node> = if let Some(l) = lane {
        feeding
            .iter()
            .copied()
            .chain(std::iter::once(l))
            .chain(picked.iter().copied())
            .cloned()
            .collect()
    } else if let Some(h) = &head {
        std::iter::once(h.clone())
            .chain(picked.iter().map(|n| (*n).clone()))
            .collect()
    } else {
        picked.iter().map(|n| (*n).clone()).collect()
    };
    // A stack, a subchain run in place, is one column, its cards one under another at its own heights.
    let mut stack_of: HashMap<&str, usize> = HashMap::new();
    if stack {
        for (i, st) in graph.stacks.iter().flatten().enumerate() {
            for m in st {
                stack_of.insert(m, i);
            }
        }
    }
    let stack_key = |n: &Node| n.module.as_deref().and_then(|m| stack_of.get(m)).copied();
    let mut columns: Vec<Vec<&Node>> = Vec::new();
    for n in &row {
        let k = stack_key(n);
        match columns.last_mut() {
            Some(last) if k.is_some() && stack_key(last[0]) == k => last.push(n),
            _ => columns.push(vec![n]),
        }
    }
    let column_h = |c: &Vec<&Node>| c.iter().map(|n| n.h).sum::<f64>();
    // As drawn, every lone card is as tall as the tallest column, the lane's clip list included: the row is one band.
    let line = if as_drawn {
        columns.iter().map(column_h).fold(0.0, f64::max)
    } else {
        tallest
    };
    let mut out: Vec<Node> = Vec::new();
    let mut in_row: HashSet<String> = HashSet::new();
    // Each card's curve cards, by the card they drive, read off the automation edges.
    let mut curves_of: HashMap<&str, Vec<&Node>> = HashMap::new();
    if curves {
        let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        for e in &edges {
            if e.automation.is_some()
                && let Some(c) = by_id.get(e.source.as_str()).filter(|c| c.kind == Kind::Curve)
            {
                curves_of.entry(&e.target).or_default().push(c);
            }
        }
    }
    let mut x = ROW_MARGIN;
    for col in &columns {
        // A card's width with its open tab, when its tabs differ: the first one until the host says otherwise.
        let own = |n: &Node| {
            n.tab_w
                .as_ref()
                .and_then(|t| t.get(tab_of.get(&n.id).copied().unwrap_or(0.0) as usize))
                .copied()
                .unwrap_or(n.w)
        };
        let width = |n: &Node| {
            if as_drawn {
                own(n)
            } else {
                js_round(own(n) * width_scale)
            }
        };
        let w = col.iter().map(|n| width(n)).fold(0.0, f64::max);
        // Where each card of the column sits, before any x: its curves hang off those tops.
        let mut y = ROW_MARGIN;
        let stacked = col.len() > 1;
        let tops: Vec<f64> = col
            .iter()
            .map(|n| {
                let h = if stacked {
                    n.h
                } else if as_drawn {
                    line
                } else {
                    tallest
                };
                let top = if stacked {
                    y
                } else {
                    ROW_MARGIN + js_round((line - h) / 2.0)
                };
                // A stack's cards share their border too when the row is glued past touching.
                y += h + gap.min(0.0);
                top
            })
            .collect();
        // The column's curves first, at its left and under its tops, spaced off both neighbours.
        let hung: Vec<(&Node, f64)> = col
            .iter()
            .enumerate()
            .flat_map(|(i, n)| {
                let top = tops[i];
                curves_of
                    .get(n.id.as_str())
                    .into_iter()
                    .flatten()
                    .map(move |c| (*c, top))
            })
            .collect();
        if !hung.is_empty() {
            x += CURVE_ROOM;
            let cw = hung.iter().map(|(c, _)| c.w).fold(0.0, f64::max);
            let mut below: Vec<(f64, f64)> = Vec::new();
            for (c, top) in &hung {
                let at = below.iter().position(|(t, _)| t == top);
                let cy = at.map_or(top + CURVE_DROP, |i| below[i].1);
                in_row.insert(c.id.clone());
                out.push(Node {
                    x: Some(x),
                    y: Some(cy),
                    ..(*c).clone()
                });
                let next = cy + c.h + CURVE_ROOM / 2.0;
                match at {
                    Some(i) => below[i].1 = next,
                    None => below.push((*top, next)),
                }
            }
            x += cw + CURVE_ROOM;
        }
        for (i, n) in col.iter().enumerate() {
            let is_head = Some(n.id.as_str()) == head_id;
            // The lead keeps its own small height, centred on the row: it is a label, not a card to fill the band.
            let h = if is_head || stacked {
                n.h
            } else if as_drawn {
                line
            } else {
                tallest
            };
            in_row.insert(n.id.clone());
            let mut card = (*n).clone();
            card.w = w;
            card.h = h;
            card.x = Some(x);
            card.y = Some(if is_head {
                ROW_MARGIN + js_round((line - n.h) / 2.0)
            } else {
                tops[i]
            });
            // Sockets are never plated here: the plates are pinned to edges this row does not use.
            card.jacks = None;
            // …and its event plugs only with its curves: what they fire is not drawn otherwise.
            if !curves {
                card.events = None;
            }
            // A lane's mini timeline is the arrangement, and it would set the height of every card beside it.
            if Some(n.id.as_str()) == lane_id && !as_drawn {
                card.clips = None;
                card.clips_more = None;
            }
            if links {
                card.flow = Some("across".to_owned());
            }
            out.push(card);
        }
        x += w + gap;
    }
    // The lead's cable into the first card, the one edge no chain file states.
    let first = head_id.and_then(|h| out.iter().find(|n| n.id != h));
    let two = || Some(vec![Pt { x: 0.0, y: 0.0 }, Pt { x: 0.0, y: 0.0 }]);
    let mut out_edges: Vec<Edge> = Vec::new();
    if links {
        if let (Some(h), Some(f)) = (&head, first) {
            out_edges.push(Edge {
                points: two(),
                ..Edge::new(&format!("{}->{}", h.id, f.id), &h.id, &f.id)
            });
        }
        // An automation landing on a card is kept with no curve to start from: its knob glows, and no cable is drawn.
        for e in edges
            .iter()
            .filter(|e| in_row.contains(&e.target) && (in_row.contains(&e.source) || e.automation.is_some()))
        {
            out_edges.push(Edge {
                id: e.id.clone(),
                source: e.source.clone(),
                target: e.target.clone(),
                label: e.label.clone(),
                automation: e.automation.clone(),
                signal: e.signal,
                target_port: e.target_port.clone(),
                source_port: e.source_port.clone(),
                target_slot: e.target_slot,
                points: two(),
                ..Edge::default()
            });
        }
    }
    let height = out
        .iter()
        .map(|n| n.y.unwrap_or(0.0) + n.h + if as_drawn { 0.0 } else { ROW_MARGIN })
        .fold(
            if as_drawn {
                ROW_MARGIN + line
            } else {
                line + ROW_MARGIN * 2.0
            },
            f64::max,
        );
    Laid {
        width: if out.is_empty() { 0.0 } else { x - gap + ROW_MARGIN },
        height: if out.is_empty() { 0.0 } else { height },
        left: 0.0,
        top: if as_drawn && !out.is_empty() { ROW_MARGIN } else { 0.0 },
        nodes: out,
        edges: out_edges,
    }
}

/// The few fields of a node the rank and spacing passes read.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct RankNode {
    pub id: String,
    pub kind: Option<Kind>,
    pub label: Option<String>,
    pub module: Option<String>,
    pub rank: Option<f64>,
    pub x: Option<f64>,
    pub w: Option<f64>,
}

/// The ranks that change when the named modules are pulled onto one rank, the deepest of theirs: `(id, rank)` pairs.
pub fn align_ranks(nodes: &[RankNode], modules: &[String]) -> Vec<(String, f64)> {
    let wanted: HashSet<&str> = modules.iter().map(String::as_str).collect();
    let in_row = |n: &RankNode| n.module.as_deref().is_some_and(|m| !m.is_empty() && wanted.contains(m));
    let count = nodes.iter().filter(|n| in_row(n)).count();
    // Nothing to align: one strip is already a row, and none at all leaves the graph as it was.
    if count < 2 {
        return Vec::new();
    }
    let shared = nodes
        .iter()
        .filter(|n| in_row(n))
        .fold(0.0, |r: f64, n| r.max(n.rank.unwrap_or(0.0)));
    nodes
        .iter()
        .filter_map(|n| {
            if in_row(n) {
                return Some((n.id.clone(), shared));
            }
            // …except the sums, drawn as sinks but standing where a source does, so they are not pushed.
            let label = n.label.as_deref().unwrap_or_default();
            (n.kind == Some(Kind::Sink) && label != MIX_OUT && label != RENDER_OUT && n.rank.unwrap_or(0.0) <= shared)
                .then(|| (n.id.clone(), shared + 1.0))
        })
        .collect()
}

/// Where the named modules move along x, equal gap, the caller's order; and the width the drawing then needs.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Spaced {
    pub moved: Vec<(String, f64)>,
    pub right: f64,
}

pub fn space_modules(nodes: &[RankNode], modules: &[String]) -> Option<Spaced> {
    let mut by_module: HashMap<&str, &RankNode> = HashMap::new();
    for n in nodes {
        if let Some(m) = n.module.as_deref().filter(|m| !m.is_empty()) {
            by_module.entry(m).or_insert(n);
        }
    }
    let row: Vec<&RankNode> = modules
        .iter()
        .filter_map(|m| by_module.get(m.as_str()).copied())
        .collect();
    if row.len() < 2 {
        return None;
    }
    // Start where the row already begins, so turning the view on does not carry the cards to the origin.
    let mut x = row.iter().map(|n| n.x.unwrap_or(0.0)).fold(f64::INFINITY, f64::min);
    let mut moved = Vec::new();
    for n in &row {
        moved.push((n.id.clone(), x));
        x += n.w.unwrap_or(0.0) + STRIP_GAP;
    }
    Some(Spaced {
        moved,
        right: x - STRIP_GAP + ROW_MARGIN,
    })
}
