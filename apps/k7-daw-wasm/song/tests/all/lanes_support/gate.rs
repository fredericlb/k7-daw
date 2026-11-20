// lanes_support/gate.rs — one project's lanes, owners, live inputs, faults, assemblies and console in the shape stageDumpLanes.ts writes.
// A reader would get wrong that a lane's events are compared as their refs: `Placement` serializes the parsed form, the TypeScript's the authored.

use serde_json::{Map, Value, json};
use song::assemblies::resolve_assemblies;
use song::console::{console_of, console_systems};
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::live::live_inputs_of;
use song::lanes::ownership::chain_ownership;
use song::lanes::references::{check_references, finding_of};
use song::lanes::rig::Rig;
use song::lanes::video::video_tracks;
use song::lanes::{Lane, LaneKind, Lanes, Material, lane_materials};
use song::routing::split_port;
use song::{Cache, Finding, Roots, Severity, Source, SourceError};
use std::collections::BTreeSet;

/// What a run saw, counted by what happened and not by what the files say.
#[derive(Default, Clone, Debug)]
pub struct Counts {
    pub sound: usize,
    pub group: usize,
    pub pattern: usize,
    pub picture: usize,
    pub frames: usize,
    pub events: usize,
    pub chains: usize,
    pub owned_apart: usize,
    pub master: usize,
    pub bus: usize,
    pub input: usize,
    pub cued: usize,
    pub tap: usize,
    pub side_resolved: usize,
    pub side_step: usize,
    pub live: usize,
    pub live_loud: usize,
    pub live_channels: usize,
    pub assemblies: usize,
    pub plugs: usize,
    pub params: usize,
    pub systems: usize,
    pub halves: usize,
    pub faulty: usize,
    pub cue_applied: usize,
    pub kinds: BTreeSet<String>,
    pub sources: BTreeSet<&'static str>,
}

impl Counts {
    pub fn add(&mut self, o: &Counts) {
        macro_rules! sum {
            ($($f:ident),*) => { $(self.$f += o.$f;)* };
        }
        sum!(
            sound,
            group,
            pattern,
            picture,
            frames,
            events,
            chains,
            owned_apart,
            master,
            bus,
            input,
            cued,
            tap,
            side_resolved,
            side_step,
            live,
            live_loud,
            live_channels,
            assemblies,
            plugs,
            params,
            systems,
            halves,
            faulty,
            cue_applied
        );
        self.kinds.extend(o.kinds.iter().cloned());
        self.sources.extend(o.sources.iter().copied());
    }
}

/// A project run: the dump, the files that stop or warn, the fault keys, and what the passes spent.
pub struct Ran {
    pub json: Value,
    pub stopped: Vec<String>,
    pub warned: Vec<String>,
    pub faults: Vec<String>,
    pub weight: usize,
    pub counts: Counts,
}

fn files(findings: &[&Finding], severity: Severity) -> Vec<String> {
    let mut v: Vec<String> = findings
        .iter()
        .filter(|f| f.severity == severity)
        .map(|f| f.file.clone())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// A lane as `laneOf` writes it, its events reduced to their refs.
fn lane_json(lane: &Lane<'_>) -> Value {
    let mut v = serde_json::to_value(lane).expect("json");
    let refs: Vec<String> = lane
        .events
        .iter()
        .map(|e| e.ref_.as_ref().map_or_else(String::new, |r| r.0.clone()))
        .collect();
    v["events"] = json!(refs);
    v
}

/// Which kind of source an id is, for the minimums.
fn source_kind(inputs: &Inputs, lanes: &Lanes<'_>, id: &str) -> &'static str {
    if inputs.sequences.values.iter().any(|(i, _)| i == id) {
        return "sequence";
    }
    if inputs.clips.values.iter().any(|(i, _)| i == id) {
        return "clip";
    }
    let (node, port) = split_port(id);
    if let Some(lane) = lanes.sound.get(node) {
        if lane.kind == LaneKind::Group {
            return "group";
        }
        if matches!(port, Some("midi" | "audio")) {
            return "lane_port";
        }
        let material = |r: &str| {
            if inputs.sequences.values.iter().any(|(i, _)| i == r) {
                Material::Midi
            } else if inputs.clips.values.iter().any(|(i, _)| i == r) {
                Material::Audio
            } else {
                Material::Missing
            }
        };
        let m = lane_materials(lane.events, &material);
        return match (m.missing, m.audio, m.midi) {
            (false, true, false) => "lane_audio",
            (false, false, true) => "lane_midi",
            (false, true, true) => "lane_mixed",
            _ => "lane_none",
        };
    }
    if lanes.picture.contains_key(node) {
        return if port == Some("audio") {
            "picture_audio"
        } else {
            "picture"
        };
    }
    "unknown"
}

/// The project at `roots`, read through `source` with `rig`, and everything 2e answers about it.
pub fn run(source: &dyn Source, cache: &mut Cache, roots: &Roots, rig: &Value) -> Result<Ran, SourceError> {
    let inputs = load_inputs(source, cache, roots, Rig::from_json(rig))?;
    let lanes = inputs.lanes();
    let own = chain_ownership(&inputs, &lanes);
    let live = live_inputs_of(&inputs, &lanes, &own);
    let checked = check_references(source, &inputs, &lanes)?;
    let asm = resolve_assemblies(source, cache, roots)?;
    let system = inputs
        .head
        .root
        .as_ref()
        .and_then(|r| r.project.as_ref())
        .and_then(|f| f.project.as_ref())
        .and_then(|p| p.console.as_ref())
        .map(|c| c.0.clone());
    let console = console_of(source, roots, &inputs.vocabulary, system.as_deref())?;
    let systems = console_systems(&inputs.vocabulary);

    let mut counts = Counts::default();
    let sound: Map<String, Value> = lanes
        .sound
        .iter()
        .map(|(k, l)| ((*k).to_owned(), lane_json(l)))
        .collect();
    let pictures: Map<String, Value> = lanes
        .picture
        .iter()
        .map(|(k, l)| ((*k).to_owned(), lane_json(l)))
        .collect();
    counts.sound = lanes.sound.values().filter(|l| l.kind != LaneKind::Group).count();
    counts.group = lanes.sound.values().filter(|l| l.kind == LaneKind::Group).count();
    counts.pattern = song::lanes::pattern_lanes(&lanes).len();
    counts.picture = lanes.picture.len();
    counts.frames = lanes.picture.values().filter(|l| l.frame.is_some()).count();
    counts.events = lanes.sound.values().map(|l| l.events.len()).sum();
    let tracks = video_tracks(&lanes, &inputs.bypassed);
    let video: Map<String, Value> = tracks
        .iter()
        .map(|(k, t)| {
            let mut v = json!({"name": t.name, "events": t.events.iter().map(|e| e.ref_.as_ref().map_or_else(String::new, |r| r.0.clone())).collect::<Vec<_>>()});
            let o = v.as_object_mut().expect("an object");
            if let Some(c) = t.color {
                o.insert("color".into(), json!(c));
            }
            if t.mute {
                o.insert("mute".into(), json!(true));
            }
            if let Some([x, y, w, h]) = t.frame {
                o.extend([("x".into(), json!(x)), ("y".into(), json!(y)), ("w".into(), json!(w)), ("h".into(), json!(h))]);
            }
            ((*k).to_owned(), v)
        })
        .collect();

    let mut owners = Map::new();
    let mut cued = Vec::new();
    counts.chains = inputs.chains.len();
    for name in inputs.chains.keys() {
        let owner = own.owner_of(name).unwrap_or("");
        owners.insert(name.clone(), json!(owner));
        if own.is_cued(name) {
            cued.push(name.clone());
        }
        counts.owned_apart += usize::from(!owner.is_empty() && !lanes.sound.contains_key(name.as_str()));
        counts.master += usize::from(owner == "@master");
    }
    counts.bus = own.bus_chains.len();
    counts.input = own.input_chains.len();
    counts.cued = cued.len();
    counts.tap = own.tap_chains.len();
    let tap: Vec<Value> = own
        .tap_chains
        .iter()
        .map(|(k, t)| json!([k, {"node": t.node, "pair": t.pair}]))
        .collect();
    let mut ids: Vec<&str> = inputs.chains.keys().map(String::as_str).collect();
    ids.extend(lanes.sound.keys().copied());
    ids.extend(
        inputs
            .chains
            .values()
            .flat_map(|c| c.use_steps.iter().map(|s| s.module.as_str())),
    );
    ids.push("~absent~");
    ids.sort_unstable();
    ids.dedup();
    let mut side = Map::new();
    for id in ids {
        let track = own.side_input_track(id).unwrap_or("");
        counts.side_resolved += usize::from(!track.is_empty());
        let is_step = !inputs.chains.contains_key(id) && !lanes.sound.contains_key(id) && !track.is_empty();
        counts.side_step += usize::from(is_step);
        side.insert(id.to_owned(), json!(track));
    }
    let live_json: Map<String, Value> = live
        .iter()
        .map(|(k, l)| {
            (
                k.clone(),
                json!({"channels": l.channels, "monitor": l.monitor.name(), "gain": l.gain}),
            )
        })
        .collect();
    counts.live = live.len();
    counts.live_loud = live.values().filter(|l| l.monitor.name() != "auto").count();
    counts.live_channels = live.values().filter(|l| !l.channels.is_empty()).count();
    counts.cue_applied = usize::from(!own.cue_pair.is_empty() && !cued.is_empty());

    let mut faults: Vec<String> = own.faults.iter().chain(&checked.faults).map(|f| f.key()).collect();
    faults.extend(console.fault.iter().map(|f| f.key()));
    for f in own.faults.iter().chain(&checked.faults).chain(&console.fault) {
        counts.kinds.insert(f.kind.name().to_owned());
    }
    faults.sort();
    faults.dedup();
    counts.faulty = usize::from(!faults.is_empty());
    for c in inputs.chains.values() {
        let from = c.from.as_deref().map(|f| match c.from_port.as_deref() {
            Some(p) if !p.is_empty() => format!("{f}#{p}"),
            _ => f.to_owned(),
        });
        for id in from.iter().map(String::as_str).chain(
            c.use_steps
                .iter()
                .flat_map(|s| s.inputs.iter().flatten().map(String::as_str)),
        ) {
            counts.sources.insert(source_kind(&inputs, &lanes, id));
        }
    }
    counts.assemblies = asm.assemblies.len();
    counts.plugs = asm.assemblies.values().map(|a| a.inputs.len() + a.outputs.len()).sum();
    counts.params = asm.assemblies.values().map(|a| a.params.len()).sum();
    counts.systems = systems.len();
    counts.halves = usize::from(console.channel.is_some()) + usize::from(console.buss.is_some());
    let mut issues: Vec<String> = asm
        .findings
        .iter()
        .map(|f| {
            format!(
                "{}|{}",
                f.file,
                if f.severity == Severity::Error {
                    "error"
                } else {
                    "warning"
                }
            )
        })
        .collect();
    issues.sort();
    issues.dedup();
    let stops = inputs.stops();
    let mut every: Vec<&Finding> = inputs.warnings.iter().collect();
    every.extend(inputs.stops());
    every.extend(asm.findings.iter());
    let faulted: Vec<Finding> = checked.faults.iter().map(|f| finding_of(&inputs, f)).collect();
    every.extend(faulted.iter());
    let dump = json!({
        "lanes": sound,
        "pictures": pictures,
        "video_tracks": video,
        "owners": owners,
        "bus": own.bus_chains,
        "input": own.input_chains,
        "tap": tap,
        "cued": cued,
        "cue_pair": own.cue_pair,
        "side": side,
        "live": live_json,
        "faults": faults,
        "assemblies": asm.assemblies,
        "assembly_issues": issues,
        "console": {
            "systems": systems,
            "channel": console.channel.as_ref().map(|h| h.with_id("x")),
            "buss": console.buss.as_ref().map(|h| h.with_id("master")),
            "fault": console.fault.as_ref().map(song::lanes::fault::Fault::key),
        },
    });
    Ok(Ran {
        json: dump,
        stopped: files(&stops, Severity::Error),
        warned: files(&every, Severity::Warning),
        faults,
        weight: asm.weight,
        counts,
    })
}
