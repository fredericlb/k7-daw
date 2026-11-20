// graph/build.rs — the private assembly of a GraphDoc from the Inputs a compile loaded.
// A reader would get wrong that the chains are derived AGAIN here, with the graph's own lane test, which is wider than the compile's (402).

use super::index::Index;
use super::meter::Meter;
use super::validate::validate_inner;
use super::{
    ClipKind, GraphDoc, GraphError, GraphModule, GraphPlacement, MAX_ISSUES, Refusal, Track, TrackType, VideoLane,
    graph_module,
};
use crate::cache::Cache;
use crate::clips::js_number;
use crate::files::{Finding, Severity};
use crate::lanes::inputs::Inputs;
use crate::lanes::references::Fault;
use crate::lanes::rig::Rig;
use crate::lanes::wiring::{WiringInput, check_chain_inputs};
use crate::lanes::{Lane, LaneKind, lane_of};
use crate::modules::{ModuleKind, Port};
use crate::resolve::Resolved;
use crate::roots::Roots;
use crate::routing::{Chain, apply_link_levels, derive_chains, split_port};
use crate::source::Source;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// The file a finding about a lane module is named by.
fn lane_label(key: &str, m: &Resolved) -> String {
    if matches!(m.home.as_deref(), Some("nodes" | "chains" | "tracks")) {
        m.own_label()
    } else {
        format!("{key}/module.k7x")
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

/// The findings and faults that stop a graph (405): the stops of the inputs but `timelines/`, and any wiring fault.
fn refusal(inputs: &Inputs) -> Option<Refusal> {
    let findings: Vec<Finding> = inputs
        .stops()
        .into_iter()
        .filter(|f| !f.file.starts_with("timelines/"))
        // `loadGraph` never reads settings.k7x (405).
        .filter(|f| f.file != "settings.k7x")
        // A signature that makes no bar is the compile's finding, and the graph still draws on 4/4 (418).
        .filter(|f| !inputs.head.compile_findings.contains(f))
        .cloned()
        .collect();
    let lanes = inputs.lanes();
    let clips: HashMap<&str, &_> = inputs.clips.values.iter().map(|(id, c)| (id.as_str(), &**c)).collect();
    let sequences: HashMap<&str, &_> = inputs
        .sequences
        .values
        .iter()
        .map(|(id, c)| (id.as_str(), &**c))
        .collect();
    let tap_fed: HashSet<String> = inputs
        .routing
        .segments
        .tap_feeds_of()
        .iter()
        .map(|f| f.to.clone())
        .collect();
    // The RAW ports: a node that only inherits its ports is unconstrained here (404).
    let inputs_of = |name: &str| -> Option<&[Port]> {
        inputs
            .nodes
            .nodes
            .get(name)
            .or_else(|| inputs.vocabulary.modules.get(name))
            .map(|m| m.inputs())
    };
    let picture = |n: &str| lanes.picture.contains_key(n);
    let wiring = WiringInput {
        chains: &inputs.chains,
        inputs_of: &inputs_of,
        sound: &lanes.sound,
        clips: &clips,
        sequences: &sequences,
        picture: &picture,
        tap_fed: &tap_fed,
    };
    let faults: Vec<Fault> = check_chain_inputs(&wiring);
    (!findings.is_empty() || !faults.is_empty()).then_some(Refusal { findings, faults })
}

/// Every module of the project then the vocabulary modules its chains name, once each, as the graph reads them.
fn modules_of(inputs: &Inputs) -> IndexMap<&str, GraphModule<'_>> {
    let mut out = IndexMap::new();
    for (k, m) in inputs.local.modules.iter().chain(inputs.extra.modules.iter()) {
        out.entry(k.as_str()).or_insert_with(|| graph_module(m));
    }
    out
}

fn track_type(k: LaneKind) -> TrackType {
    match k {
        LaneKind::Group => TrackType::Group,
        LaneKind::Strudel => TrackType::Strudel,
        _ => TrackType::Single,
    }
}

/// What a placement names, through maps built once: a clip, then a sequence, then a timeline (417).
struct Kinds<'i> {
    clips: HashSet<&'i str>,
    sequences: HashSet<&'i str>,
    timelines: HashSet<&'i str>,
}

impl Kinds<'_> {
    fn of(&self, r: &str) -> ClipKind {
        if self.clips.contains(r) {
            ClipKind::Audio
        } else if self.sequences.contains(r) {
            ClipKind::Midi
        } else if self.timelines.contains(r) {
            ClipKind::Timeline
        } else {
            ClipKind::Missing
        }
    }
}

fn track_of<'i>(
    name: &'i str,
    lane: &Lane<'i>,
    m: &'i Resolved,
    mute: bool,
    meter: &Meter,
    kinds: &Kinds<'i>,
    findings: &mut Vec<Finding>,
) -> Track<'i> {
    let mut placements = Vec::with_capacity(lane.events.len());
    let mut dropped = 0usize;
    for p in lane.events {
        let r = p.ref_.as_ref().map_or("", |r| r.0.as_str());
        let start = meter.position(p.start.as_ref());
        if start.is_finite() {
            placements.push(GraphPlacement {
                ref_: r,
                start,
                kind: kinds.of(r),
            });
        } else {
            dropped += 1;
        }
    }
    if dropped > 0 {
        findings.push(warning(
            lane_label(name, m),
            format!(
                "lane '{}': {dropped} placement(s) with no usable start were left out",
                super::issues::cut(name)
            ),
        ));
    }
    Track {
        name,
        kind: track_type(lane.kind),
        color: lane.color,
        mute,
        placements,
        tracks: lane.tracks,
        strudel: lane.strudel,
        video_source: lane.video_source,
    }
}

pub(super) fn load<'i>(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    inputs: &'i Inputs,
) -> Result<GraphDoc<'i>, GraphError> {
    if let Some(r) = refusal(inputs) {
        return Err(GraphError::Refused(r));
    }
    let modules = modules_of(inputs);
    let kinds = Kinds {
        clips: inputs.clips.values.iter().map(|(id, _)| id.as_str()).collect(),
        sequences: inputs.sequences.values.iter().map(|(id, _)| id.as_str()).collect(),
        timelines: inputs.timelines.values.iter().map(|(id, _)| id.as_str()).collect(),
    };
    let meter = Meter::of(inputs);
    let mut findings: Vec<Finding> = Vec::new();
    for f in [
        &inputs.clips.findings,
        &inputs.sequences.findings,
        &inputs.timelines.findings,
    ] {
        findings.extend(f.iter().cloned());
    }
    // The sound lanes then the picture lanes (403): local modules first, then the vocabulary's.
    let resolved = inputs.local.modules.iter().chain(inputs.extra.modules.iter());
    let mut tracks = IndexMap::new();
    let mut disabled: Vec<&str> = Vec::new();
    let mut video_tracks = IndexMap::new();
    for (key, m) in resolved {
        let Some(lane) = lane_of(m) else { continue };
        let name = key.as_str();
        if lane.kind == LaneKind::Picture {
            if inputs.local.modules.contains_key(name) && !video_tracks.contains_key(name) {
                video_tracks.insert(
                    name,
                    VideoLane {
                        color: lane.color,
                        mute: inputs.bypassed.contains(name) || lane.disabled,
                    },
                );
            }
        } else if lane.disabled {
            disabled.push(name);
        } else if !tracks.contains_key(name) {
            let mute = inputs.bypassed.contains(name);
            tracks.insert(name, track_of(name, &lane, m, mute, &meter, &kinds, &mut findings));
        }
    }
    disabled.sort_unstable();
    disabled.dedup();
    // Chains: derived again with the graph's lane test, minus those a disabled lane feeds, then the levels (402).
    let is_lane = |n: &str| modules.get(n).is_some_and(|m| m.kind == ModuleKind::Track) || video_tracks.contains_key(n);
    let disabled_set: HashSet<&str> = disabled.iter().copied().collect();
    let mut chains: IndexMap<String, Chain> = derive_chains(&inputs.routing.segments, &is_lane);
    chains.retain(|_, c| !c.from.as_deref().is_some_and(|f| disabled_set.contains(f)));
    apply_link_levels(&mut chains, &inputs.routing.levels);
    let assembled = crate::assemblies::resolve_assemblies(source, cache, roots)?;
    findings.extend(assembled.findings);
    let positions = inputs.routing.segments.positions.as_ref().filter(|p| !p.is_empty());
    let mut g = GraphDoc {
        chains,
        tracks,
        video_tracks,
        disabled,
        modules,
        positions,
        assemblies: assembled.assemblies,
        tap_feeds: inputs.routing.segments.tap_feeds_of(),
        issues: Vec::new(),
        warnings: Vec::new(),
        findings,
    };
    let (issues, more) = validate_inner(&g, &inputs.vocabulary);
    g.warnings = issues.iter().map(|i| i.text.clone()).collect();
    g.issues = issues;
    sentences(&mut g, &inputs.rig, more);
    Ok(g)
}

/// Whether `to` is a bus the compile sends to: a chain of its own, entered by its main input, fed by no lane.
fn is_bus_target(g: &GraphDoc<'_>, idx: &Index<'_>, to: &str) -> bool {
    if split_port(to).1.is_some() || idx.is_sink(to) {
        return false;
    }
    let is_lane = |n: &str| g.tracks.contains_key(n) || idx.is_disabled(n);
    g.chains
        .get(to)
        .is_some_and(|c| !c.from.as_deref().is_some_and(is_lane))
}

/// The sentences outside `validate_issues`, under the same cap: the levels not played, then the input labels (411).
fn sentences(g: &mut GraphDoc<'_>, rig: &Rig, mut more: bool) {
    let idx = Index::new(g);
    let mut room = MAX_ISSUES.saturating_sub(g.warnings.len());
    let mut out: Vec<String> = Vec::new();
    // Written here and not by `unplayed_levels`, which copies every name before the cap can stop it.
    'chains: for (name, c) in &g.chains {
        for o in &c.out {
            if o.gain == 1.0 || is_bus_target(g, &idx, &o.to) {
                continue;
            }
            if room == 0 {
                more = true;
                break 'chains;
            }
            room -= 1;
            out.push(format!(
                "chain '{}': the level {} on its link to '{}' is not played — only a link to a bus carries one",
                super::issues::cut(name),
                js_number(o.gain),
                super::issues::cut(&o.to)
            ));
        }
    }
    let labels = super::validate_lanes::input_labels(g, rig, room, &mut more);
    out.extend(labels);
    g.warnings.extend(out);
    if more {
        g.warnings
            .push(format!("more than {MAX_ISSUES} warnings — the rest are left out"));
    }
}
