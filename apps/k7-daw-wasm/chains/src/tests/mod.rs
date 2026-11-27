// tests — the cases the page's `ui-chains` tests held over its facades, run against the crate itself: the build, the rows, the stacks, the layout.
// A reader would get wrong that the nodes and cables are read as JSON: it is the shape the page receives, so a case reads as its TypeScript did.

mod assemblies;
mod clips;
mod events;
mod jacks;
mod lanes;
mod layout_rules;
mod rank;
mod rows;
mod seams;
mod stacks;
mod sums;
mod vertical;
mod wires;

use crate::assembly::fold_assemblies;
use crate::build::{BuildOptions, build_graph};
use crate::graph::Graph;
use crate::graph_layout::layout;
use crate::module_row::{RankNode, align_ranks};
use crate::node::{Edge, Node, Pt};
use crate::stack_layout::stack_layout;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// A graph document as the page sends it: the chains and the tracks, nothing else.
pub fn doc(chains: Value, tracks: Value) -> Value {
    json!({"chains": chains, "tracks": tracks, "warnings": []})
}

/// `doc` with more top-level fields merged in.
pub fn doc_with(chains: Value, tracks: Value, extra: Value) -> Value {
    let mut d = doc(chains, tracks);
    if let (Some(d), Some(x)) = (d.as_object_mut(), extra.as_object()) {
        d.extend(x.clone());
    }
    d
}

pub fn graph(v: &Value) -> Graph {
    serde_json::from_value(v.clone()).expect("a graph")
}

pub fn opts(v: Value) -> BuildOptions {
    serde_json::from_value(v).expect("options")
}

/// The cards and cables a build answers, as the JSON the page reads.
pub struct Built {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    typed: (Vec<Node>, Vec<Edge>),
}

pub fn built(g: &Value, o: Value) -> Built {
    let (nodes, edges) = build_graph(&graph(g), &opts(o));
    Built::of(nodes, edges)
}

/// A build with the graph's boxes folded, those named drawn open.
pub fn folded(g: &Value, open: &[&str]) -> Built {
    let gr = graph(g);
    let (nodes, edges) = build_graph(&gr, &BuildOptions::default());
    let open: HashSet<String> = open.iter().map(|s| (*s).to_owned()).collect();
    let (nodes, edges) = fold_assemblies(&gr, nodes, edges, &open);
    Built::of(nodes, edges)
}

impl Built {
    pub fn of(nodes: Vec<Node>, edges: Vec<Edge>) -> Built {
        let all = |v: Value| v.as_array().cloned().unwrap_or_default();
        Built {
            nodes: all(serde_json::to_value(&nodes).expect("nodes")),
            edges: all(serde_json::to_value(&edges).expect("edges")),
            typed: (nodes, edges),
        }
    }

    /// The build laid out, with the file's pins by name and where each card was drawn last.
    pub fn laid(&self, pins: Value, prev: Value) -> Value {
        self.laid_without(pins, prev, None)
    }

    /// As `laid`, the cable at index `skip` left out.
    pub fn laid_without(&self, pins: Value, prev: Value, skip: Option<usize>) -> Value {
        let memory = |v: Value| serde_json::from_value::<HashMap<String, Pt>>(v).ok();
        let (pins, prev) = (memory(pins), memory(prev));
        let kept = (0..self.typed.1.len()).filter(|i| Some(*i) != skip);
        let edges = kept.map(|i| self.typed.1[i].clone()).collect();
        let laid = layout(self.typed.0.clone(), edges, pins.as_ref(), prev.as_ref());
        serde_json::to_value(laid).expect("a laid graph")
    }

    /// The stack arrangement, each place merged onto its built card as the page does.
    pub fn stacked(&self, g: &Value, order: &[&str]) -> Value {
        let order: Vec<String> = order.iter().map(|s| (*s).to_owned()).collect();
        let answer = stack_layout(&self.typed.0, &self.typed.1, &graph(g), &order);
        let answer = serde_json::to_value(answer).expect("a stack answer");
        let merged = list(&answer["nodes"]).into_iter().map(|p| {
            let mut n = self.node(&text(&p["id"])).clone();
            n["jacks"] = p["jacks"].clone();
            for k in ["x", "y", "w", "bar"] {
                n[k] = p[k].clone();
            }
            n
        });
        json!({"nodes": merged.collect::<Vec<_>>(), "sides": answer["sides"], "width": answer["width"], "height": answer["height"]})
    }

    /// The graph laid out after the named modules were pulled onto one rank.
    pub fn laid_aligned(&self, modules: &[&str]) -> Value {
        let mut nodes = self.typed.0.clone();
        for (n, v) in nodes.iter_mut().zip(self.aligned(modules)) {
            n.rank = v["rank"].as_f64();
        }
        let laid = layout(nodes, self.typed.1.clone(), None, None);
        serde_json::to_value(laid).expect("a laid graph")
    }

    /// The built nodes with the named modules pulled onto one rank.
    pub fn aligned(&self, modules: &[&str]) -> Vec<Value> {
        let ranked: Vec<RankNode> = self
            .nodes
            .iter()
            .filter_map(|n| serde_json::from_value(n.clone()).ok())
            .collect();
        let modules: Vec<String> = modules.iter().map(|s| (*s).to_owned()).collect();
        let changed: HashMap<String, f64> = align_ranks(&ranked, &modules).into_iter().collect();
        let set = |n: &Value| {
            let mut n = n.clone();
            if let Some(r) = changed.get(&text(&n["id"])) {
                n["rank"] = json!(r);
            }
            n
        };
        self.nodes.iter().map(set).collect()
    }

    pub fn parts(&self) -> (Vec<Node>, Vec<Edge>) {
        self.typed.clone()
    }

    pub fn typed(&self, id: &str) -> &Node {
        self.typed
            .0
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("no node {id}"))
    }

    pub fn find(&self, id: &str) -> Option<&Value> {
        self.nodes.iter().find(|n| n["id"] == id)
    }

    pub fn node(&self, id: &str) -> &Value {
        self.find(id).unwrap_or_else(|| panic!("no node {id}"))
    }

    pub fn of_kind(&self, kind: &str) -> Vec<&Value> {
        self.nodes.iter().filter(|n| n["kind"] == kind).collect()
    }

    pub fn ids(&self) -> Vec<String> {
        self.nodes.iter().map(|n| text(&n["id"])).collect()
    }

    /// Every cable as `source->target`, sorted.
    pub fn pairs(&self) -> Vec<String> {
        let mut p: Vec<String> = self.edges.iter().map(pair).collect();
        p.sort();
        p
    }

    pub fn edge_from(&self, source: &str) -> Option<&Value> {
        self.edges.iter().find(|e| e["source"] == source)
    }
}

pub fn pair(e: &Value) -> String {
    format!("{}->{}", text(&e["source"]), text(&e["target"]))
}

pub fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

pub fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

/// The card of a laid or built list by id.
pub fn card<'a>(nodes: &'a [Value], id: &str) -> &'a Value {
    nodes
        .iter()
        .find(|n| n["id"] == id)
        .unwrap_or_else(|| panic!("no card {id}"))
}

/// The first card of a list drawn for the module of that name.
pub fn module<'a>(nodes: &'a [Value], name: &str) -> &'a Value {
    nodes
        .iter()
        .find(|n| n["module"] == name)
        .unwrap_or_else(|| panic!("no module {name}"))
}

pub fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}
