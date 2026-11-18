// graph/view/mod.rs — the WebGraph a graph is drawn as: every node's flags, panel and choices, every lane's clips, every cable's level.
// A reader would get wrong that the budget counts the OUTPUT: past it the topology stays whole and the steps go bare (419, 420).

mod amp;
mod choices;
mod flags;
mod handed;
mod lanes;
mod parts;
mod sounds;
mod step;
pub mod web;

pub use amp::{AmpKind, AmpModel, amp_of_json};
pub use flags::{PluginFlags, flags_of, param_defaults, stage_of};
pub use handed::{Folder, ModelNeed, handed_of, load_compiled_graph, model_need, render_exists};
pub use web::{WebChain, WebChainStep, WebGraph, WebOut};

use super::{GraphDoc, GraphError, Index, Spend, lane_strips_of, load_graph, node_places, node_stacks, picture_lanes};
use crate::cache::Cache;
use crate::curves::read_curves;
use crate::files::{Finding, Severity};
use crate::graph::meter::Meter;
use crate::lanes::inputs::{Inputs, load_inputs};
use crate::lanes::rig::Rig;
use crate::roots::Roots;
use crate::source::{Source, SourceError};
use choices::Walks;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use step::{Facts, body, plugin_id, serialized_len};
use web::{WebChain as Chain, WebChainStep as Step, WebOut as Out, bare_step};

/// A node name to the name it is marked with.
pub type Marks = IndexMap<String, String>;

/// A clip's compiled place: where it starts and how long it runs, in beats.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipExtent {
    pub start: f64,
    pub length: f64,
}

/// One freeze plan in force, as `activeFreezePlans` answers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreezeMark {
    pub at: String,
    pub lanes: Vec<String>,
    pub modules: Vec<String>,
}

/// One automatic freeze: the VST module it is for, whether its render stands in, whether the engine hosts the VST.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoMark {
    pub module: String,
    pub ready: bool,
    pub hosted: bool,
    pub lanes: Vec<String>,
    pub modules: Vec<String>,
}

/// What the compile hands the graph, as plain data (430): none of it when the project does not compile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Handed {
    pub extents: Option<IndexMap<String, Vec<ClipExtent>>>,
    pub mixers: Option<IndexMap<String, String>>,
    pub colors: Option<IndexMap<String, String>>,
    pub manual: Vec<FreezeMark>,
    pub auto: Vec<AutoMark>,
}

/// The graph as the Chains tab draws it, and what its building found.
pub struct Answer {
    pub graph: WebGraph,
    pub findings: Vec<Finding>,
    pub capped: bool,
    pub spent: usize,
}

/// The file a module's step is named by when the budget stops there.
fn file_of(g: &GraphDoc<'_>, module: &str) -> String {
    match g
        .modules
        .get(module)
        .and_then(|m| m.m.home.as_deref().map(|h| (h, m.m)))
    {
        Some(("chains", m)) => m.own_label(),
        _ => "graph.k7x".to_owned(),
    }
}

fn warning(file: String, message: String) -> Finding {
    Finding {
        file,
        line: 1,
        col: 1,
        message,
        severity: Severity::Warning,
    }
}

/// The chains of the graph, their cables and their steps; the steps are paid for after the topology.
fn chains_of(
    g: &GraphDoc<'_>,
    inputs: &Inputs,
    facts: &Facts<'_>,
    walks: &mut Walks<'_>,
    spend: &mut Spend,
    stopped: &mut Option<String>,
) -> Result<IndexMap<String, Chain>, SourceError> {
    let places = inputs.routing.project.as_ref().map(node_places).unwrap_or_default();
    let mut chains: IndexMap<String, Chain> = IndexMap::new();
    for (name, c) in &g.chains {
        // The topology is always drawn; it is charged so the steps that follow know what is left.
        let mut bytes = name.len() + c.from.as_deref().map_or(0, str::len) + c.from_port.as_deref().map_or(0, str::len);
        let out: Vec<Out> = c
            .out
            .iter()
            .map(|o| {
                bytes += o.to.len() + o.from_port.as_deref().map_or(0, str::len) + 16;
                Out {
                    to: o.to.clone(),
                    gain: o.gain,
                    from_port: o.from_port.clone().filter(|p| !p.is_empty()),
                }
            })
            .collect();
        spend.bytes(bytes);
        chains.insert(
            name.clone(),
            Chain {
                from: c.from.clone().filter(|f| !f.is_empty()),
                from_port: c.from_port.clone().filter(|p| !p.is_empty()),
                use_steps: Vec::new(),
                out,
            },
        );
    }
    let mut bodies: HashMap<&str, (Step, usize)> = HashMap::new();
    for (name, c) in &g.chains {
        let mut steps = Vec::with_capacity(c.use_steps.len());
        for s in &c.use_steps {
            let enabled = s.enabled;
            let Some(m) = g.modules.get(s.module.as_str()) else {
                let mut miss = bare_step(&s.module, enabled);
                miss.missing = Some(true);
                spend.bytes(s.module.len());
                steps.push(miss);
                continue;
            };
            if spend.capped() {
                stopped.get_or_insert_with(|| file_of(g, &s.module));
                steps.push(bare_step(&s.module, enabled));
                continue;
            }
            if !bodies.contains_key(s.module.as_str()) {
                let file = file_of(g, &s.module);
                match body(facts, &s.module, m, places.get(s.module.as_str()), walks, spend, &file)? {
                    Some(b) => {
                        let w = serialized_len(&b);
                        bodies.insert(s.module.as_str(), (b, w));
                    }
                    None => {
                        stopped.get_or_insert(file);
                        steps.push(bare_step(&s.module, enabled));
                        continue;
                    }
                }
            }
            let Some((b, w)) = bodies.get(s.module.as_str()) else {
                continue;
            };
            let pid = plugin_id(name, &s.module, m, facts.scan);
            let units = (*w + pid.as_ref().map_or(0, String::len)) / 64 + 1;
            if !spend.charge(units) {
                stopped.get_or_insert_with(|| file_of(g, &s.module));
                steps.push(bare_step(&s.module, enabled));
                continue;
            }
            let mut step = b.clone();
            step.plugin_id = pid;
            step.enabled = enabled;
            steps.push(step);
        }
        if let Some(w) = chains.get_mut(name) {
            w.use_steps = steps;
        }
    }
    Ok(chains)
}

/// A string map as `Some` when it holds anything.
fn some<K, V>(m: IndexMap<K, V>) -> Option<IndexMap<K, V>> {
    (!m.is_empty()).then_some(m)
}

/// The graph a `GraphDoc` is drawn as (419, 420).
pub fn web_graph(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    inputs: &Inputs,
    g: &GraphDoc<'_>,
    handed: &Handed,
) -> Result<Answer, SourceError> {
    let read = read_curves(source, cache, &inputs.project)?;
    let mut findings: Vec<Finding> = g.findings.clone();
    findings.extend(read.findings);
    let scan = &inputs.vocabulary.scan;
    let facts = Facts {
        scan,
        curves: &read.curves,
    };
    let idx = Index::new(g);
    let mut spend = Spend::new();
    let mut stopped: Option<String> = None;
    let mut walks = Walks::new(source, roots, inputs);
    let mut chains = chains_of(g, inputs, &facts, &mut walks, &mut spend, &mut stopped)?;
    parts::input_taps(g, &mut chains, &mut spend);
    let meter = Meter::of(inputs);
    let (mut colors, mut signals, mut alternatives, mut clips) =
        (IndexMap::new(), IndexMap::new(), IndexMap::new(), IndexMap::new());
    for (id, t) in &g.tracks {
        let color = t
            .color
            .map(str::to_owned)
            .or_else(|| handed.colors.as_ref().and_then(|c| c.get(*id).cloned()));
        if let Some(c) = color.filter(|c| !c.is_empty()) {
            spend.bytes(id.len() + c.len());
            colors.insert((*id).to_owned(), c);
        }
        spend.bytes(id.len());
        signals.insert((*id).to_owned(), lanes::signal(t));
        if let Some(a) = lanes::track_alternative(g, id, &mut spend) {
            alternatives.insert((*id).to_owned(), a);
        }
        if !spend.capped()
            && let Some(list) = lanes::track_clips(t, handed, &meter, &mut spend)
        {
            clips.insert((*id).to_owned(), list);
        }
    }
    let frozen = lanes::frozen(handed, &mut spend);
    let (baked, waiting) = lanes::auto(handed, &mut spend);
    let picture = picture_lanes(g);
    let mut mixers: IndexMap<String, String> = handed
        .mixers
        .iter()
        .flatten()
        .filter(|(id, _)| g.tracks.contains_key(id.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (lane, strip) in lane_strips_of(g, &idx, &picture) {
        mixers.insert(lane.to_owned(), strip.to_owned());
    }
    let mixers = lanes::paid_mixers(mixers, &mut spend);
    let modules = parts::doc_notes(g, &mut spend);
    let assemblies = if spend.capped() {
        None
    } else {
        parts::assemblies(&g.assemblies, &chains, &mut spend)
    };
    let curves = if spend.capped() {
        None
    } else {
        parts::curves(&read.curves, &mut spend)
    };
    let positions = g.positions.filter(|_| !spend.capped()).map(|p| {
        let mut out = IndexMap::new();
        for (k, v) in p {
            if !spend.bytes(k.len() + 32) {
                break;
            }
            out.insert(k.clone(), *v);
        }
        out
    });
    let tap_feeds = lanes::paid_tap_feeds(g.tap_feeds, &mut spend);
    let stacks = inputs
        .routing
        .project
        .as_ref()
        .map(|p| lanes::paid_stacks(node_stacks(p), &mut spend));
    findings.append(&mut walks.findings);
    let capped = spend.capped();
    if capped {
        let file = stopped.unwrap_or_else(|| "graph.k7x".to_owned());
        findings.push(warning(
            file,
            format!(
                "the graph holds more than {} units of output — its steps and the rest are left bare",
                super::BUDGET
            ),
        ));
    }
    let graph = WebGraph {
        chains,
        tracks: g.tracks.keys().map(|k| (*k).to_owned()).collect(),
        positions: positions.and_then(some),
        video_tracks: g.video_tracks.keys().map(|k| (*k).to_owned()).collect(),
        track_signals: signals,
        track_colors: colors,
        stacks,
        track_mixers: some(mixers),
        track_clips: some(clips),
        track_alternatives: some(alternatives),
        tap_feeds: (!tap_feeds.is_empty()).then_some(tap_feeds),
        frozen: some(frozen),
        auto_frozen: some(baked),
        auto_freeze_waiting: some(waiting),
        modules,
        curves,
        assemblies,
        warnings: g.warnings.clone(),
        issues: (!g.issues.is_empty()).then(|| g.issues.clone()),
    };
    Ok(Answer {
        graph,
        findings,
        capped,
        spent: spend.used(),
    })
}

/// A project read and drawn: what 2l calls. `Refused` when the TypeScript's `loadGraph` would throw (405).
pub fn load_web_graph(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    rig: Rig,
    handed: &Handed,
) -> Result<Answer, GraphError> {
    let inputs = load_inputs(source, cache, roots, rig)?;
    let g = load_graph(source, cache, roots, &inputs)?;
    Ok(web_graph(source, cache, roots, &inputs, &g, handed)?)
}
