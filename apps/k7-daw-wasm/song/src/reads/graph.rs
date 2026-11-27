// reads/graph.rs — what the graph document says of a lane: the MIDI device that records into it, the input feeding it, the rack it reaches.
// A reader would get wrong that no midi-in node is not "any device": an empty label is any device, no node is no MIDI at all.

use super::model::{RChain, RGraph, RRack};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

/// The first segment of a port ref: `piano#midi` names a port of the piano.
fn node_of(to: &str) -> &str {
    to.split('#').next().unwrap_or(to)
}

fn feeds(c: &RChain, target: &str) -> bool {
    c.out.as_ref().is_some_and(|o| o.iter().any(|o| o.to == target))
}

/// The label of the midi-in node feeding `track` (`""` any device), or none when none is wired to it.
pub fn midi_in_device_for(g: &RGraph, track: &str) -> Option<String> {
    for c in g.chains.values() {
        if !feeds(c, track) {
            continue;
        }
        if let Some(step) = c.r#use.iter().find(|s| s.device.is_some()) {
            return Some(step.device.clone().unwrap_or_default());
        }
    }
    None
}

/// Every lane a midi-in node is wired to: the lanes that arm in the browser before they hold a sequence.
pub fn midi_in_lanes(g: &RGraph) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in g.chains.values() {
        if c.r#use.iter().any(|s| s.device.is_some()) {
            for o in c.out.iter().flatten() {
                if seen.insert(o.to.clone()) {
                    out.push(o.to.clone());
                }
            }
        }
    }
    out
}

/// An instrument a keyboard is wired straight into, no lane between: the engine track it plays and the node's label.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlayedTarget {
    pub track: String,
    pub device: String,
}

/// Every instrument a midi-in node feeds directly, played live whatever is armed and never recorded.
pub fn midi_in_played(g: &RGraph) -> Vec<PlayedTarget> {
    let lanes: HashSet<&str> = g.tracks.iter().flatten().map(String::as_str).collect();
    let mut out = Vec::new();
    for c in g.chains.values() {
        let Some(step) = c.r#use.iter().find(|s| s.device.is_some()) else { continue };
        for o in c.out.iter().flatten() {
            let to = node_of(&o.to);
            if lanes.contains(to) {
                continue;
            }
            // The engine runs that instrument as the track of the run it starts, named after that run.
            let run = g.chains.iter().find(|(_, r)| r.r#use.first().is_some_and(|s| s.module == to)).map(|(k, _)| k.as_str());
            out.push(PlayedTarget { track: run.unwrap_or(to).to_owned(), device: step.device.clone().unwrap_or_default() });
        }
    }
    out
}

/// A live input node as its file states it: the module to write, and the fields the title bar shows.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneInput {
    pub module: String,
    #[cfg_attr(feature = "ts", ts(type = "\"audio\" | \"midi\" | \"camera\""))]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub channel: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "\"off\" | \"auto\" | \"on\""))]
    pub monitor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub device: Option<String>,
}

/// The first live input (`monitor` or `device`) feeding `track`, directly or through other chains.
pub fn lane_input(g: &RGraph, track: &str) -> Option<LaneInput> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut todo = vec![track.to_owned()];
    while let Some(target) = todo.pop() {
        if !seen.insert(target.clone()) {
            continue;
        }
        for c in g.chains.values() {
            if !c.out.iter().flatten().any(|o| node_of(&o.to) == target) {
                continue;
            }
            if let Some(s) = c.r#use.iter().find(|s| s.monitor.is_some() || s.device.is_some()) {
                let kind = if s.device.is_some() {
                    "midi"
                } else if s.camera.is_some() {
                    "camera"
                } else {
                    "audio"
                };
                return Some(LaneInput {
                    module: s.module.clone(),
                    kind: kind.to_owned(),
                    channel: s.channel.clone(),
                    monitor: s.monitor.clone(),
                    device: s.device.clone(),
                });
            }
            let head = c.from.clone().or_else(|| c.r#use.first().map(|s| s.module.clone()));
            if let Some(h) = head.filter(|h| !h.is_empty()) {
                todo.push(h);
            }
        }
    }
    None
}

/// The sounds of the first rack downstream of `track`, walked in signal order, bypassed steps counting, cycle-safe.
pub fn rack_for(g: &RGraph, track: &str) -> Option<Vec<RRack>> {
    if track.is_empty() {
        return None;
    }
    let mut queue: std::collections::VecDeque<&str> =
        g.chains.iter().filter(|(_, c)| c.from.as_deref() == Some(track)).map(|(k, _)| k.as_str()).collect();
    let mut seen: HashSet<&str> = queue.iter().copied().collect();
    while let Some(name) = queue.pop_front() {
        let Some(chain) = g.chains.get(name) else { continue };
        for step in &chain.r#use {
            if let Some(r) = step.rack.as_ref().filter(|r| !r.is_empty()) {
                return Some(r.clone());
            }
        }
        for out in chain.out.iter().flatten() {
            if seen.contains(out.to.as_str()) {
                continue;
            }
            if let Some((k, _)) = g.chains.get_key_value(out.to.as_str()) {
                seen.insert(k.as_str());
                queue.push_back(k.as_str());
            }
        }
    }
    None
}

/// Each rack key's name for the roll's gutter: its sample's file name, else its clip, else its symbol; several on one key joined.
pub fn rack_key_names(rack: &[RRack]) -> Vec<(f64, String)> {
    let mut names: Vec<(f64, String)> = Vec::new();
    for s in rack {
        let file = s.sample.as_deref().and_then(|p| p.rsplit('/').next()).map(|f| match f.rfind('.') {
            Some(i) if i + 1 < f.len() => &f[..i],
            _ => f,
        });
        let name = file
            .filter(|f| !f.is_empty())
            .or(s.clip.as_deref().filter(|c| !c.is_empty()))
            .unwrap_or(&s.symbol);
        match names.iter_mut().find(|(n, _)| *n == s.note) {
            Some((_, had)) => *had = format!("{had} / {name}"),
            None => names.push((s.note, name.to_owned())),
        }
    }
    names
}

/// A node ref without its port: everything before the last `#`, the whole ref when that leaves nothing or no port follows.
pub(super) fn node_without_port(r: &str) -> &str {
    match r.rfind('#') {
        Some(cut) if cut > 0 && cut + 1 < r.len() => &r[..cut],
        _ => r,
    }
}

/// The graph without its notes (`@meta/doc` nodes), for a view that does not draw them.
pub fn without_notes(mut graph: Value, notes: &HashSet<String>) -> Value {
    if notes.is_empty() {
        return graph;
    }
    let Some(chains) = graph.get_mut("chains").and_then(Value::as_object_mut) else { return graph };
    for chain in chains.values_mut() {
        let Some(c) = chain.as_object_mut() else { continue };
        let uses: Vec<Value> = match c.get("use") {
            Some(Value::Array(a)) => a
                .iter()
                .filter(|s| !s.get("module").and_then(Value::as_str).is_some_and(|m| notes.contains(m)))
                .cloned()
                .collect(),
            _ => Vec::new(),
        };
        let outs: Vec<Value> = match c.get("out") {
            Some(Value::Array(a)) => a
                .iter()
                .filter(|o| !o.get("to").and_then(Value::as_str).is_some_and(|t| notes.contains(node_without_port(t))))
                .cloned()
                .collect(),
            _ => Vec::new(),
        };
        c.insert("use".to_owned(), Value::Array(uses));
        c.insert("out".to_owned(), Value::Array(outs));
    }
    graph
}

/// A line that hands a live input a channel: the node card's pick.
pub fn gives_channel(line: &str) -> bool {
    let Some(rest) = line.trim().strip_prefix("node param ") else { return false };
    let Some(end) = rest.find(char::is_whitespace) else { return false };
    if end == 0 {
        return false;
    }
    rest[end..].strip_prefix(" @channel ").and_then(|t| t.chars().next()).is_some_and(|c| !c.is_whitespace())
}

/// Whether the graph holds an input that is monitored and reaches a channel: the state that can feed back.
pub fn has_audible_input(g: &RGraph) -> bool {
    g.chains
        .values()
        .any(|c| c.r#use.iter().any(|s| s.monitor.as_deref() == Some("on") && s.channel.as_ref().is_some_and(|c| !c.is_empty())))
}
