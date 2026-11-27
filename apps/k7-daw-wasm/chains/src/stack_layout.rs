// stack_layout.rs — the graph as stacks: one column per lane (its inputs, its card, every module on its path), the sum's last.
// A reader would get wrong that nothing is pinned or remembered: it is an arrangement of the same graph, and it answers only where cards go.

use crate::geom::MIX_OUT;
use crate::graph::Graph;
use crate::node::{Edge, Jacks, Kind, Node};
use crate::subgraph::{self, ChainView, GraphView, OutView, StepView};
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The air around the stacks, between two columns, and between a column's inputs and its head.
const STACK_MARGIN: f64 = 16.0;
const STACK_COL_GAP: f64 = 24.0;
const STACK_INPUT_GAP: f64 = 24.0;

/// Where a card goes, and the two things the arrangement changes on it: a bar's flag and its sockets.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct Placed {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar: Option<bool>,
    /// The sockets the card keeps: none, or only the outputs of a card forking to more than one place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jacks: Option<Jacks>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StackAnswer {
    pub nodes: Vec<Placed>,
    /// The lane of the column's right side a cable that skips cards runs down, by cable id.
    pub sides: Vec<(String, f64)>,
    pub width: f64,
    pub height: f64,
}

/// A source the graph starts from: a live input, a lens, a MIDI device, drawn over the lane it feeds.
fn is_input(n: &Node) -> bool {
    n.kind == Kind::Module && (n.monitor.is_some() || n.camera.is_some() || n.device.is_some())
}

#[derive(Default)]
struct Column {
    inputs: Vec<usize>,
    body: Vec<usize>,
}

fn view_of(graph: &Graph) -> GraphView {
    GraphView {
        chains: graph
            .chains
            .iter()
            .map(|(k, c)| {
                let steps = c.steps.iter().map(|s| StepView {
                    module: Some(s.module.clone()),
                    category: s.rest.get("category").and_then(|v| v.as_str()).map(str::to_owned),
                });
                let view = ChainView {
                    from: c.from.clone(),
                    out: c.out.iter().map(|o| OutView { to: o.to.clone() }).collect(),
                    steps: steps.collect(),
                };
                (k.clone(), view)
            })
            .collect(),
        video_tracks: graph.video_tracks.clone(),
    }
}

/// A lane's place in the song's order; a lane the order does not name goes last.
fn order(track_order: &[String], id: &str) -> usize {
    track_order.iter().position(|t| t == id).unwrap_or(track_order.len())
}

/// The stack arrangement of an already-built graph: lanes in `track_order`, whatever no lane reaches, the sum last.
pub fn stack_layout(nodes: &[Node], edges: &[Edge], graph: &Graph, track_order: &[String]) -> StackAnswer {
    let view = view_of(graph);
    let mut by_module: HashMap<&str, usize> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        if let Some(m) = n.module.as_deref().filter(|m| !m.is_empty()) {
            by_module.entry(m).or_insert(i);
        }
    }
    let by_id: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut placed: HashSet<usize> = HashSet::new();
    let mut columns: Vec<Column> = Vec::new();
    fn take(nodes: &[Node], placed: &mut HashSet<usize>, i: Option<usize>, into: &mut Vec<usize>) {
        let Some(i) = i else { return };
        if placed.contains(&i) || is_input(&nodes[i]) {
            return;
        }
        into.push(i);
        placed.insert(i);
    }
    // One column per lane, in the song's order: the inputs feeding it, its own card, each module on its path once.
    let mut lanes: Vec<usize> = (0..nodes.len()).filter(|i| nodes[*i].kind == Kind::Track).collect();
    let lane_name = |i: usize| {
        nodes[i]
            .module
            .as_deref()
            .filter(|m| !m.is_empty())
            .unwrap_or(&nodes[i].label)
            .to_owned()
    };
    lanes.sort_by_key(|i| order(track_order, &lane_name(*i)));
    for lane in lanes {
        let mut col = Column::default();
        take(nodes, &mut placed, Some(lane), &mut col.body);
        for m in subgraph::track_modules(&view, &lane_name(lane)) {
            take(nodes, &mut placed, by_module.get(m.as_str()).copied(), &mut col.body);
        }
        for e in edges {
            let src = by_id.get(e.source.as_str()).copied();
            if let Some(s) = src
                && e.target == nodes[lane].id
                && is_input(&nodes[s])
                && !placed.contains(&s)
            {
                col.inputs.push(s);
                placed.insert(s);
            }
        }
        columns.push(col);
    }
    // The sum's column, kept for last: the master run (the chain out of it), then where it lands.
    let mut mix_col = Column::default();
    let sink_named = |to: &str| nodes.iter().position(|n| n.kind == Kind::Sink && n.label == to);
    if let Some(mix) = sink_named(MIX_OUT) {
        take(nodes, &mut placed, Some(mix), &mut mix_col.body);
        for c in graph.chains.values().filter(|c| c.from.as_deref() == Some(MIX_OUT)) {
            for s in &c.steps {
                take(
                    nodes,
                    &mut placed,
                    by_module.get(s.module.as_str()).copied(),
                    &mut mix_col.body,
                );
            }
            for o in &c.out {
                take(nodes, &mut placed, sink_named(&o.to), &mut mix_col.body);
            }
        }
    }
    // What no lane reaches (a bus, a curve, a note) gets a column per chain, in drawing order.
    let mut rest: IndexMap<String, Vec<usize>> = IndexMap::new();
    for (i, n) in nodes.iter().enumerate() {
        if placed.contains(&i) || is_input(n) {
            continue;
        }
        rest.entry(n.chain.clone().unwrap_or_else(|| n.id.clone()))
            .or_default()
            .push(i);
    }
    columns.extend(rest.into_values().map(|body| Column {
        inputs: Vec::new(),
        body,
    }));
    // …an input feeding no lane in a column of its own…
    for (i, n) in nodes.iter().enumerate() {
        if is_input(n) && !placed.contains(&i) {
            columns.push(Column {
                inputs: vec![i],
                body: Vec::new(),
            });
        }
    }
    // …and the sum's, always the last: the sum is where every column ends up.
    if !mix_col.body.is_empty() {
        columns.push(mix_col);
    }
    let widths: Vec<f64> = columns
        .iter()
        .map(|c| c.inputs.iter().chain(&c.body).fold(0.0, |w: f64, i| w.max(nodes[*i].w)))
        .collect();
    let pile = |c: &Column| c.inputs.iter().map(|i| nodes[*i].h).sum::<f64>();
    // Every column's head on one line, under the tallest pile of inputs any column carries.
    let inputs_h = columns.iter().map(pile).fold(0.0, f64::max);
    let head = STACK_MARGIN
        + if inputs_h > 0.0 {
            inputs_h + STACK_INPUT_GAP
        } else {
            0.0
        };
    let mut out: Vec<Placed> = Vec::new();
    let mut at: HashMap<String, (f64, f64, f64, bool)> = HashMap::new();
    let (mut x, mut bottom) = (STACK_MARGIN, head);
    for (ci, c) in columns.iter().enumerate() {
        let mut place = |i: usize, y: f64, bar: bool| -> f64 {
            let n = &nodes[i];
            out.push(Placed {
                id: n.id.clone(),
                x,
                y,
                w: widths[ci],
                bar: bar.then_some(true),
                jacks: None,
            });
            at.insert(n.id.clone(), (x, y, n.h, bar));
            y + n.h
        };
        // The inputs sit right over their lane, as wide as its column, and end where the column begins.
        let mut iy = head - if inputs_h > 0.0 { STACK_INPUT_GAP } else { 0.0 } - pile(c);
        for i in &c.inputs {
            iy = place(*i, iy, true);
        }
        let mut cy = head;
        for i in &c.body {
            cy = place(*i, cy, false);
        }
        bottom = bottom.max(cy);
        x += widths[ci] + STACK_COL_GAP;
    }
    // A card forking to more than one place keeps its output sockets, so each cable leaves from its own.
    let mut leaving: HashMap<&str, usize> = HashMap::new();
    for e in edges.iter().filter(|e| e.automation.is_none()) {
        *leaving.entry(e.source.as_str()).or_insert(0) += 1;
    }
    for p in &mut out {
        let jacks = by_id.get(p.id.as_str()).and_then(|i| nodes[*i].jacks.as_ref());
        if leaving.get(p.id.as_str()).copied().unwrap_or(0) > 1
            && let Some(j) = jacks
        {
            p.jacks = Some(Jacks {
                inputs: Vec::new(),
                outputs: j.outputs.clone(),
            });
        }
    }
    // A cable between two cards of one column that are not stacked against each other skips the cards between them.
    let mut lanes_used: Vec<(f64, f64)> = Vec::new();
    let mut sides = Vec::new();
    for e in edges {
        let (Some(src), Some(tgt)) = (at.get(&e.source), at.get(&e.target)) else {
            continue;
        };
        if e.automation.is_some() || src.3 || src.0 != tgt.0 || tgt.1 == src.1 + src.2 {
            continue;
        }
        let slot = lanes_used.iter().position(|(sx, _)| *sx == src.0);
        let lane = slot.map_or(0.0, |s| lanes_used[s].1);
        match slot {
            Some(s) => lanes_used[s].1 = lane + 1.0,
            None => lanes_used.push((src.0, 1.0)),
        }
        sides.push((e.id.clone(), lane));
    }
    StackAnswer {
        nodes: out,
        sides,
        width: if columns.is_empty() {
            0.0
        } else {
            x - STACK_COL_GAP + STACK_MARGIN
        },
        height: bottom + STACK_MARGIN,
    }
}
