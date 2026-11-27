// build.rs — a graph turned into unpositioned cards and cables, mirroring the signal path: the shared state, the sockets, the ranks.
// A reader would get wrong that ids are the contract: `trk:`, `mod:`, `chn:`, `snk:` and a cable's `a->b` key are what the layout and the page address.

use crate::geom::{MAIN_OUT, MIX_OUT, RENDER_OUT, VIDEO_OUT, jacks_for, node_size};
use crate::graph::{Graph, clip_span_of};
use crate::grid_fit::{GridFit, fit_panel};
use crate::node::{Edge, Jacks, Kind, Node, Signal};
use crate::node_ui::{InputDevice, Machine, main_out_ui, render_ui};
use crate::panel::Panel;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// What a host tells the build beyond the document: the devices, the machine, the shape to draw.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct BuildOptions {
    /// Draw a card for the devices the signal leaves by; absent means yes.
    pub draw_main_out: Option<bool>,
    /// The song's output level in dB, a project setting the document does not hold.
    pub main_out_volume: Option<f64>,
    /// Which row of the render card's dropdown is ticked, the frame's shape.
    pub aspect: Option<f64>,
    pub input_devices: Option<Vec<InputDevice>>,
    pub cameras: Option<Vec<String>>,
    pub midi_devices: Option<Vec<String>>,
    pub fit: Option<GridFit>,
    /// Which of a panel's two shapes to draw: `horizontal` (the default) or its narrow twin.
    pub layout: Option<String>,
}

impl BuildOptions {
    pub fn machine(&self) -> Machine<'_> {
        Machine {
            inputs: self.input_devices.as_deref().unwrap_or_default(),
            cameras: self.cameras.as_deref().unwrap_or_default(),
            midi: self.midi_devices.as_deref().unwrap_or_default(),
        }
    }

    pub fn vertical(&self) -> bool {
        self.layout.as_deref() == Some("vertical")
    }
}

/// One run's shared state and writers.
pub struct Build<'a> {
    pub graph: &'a Graph,
    pub opts: &'a BuildOptions,
    pub nodes: IndexMap<String, Node>,
    /// Keyed by `source->target`, so a connection is drawn once.
    pub edges: IndexMap<String, Edge>,
    /// Which curves a binding put on the graph, so the ones nothing binds can be drawn beside them.
    pub bound_curves: HashSet<String>,
    /// The maximum step index a module holds across chains, so a shared one ranks after every pipeline.
    pub module_rank: IndexMap<String, f64>,
    /// A fader's lane, inverted once: the document keys by lane, and a node asks the other way.
    pub mixer_owner: HashMap<String, String>,
    pub clip_span: f64,
    pub used_modules: HashSet<String>,
    pub feeds_sum: HashMap<&'static str, bool>,
    pairs: HashMap<String, Vec<String>>,
}

pub fn track_id(n: &str) -> String {
    format!("trk:{n}")
}
pub fn sink_id(n: &str) -> String {
    format!("snk:{n}")
}
pub fn module_id(n: &str) -> String {
    format!("mod:{n}")
}
pub fn chain_id(n: &str) -> String {
    format!("chn:{n}")
}

pub fn is_sum(n: &str) -> bool {
    n == MIX_OUT || n == RENDER_OUT
}

pub fn is_device(n: &str) -> bool {
    n == MAIN_OUT || n == VIDEO_OUT
}

impl<'a> Build<'a> {
    pub fn new(graph: &'a Graph, opts: &'a BuildOptions) -> Build<'a> {
        let mut used = HashSet::new();
        for c in graph.chains.values() {
            for s in &c.steps {
                used.insert(s.module.clone());
            }
        }
        let mixer_owner = graph
            .track_mixers
            .iter()
            .flatten()
            .map(|(track, node)| (node.clone(), track.clone()))
            .collect();
        Build {
            graph,
            opts,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            bound_curves: HashSet::new(),
            module_rank: IndexMap::new(),
            mixer_owner,
            clip_span: clip_span_of(graph),
            used_modules: used,
            feeds_sum: HashMap::from([(MIX_OUT, false), (RENDER_OUT, false)]),
            pairs: HashMap::new(),
        }
    }

    pub fn is_chain(&self, n: &str) -> bool {
        self.graph.chains.contains_key(n)
    }

    pub fn is_video_track(&self, n: &str) -> bool {
        self.graph
            .video_tracks
            .as_ref()
            .is_some_and(|v| v.iter().any(|t| t == n))
    }

    /// A lane, of either kind: never followed as a chain, and never drawn as a sink.
    pub fn is_lane(&self, n: &str) -> bool {
        self.graph.tracks.iter().any(|t| t == n) || self.is_video_track(n)
    }

    /// A chain's end: a module's card, or the sum's own card when a master run holds none.
    fn end_id(&self, chain: &str, module: Option<&str>) -> String {
        match module {
            Some(m) => module_id(m),
            None if chain == MIX_OUT => sink_id(MIX_OUT),
            None if chain == RENDER_OUT => sink_id(RENDER_OUT),
            None => chain_id(chain),
        }
    }

    pub fn entry_id(&self, chain: &str) -> String {
        let steps = &self.graph.chains[chain].steps;
        self.end_id(chain, steps.first().map(|s| s.module.as_str()))
    }

    pub fn exit_id(&self, chain: &str) -> String {
        let steps = &self.graph.chains[chain].steps;
        self.end_id(chain, steps.last().map(|s| s.module.as_str()))
    }

    /// Adds a card once: a second write only merges the chains a module belongs to.
    pub fn add(&mut self, node: Node) -> String {
        let id = node.id.clone();
        if let Some(existing) = self.nodes.get_mut(&id) {
            if node.kind == Kind::Module
                && let Some(chains) = &node.chains
            {
                let mut merged: Vec<String> = existing.chains.clone().unwrap_or_default();
                for c in chains {
                    if !merged.contains(c) {
                        merged.push(c.clone());
                    }
                }
                // The sub line names the chains sharing this module, so it must follow the merge.
                existing.sub = (merged.len() > 1).then(|| merged.join(", "));
                existing.chains = Some(merged);
            }
        } else {
            self.nodes.insert(id.clone(), node);
        }
        id
    }

    /// A destination with no module behind it: a built-in output, or a name the graph cannot account for.
    pub fn add_sink(&mut self, name: &str, main_out: bool) -> String {
        let mut n = Node::plain(&sink_id(name), name, Kind::Sink);
        if main_out {
            self.main_out_card(&mut n);
        }
        self.add(n)
    }

    /// What the render sum's card carries: the frame's shape as a field, when the host said what it is.
    pub fn render_card(opts: &BuildOptions, n: &mut Node) {
        let Some(aspect) = opts.aspect.filter(|a| *a >= 0.0) else {
            return;
        };
        let ui = render_ui();
        (n.w, n.h) = node_size(Some(&ui), false);
        n.module = Some(RENDER_OUT.to_owned());
        n.ui = Some(ui);
        n.aspect = Some(aspect);
    }

    /// …and what the main output's card carries: its own fader, absent rather than 0.
    pub fn main_out_card(&self, n: &mut Node) {
        let ui = main_out_ui();
        (n.w, n.h) = node_size(Some(&ui), false);
        n.module = Some(MAIN_OUT.to_owned());
        n.ui = Some(ui);
        if let Some(v) = self.opts.main_out_volume {
            n.params = Some(Map::from_iter([("Volume".to_owned(), Value::from(v))]));
        }
    }

    /// Adds a connection once, once per socket and not per pair.
    pub fn link(
        &mut self,
        source: &str,
        target: &str,
        label: Option<String>,
        signal: Option<Signal>,
        target_port: Option<&str>,
        source_port: Option<&str>,
    ) {
        if source == target {
            return;
        }
        let pk = if source < target {
            format!("{source}|{target}")
        } else {
            format!("{target}|{source}")
        };
        let on_pair = self.pairs.get(&pk).cloned().unwrap_or_default();
        // A cable already here that this expression could be: same source port, or either naming none.
        let existing = on_pair.iter().find(|id| {
            self.edges.get(*id).is_some_and(|e| {
                source_port.is_none_or(str::is_empty)
                    || e.source_port.as_deref().is_none_or(str::is_empty)
                    || e.source_port.as_deref() == source_port
            })
        });
        if let Some(id) = existing.cloned() {
            let Some(e) = self.edges.get_mut(&id) else { return };
            if e.label.as_deref().is_none_or(str::is_empty) && label.as_deref().is_some_and(|l| !l.is_empty()) {
                e.label = label;
            }
            if e.signal.is_none() && signal.is_some() {
                e.signal = signal;
            }
            if e.target_port.as_deref().is_none_or(str::is_empty) && target_port.is_some_and(|p| !p.is_empty()) {
                e.target_port = target_port.map(str::to_owned);
            }
            if e.source_port.as_deref().is_none_or(str::is_empty) && source_port.is_some_and(|p| !p.is_empty()) {
                e.source_port = source_port.map(str::to_owned);
            }
            return;
        }
        // The port is in the id: two cables between one pair need two, and the renderer keys on it.
        let key = match source_port {
            Some(p) if !p.is_empty() => format!("{source}#{p}->{target}"),
            _ => format!("{source}->{target}"),
        };
        self.pairs.entry(pk).or_default().push(key.clone());
        let mut e = Edge::new(&key, source, target);
        e.label = label;
        e.signal = signal;
        e.target_port = target_port.map(str::to_owned);
        e.source_port = source_port.map(str::to_owned);
        self.edges.insert(key, e);
    }

    /// What leaves a chain: its last module, or whatever reached a pass-through.
    pub fn exit_signal(&self, chain: &str) -> Option<Signal> {
        self.exit_signal_seen(chain, &mut HashSet::new())
    }

    fn exit_signal_seen(&self, chain: &str, seen: &mut HashSet<String>) -> Option<Signal> {
        let c = self.graph.chains.get(chain)?;
        if !seen.insert(chain.to_owned()) {
            return None;
        }
        if let Some(last) = c.steps.last() {
            return last.emits;
        }
        let from = c.from.as_deref().filter(|f| !f.is_empty())?;
        // A run out of a sum is answered by name: following `from` re-enters this chain.
        if is_sum(from) {
            return Some(if from == RENDER_OUT {
                Signal::Video
            } else {
                Signal::Audio
            });
        }
        match c.from_port.as_deref() {
            Some("midi") => return Some(Signal::Midi),
            Some("audio") => return Some(Signal::Audio),
            _ => {}
        }
        if self.is_chain(from) && !self.is_lane(from) {
            return self.exit_signal_seen(from, seen);
        }
        if self.is_video_track(from) {
            return Some(Signal::Video);
        }
        self.graph.track_signals.as_ref().and_then(|t| t.get(from)).copied()
    }
}

#[derive(Default)]
pub struct Sockets {
    pub fed: HashMap<String, HashSet<String>>,
    pub left: HashMap<String, HashSet<String>>,
    pub bare: HashMap<String, usize>,
}

/// Which sockets have a cable, per node: a last pass, since it needs every cable.
pub fn count_sockets(edges: &mut IndexMap<String, Edge>) -> Sockets {
    let mut s = Sockets::default();
    for e in edges.values_mut() {
        if e.automation.is_some() {
            continue;
        }
        s.fed
            .entry(e.target.clone())
            .or_default()
            .insert(e.target_port.clone().unwrap_or_default());
        if e.target_port.as_deref().is_none_or(str::is_empty) {
            // Ranked in the graph's own declaration order, so a cable keeps its socket from one draw to the next.
            let n = s.bare.entry(e.target.clone()).or_insert(0);
            e.target_slot = Some(*n as f64);
            *n += 1;
        }
        s.left
            .entry(e.source.clone())
            .or_default()
            .insert(e.source_port.clone().unwrap_or_default());
    }
    s
}

/// What leaves a node colours its output jack; an unfed input borrows the cable that reaches it.
fn assign_jacks(b: &mut Build, s: &Sockets) {
    let mut arriving: HashMap<&str, Signal> = HashMap::new();
    let mut leaving: HashMap<&str, Option<Signal>> = HashMap::new();
    for e in b.edges.values().filter(|e| e.automation.is_none()) {
        if let Some(sig) = e.signal {
            arriving.entry(&e.target).or_insert(sig);
        }
        leaving.entry(&e.source).or_insert(e.signal);
    }
    let none = HashSet::new();
    let mix_id = sink_id(MIX_OUT);
    let mut jacks: Vec<(String, Jacks)> = Vec::new();
    for n in b.nodes.values().filter(|n| n.kind != Kind::Curve) {
        let source = n.monitor.is_some() || n.id == mix_id;
        let track = n.kind == Kind::Track;
        let inputs = if source {
            Vec::new()
        } else {
            let fallback = arriving.get(n.id.as_str()).copied().or(if track {
                leaving.get(n.id.as_str()).copied().flatten()
            } else {
                None
            });
            jacks_for(
                n.inputs.as_deref(),
                s.fed.get(&n.id).unwrap_or(&none),
                fallback,
                true,
                false,
                None,
                s.bare.get(&n.id).copied().unwrap_or(0),
            )
        };
        let outputs = if n.kind == Kind::Sink && n.id != mix_id {
            Vec::new()
        } else {
            let bare_on = track.then(|| match b.graph.track_signals.as_ref().and_then(|t| t.get(&n.label)) {
                Some(Signal::Midi) => "midi",
                Some(Signal::Video) => "video",
                _ => "audio",
            });
            jacks_for(
                n.outputs.as_deref(),
                s.left.get(&n.id).unwrap_or(&none),
                leaving.get(n.id.as_str()).copied().flatten(),
                false,
                track,
                bare_on,
                0,
            )
        };
        jacks.push((n.id.clone(), Jacks { inputs, outputs }));
    }
    for (id, j) in jacks {
        if let Some(n) = b.nodes.get_mut(&id) {
            n.jacks = Some(j);
        }
    }
}

/// Rank hints, top to bottom: inputs, lanes, lane modules, the faders, the sum, the master run, sinks.
fn assign_ranks(b: &mut Build) {
    let graph = b.graph;
    // The modules on a master run, ranked after every lane rather than by their small position inside that run.
    let mut on_master: HashSet<String> = HashSet::new();
    for c in graph.chains.values() {
        if is_sum(c.from.as_deref().unwrap_or_default()) {
            for s in &c.steps {
                on_master.insert(module_id(&s.module));
            }
        }
    }
    let (input_rank, track_rank) = (0.0, 1.0);
    let mut lane_rank: IndexMap<String, f64> = IndexMap::new();
    let mut master_max = -1.0_f64;
    for (id, r) in &b.module_rank {
        if on_master.contains(id) {
            master_max = master_max.max(*r);
        } else {
            lane_rank.insert(id.clone(), track_rank + 1.0 + r);
        }
    }
    // The lane faders share one row, the deepest of theirs, and what follows one in its chain moves down with it.
    let faders: Vec<&String> = lane_rank
        .keys()
        .filter(|id| {
            b.nodes
                .get(*id)
                .is_some_and(|n| n.track_face.as_deref().is_some_and(|f| !f.is_empty()))
        })
        .collect();
    if faders.len() > 1 {
        let row = faders.iter().map(|id| lane_rank[*id]).fold(f64::MIN, f64::max);
        for c in graph.chains.values() {
            if is_sum(c.from.as_deref().unwrap_or_default()) {
                continue;
            }
            let at = c.steps.iter().position(|s| b.mixer_owner.contains_key(&s.module));
            if let Some(at) = at {
                for (i, s) in c.steps.iter().enumerate().skip(at) {
                    let id = module_id(&s.module);
                    let cur = lane_rank.get(&id).copied().unwrap_or(0.0);
                    lane_rank.insert(id, cur.max(row + (i - at) as f64));
                }
            }
        }
    }
    let lane_max = lane_rank.values().copied().fold(track_rank, f64::max);
    // The sum comes after every lane: ranked where a source is, it sat on the lane row and one cable spanned the page.
    let sum_rank = lane_max + 1.0;
    let sink_rank = sum_rank + 2.0 + master_max;
    let mix_id = sink_id(MIX_OUT);
    let render_id = sink_id(RENDER_OUT);
    let mut first_auto: HashMap<String, String> = HashMap::new();
    for e in b.edges.values().filter(|e| e.automation.is_some()) {
        first_auto.entry(e.source.clone()).or_insert_with(|| e.target.clone());
    }
    let mut ranks: HashMap<String, f64> = HashMap::new();
    for n in b.nodes.values() {
        let rank = match n.kind {
            Kind::Track => Some(track_rank),
            // A live input feeds a lane, so its row is above the lanes.
            Kind::Module if n.monitor.is_some() => Some(input_rank),
            _ if n.id == mix_id || n.id == render_id => Some(sum_rank),
            Kind::Module => Some(if on_master.contains(&n.id) {
                sum_rank + 1.0 + b.module_rank.get(&n.id).copied().unwrap_or(0.0)
            } else {
                lane_rank.get(&n.id).copied().unwrap_or(track_rank + 1.0)
            }),
            Kind::Sink => Some(sink_rank),
            Kind::Chain => Some(track_rank + 1.0),
            // A curve sits at the same rank as the module it drives, once that one has its own.
            Kind::Curve => first_auto.get(&n.id).and_then(|t| ranks.get(t)).copied(),
            Kind::Assembly => None,
        };
        if let Some(r) = rank {
            ranks.insert(n.id.clone(), r);
        }
    }
    for (id, r) in ranks {
        if let Some(n) = b.nodes.get_mut(&id) {
            n.rank = Some(r);
        }
    }
}

/// A card in its narrow shape: the panel's `vertical` twin when it has one, its own grid otherwise, tabs as a menu either way.
fn narrow_node(n: &mut Node) {
    let Some(ui) = n.ui.take() else { return };
    if n.strip == Some(true) {
        n.ui = Some(ui);
        return;
    }
    let narrow = match ui.vertical.clone() {
        Some(v) => Panel {
            script: ui.script.clone(),
            vertical: None,
            tabs_as: Some("dropdown".to_owned()),
            ..*v
        },
        None => Panel {
            vertical: None,
            tabs_as: Some("dropdown".to_owned()),
            ..ui.clone()
        },
    };
    resize(n, &ui, narrow);
}

/// Swaps a card's panel and grows or shrinks the card by what the panel changed.
fn resize(n: &mut Node, before: &Panel, after: Panel) {
    let (bw, bh) = node_size(Some(before), false);
    let (aw, ah) = node_size(Some(&after), false);
    n.ui = Some(after);
    n.w += aw - bw;
    n.h += ah - bh;
}

/// Each tab's card height, from the rows its widgets and the shared ones really reach.
fn tab_heights(n: &mut Node) {
    let Some(ui) = &n.ui else { return };
    let Some(tabs) = ui.tabs.as_ref().filter(|t| !t.is_empty()) else {
        return;
    };
    if n.strip == Some(true) {
        return;
    }
    let reach = |ws: &crate::panel::Widgets| ws.values().map(|w| w.row + w.rows()).fold(0.0, f64::max);
    let shared = reach(&ui.widgets);
    let full = node_size(Some(ui), false).1;
    let hs: Vec<f64> = tabs
        .iter()
        .map(|t| {
            let rows = 1.0_f64.max(shared).max(reach(&t.widgets));
            n.h + node_size(Some(&Panel { rows, ..ui.clone() }), false).1 - full
        })
        .collect();
    if hs.iter().any(|h| *h != n.h) {
        n.tab_h = Some(hs);
    }
}

/// A strip is left whole: its fader is its height, and a fader folded beside itself is no strip.
fn fit_node(n: &mut Node, fit: GridFit) {
    let Some(ui) = n.ui.clone() else { return };
    if n.strip == Some(true) {
        return;
    }
    let Some(fitted) = fit_panel(&ui, fit) else { return };
    let after_w = node_size(Some(&fitted), false).0;
    let tabs = fitted.tabs.clone();
    resize(n, &ui, fitted.clone());
    // …and its width per tab, each by the same difference: the card is as wide as the page it shows.
    if let Some(tabs) = tabs.filter(|ts| ts.iter().any(|t| t.cols.is_some())) {
        let w = n.w;
        n.tab_w = Some(
            tabs.iter()
                .map(|t| {
                    let p = Panel {
                        cols: t.cols.unwrap_or(fitted.cols),
                        ..fitted.clone()
                    };
                    w + node_size(Some(&p), false).0 - after_w
                })
                .collect(),
        );
    }
}

/// Builds nodes and unpositioned cables from the graph, mirroring the signal path.
pub fn build_graph(graph: &Graph, opts: &BuildOptions) -> (Vec<Node>, Vec<Edge>) {
    let mut b = Build::new(graph, opts);
    for (name, c) in &graph.chains {
        b.add_chain_modules(name, c);
        b.add_chain_source(name, c);
        b.add_chain_outputs(name, c);
    }
    b.add_tap_feeds();
    b.add_sums();
    b.add_orphan_lanes();
    // The socket pass needs every cable; the free curves add none, so they may join first.
    let sockets = count_sockets(&mut b.edges);
    b.add_free_curves();
    assign_jacks(&mut b, &sockets);
    assign_ranks(&mut b);
    // The narrow twin first, then the fold for a panel that has none and is still too wide.
    if opts.vertical() {
        b.nodes.values_mut().for_each(narrow_node);
    }
    if let Some(fit) = opts.fit {
        b.nodes.values_mut().for_each(|n| fit_node(n, fit));
    }
    // A narrow card is as tall as its open page, not its longest.
    if opts.vertical() {
        b.nodes.values_mut().for_each(tab_heights);
    }
    (b.nodes.into_values().collect(), b.edges.into_values().collect())
}

#[cfg(test)]
mod tests;
