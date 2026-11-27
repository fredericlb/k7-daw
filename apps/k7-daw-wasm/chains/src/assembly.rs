// assembly.rs — folding a project's assemblies into the built graph: a box instead of its members when closed, a titled frame when open.
// A reader would get wrong that this changes no cable's meaning: it only moves where a cable lands, and sockets are re-typed after.

use crate::geom::{jacks_for, node_size};
use crate::graph::{Graph, Plug};
use crate::node::{AssemblyOf, Edge, Jacks, Kind, Node, PlugAt, Port, Signal};
use crate::node_ui::rack_grid;
use crate::panel::{Panel, Widget};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// The id an assembly's card has, in the family of the `mod:` / `trk:` / `snk:` ids the build mints.
pub fn assembly_id(name: &str) -> String {
    format!("asm:{name}")
}

/// The panel a closed box carries: one knob per promoted param, bound by the name the template gave it.
fn assembly_ui(names: &[&String]) -> Panel {
    let (cols, rows) = rack_grid(names.len());
    let widgets = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let (col, row) = ((i as f64) % cols, ((i as f64) / cols).floor());
            (format!("asm:{name}"), Widget::at("knob", col, row).bound(name, name))
        })
        .collect();
    Panel {
        cols,
        rows,
        widgets,
        ..Panel::default()
    }
}

/// A node's own name in the chains file: a member is matched by it.
fn name_of(n: &Node) -> Option<&str> {
    matches!(n.kind, Kind::Module | Kind::Track).then(|| n.module.as_deref().unwrap_or(&n.label))
}

/// The plug a cable off (or onto) this member leaves by: the one naming the port, else the port-less one.
fn plug_for<'a>(plugs: &'a [Plug], node: &str, port: Option<&str>) -> Option<&'a Plug> {
    let mine: Vec<&Plug> = plugs.iter().filter(|p| p.node == node).collect();
    mine.iter()
        .find(|p| p.port.as_deref() == port)
        .or_else(|| mine.iter().find(|p| p.port.as_deref().is_none_or(str::is_empty)))
        .copied()
}

fn plug_port(p: &Plug) -> Port {
    Port {
        name: p.name.clone(),
        kind: p.kind.unwrap_or(Signal::Audio),
        mandatory: (p.mandatory == Some(false)).then_some(false),
    }
}

/// Folds every assembly the graph declares into the built nodes and cables.
pub fn fold_assemblies(
    graph: &Graph,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    open: &HashSet<String>,
) -> (Vec<Node>, Vec<Edge>) {
    let Some(declared) = graph.assemblies.as_ref().filter(|a| !a.is_empty()) else {
        return (nodes, edges);
    };
    let mut by_name: HashMap<&str, usize> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        if let Some(name) = name_of(n).filter(|n| !n.is_empty()) {
            by_name.entry(name).or_insert(i);
        }
    }
    // Which box each member node id belongs to, and the box card each one gets.
    let mut box_of: HashMap<String, String> = HashMap::new();
    let mut boxes: Vec<Node> = Vec::new();
    for (id, a) in declared {
        let members: Vec<&Node> = a
            .members
            .iter()
            .filter_map(|m| by_name.get(m.as_str()))
            .map(|i| &nodes[*i])
            .collect();
        // A box whose members are all absent is a box about nothing: the diagnostics already said so.
        if members.is_empty() {
            continue;
        }
        let is_open = open.contains(id);
        for m in &members {
            box_of.insert(m.id.clone(), id.clone());
        }
        let mut plugs: IndexMap<String, PlugAt> = IndexMap::new();
        for p in a.inputs.iter().flatten().chain(a.outputs.iter().flatten()) {
            plugs.insert(
                p.name.clone(),
                PlugAt {
                    node: p.node.clone(),
                    port: p.port.clone().filter(|p| !p.is_empty()),
                },
            );
        }
        // A closed box wears the plugs and the promoted knobs; an open one is a frame and wears neither.
        let ui = a
            .params
            .as_ref()
            .filter(|p| !is_open && !p.is_empty())
            .map(|p| assembly_ui(&p.keys().collect::<Vec<_>>()));
        let mut b = Node::plain(&assembly_id(id), &a.label, Kind::Assembly);
        // The instance's name, so a pin and `graph move` address the box and not the template's label.
        b.module = Some(id.clone());
        b.missing = a.missing == Some(true);
        if !is_open {
            b.inputs = Some(a.inputs.iter().flatten().map(plug_port).collect());
            b.outputs = Some(a.outputs.iter().flatten().map(plug_port).collect());
        }
        (b.w, b.h) = node_size(ui.as_ref(), false);
        b.ui = ui;
        b.assembly_of = Some(AssemblyOf {
            uses: a.uses.clone(),
            open: is_open,
            members: a.members.clone(),
            plugs,
            params: a.params.clone(),
            missing: (a.missing == Some(true)).then_some(true),
        });
        // Where the earliest member stood: the box replaces the whole column.
        b.rank = members.iter().filter_map(|m| m.rank).reduce(f64::min);
        boxes.push(b);
    }
    if boxes.is_empty() {
        return (nodes, edges);
    }
    let names: HashMap<String, String> = nodes
        .iter()
        .filter_map(|n| name_of(n).map(|name| (n.id.clone(), name.to_owned())))
        .collect();
    let mut out_nodes: Vec<Node> = Vec::new();
    for n in nodes {
        match box_of.get(&n.id) {
            None => out_nodes.push(n),
            // An open box keeps its members, tagged so the layout can lay them out inside the frame.
            Some(b) if open.contains(b) => out_nodes.push(Node {
                assembly: Some(b.clone()),
                ..n
            }),
            Some(_) => {}
        }
    }
    out_nodes.extend(boxes);

    let mut out_edges: IndexMap<String, Edge> = IndexMap::new();
    for e in edges {
        let from = box_of.get(&e.source);
        let to = box_of.get(&e.target);
        let same = from.is_some() && from == to;
        // Both ends inside one closed box: the cable is what the box is hiding.
        if same && from.is_some_and(|f| !open.contains(f)) {
            continue;
        }
        let mut next = e.clone();
        // …and inside an open one it is the box's own wiring, which is what the frame is showing.
        if same {
            next.assembly = from.cloned();
        }
        if let Some(f) = from.filter(|f| !open.contains(*f)) {
            let a = graph.assemblies.as_ref().and_then(|m| m.get(f));
            let plug = plug_for(
                a.and_then(|a| a.outputs.as_deref()).unwrap_or_default(),
                names.get(&e.source).map_or("", String::as_str),
                e.source_port.as_deref(),
            );
            next.source = assembly_id(f);
            next.source_port = plug.map(|p| p.name.clone());
        }
        if let Some(t) = to.filter(|t| !open.contains(*t)) {
            let a = graph.assemblies.as_ref().and_then(|m| m.get(t));
            let plug = plug_for(
                a.and_then(|a| a.inputs.as_deref()).unwrap_or_default(),
                names.get(&e.target).map_or("", String::as_str),
                e.target_port.as_deref(),
            );
            next.target = assembly_id(t);
            next.target_port = plug.map(|p| p.name.clone());
            // The slot is a rank among the bare cables reaching the old target; the box redraws its own.
            next.target_slot = None;
        }
        // Two members feeding one thing collapse to one cable off the box: the same plug, drawn once.
        let key = format!(
            "{}->{}#{}#{}",
            next.source,
            next.target,
            next.source_port.as_deref().unwrap_or_default(),
            next.target_port.as_deref().unwrap_or_default()
        );
        out_edges.entry(key.clone()).or_insert_with(|| Edge { id: key, ..next });
    }
    let mut edges: Vec<Edge> = out_edges.into_values().collect();
    with_sockets(&mut out_nodes, &mut edges);
    (out_nodes, edges)
}

/// Re-runs the socket pass over the boxes alone: their cables moved, so their jacks are a new question.
fn with_sockets(nodes: &mut [Node], edges: &mut [Edge]) {
    let mut fed: HashMap<String, HashSet<String>> = HashMap::new();
    let mut left: HashMap<String, HashSet<String>> = HashMap::new();
    let mut bare: HashMap<String, usize> = HashMap::new();
    let mut arriving: HashMap<String, Signal> = HashMap::new();
    let mut leaving: HashMap<String, Option<Signal>> = HashMap::new();
    for e in edges.iter_mut() {
        if e.automation.is_some() {
            continue;
        }
        fed.entry(e.target.clone())
            .or_default()
            .insert(e.target_port.clone().unwrap_or_default());
        if e.target_port.as_deref().is_none_or(str::is_empty) {
            let n = bare.entry(e.target.clone()).or_insert(0);
            e.target_slot = Some(*n as f64);
            *n += 1;
        }
        left.entry(e.source.clone())
            .or_default()
            .insert(e.source_port.clone().unwrap_or_default());
        if let Some(s) = e.signal {
            arriving.entry(e.target.clone()).or_insert(s);
        }
        leaving.entry(e.source.clone()).or_insert(e.signal);
    }
    let none = HashSet::new();
    for n in nodes.iter_mut().filter(|n| n.kind == Kind::Assembly) {
        // An open box is a frame: nothing plugs into it, because its members are showing the real sockets.
        if n.assembly_of.as_ref().is_some_and(|a| a.open) {
            n.jacks = Some(Jacks::default());
            continue;
        }
        // A box's sockets are its plugs and nothing else: a template that declares none has said the box takes none.
        let ports = |ports: &Option<Vec<Port>>, taken, fallback, grow, count| match ports.as_deref() {
            Some(p) if !p.is_empty() => jacks_for(Some(p), taken, fallback, grow, false, None, count),
            _ => Vec::new(),
        };
        n.jacks = Some(Jacks {
            inputs: ports(
                &n.inputs,
                fed.get(&n.id).unwrap_or(&none),
                arriving.get(&n.id).copied(),
                true,
                bare.get(&n.id).copied().unwrap_or(0),
            ),
            outputs: ports(
                &n.outputs,
                left.get(&n.id).unwrap_or(&none),
                leaving.get(&n.id).copied().flatten(),
                false,
                0,
            ),
        });
    }
}
