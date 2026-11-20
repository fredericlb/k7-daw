// lanes/rig.rs — this machine's rig as the host hands it: channel labels in and out, and the cue pair; read from JSON, never failing.
// A reader would get wrong that a channel is read as JavaScript's `Number(v)` reads it and a label as `String(v)`: `"3"` and `true` count (250).

use super::jsvalue::{to_number, to_text};
use crate::files::scalar::js_space;
use crate::modules::Input;
use serde_json::Value;
use std::collections::HashMap;

/// At most this many channels one label may name; the rest are dropped (250).
pub const MAX_CHANNELS: usize = 64;
/// At most this many labels one direction keeps; the rest are dropped (250).
pub const MAX_LABELS: usize = 4_096;

/// One label of the interface: its 1-based channels and what the person calls them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelLabel {
    pub ch: Vec<u32>,
    pub label: String,
}

/// The pair the CUE bus leaves by: a label of the outputs, or the numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueOutput {
    Label(String),
    Channels(Vec<u32>),
}

/// The rig, the empty one by default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rig {
    pub channels_in: Vec<ChannelLabel>,
    pub channels_out: Vec<ChannelLabel>,
    pub cue_output: Option<CueOutput>,
}

/// Physical channels, and the label that named none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Channels {
    pub channels: Vec<u32>,
    pub unknown_label: Option<String>,
}

/// A value that `Number(v)` makes a whole number from 1 to `u32::MAX`; past that no device has a channel, and a channel is never `f64`.
fn channel_of(v: &Value) -> Option<u32> {
    let f = to_number(v);
    (f.fract() == 0.0 && f >= 1.0 && f <= f64::from(u32::MAX)).then_some(f as u32)
}

fn channels_of(list: &[Value]) -> Vec<u32> {
    list.iter().filter_map(channel_of).take(MAX_CHANNELS).collect()
}

fn labels_of(v: Option<&Value>) -> Vec<ChannelLabel> {
    let mut out = Vec::new();
    for e in v.and_then(Value::as_array).into_iter().flatten() {
        if out.len() >= MAX_LABELS {
            break;
        }
        let ch = match e.get("ch") {
            Some(Value::Array(list)) => channels_of(list),
            Some(one) => channels_of(std::slice::from_ref(one)),
            None => Vec::new(),
        };
        let label = match e.get("label") {
            None | Some(Value::Null) => String::new(),
            Some(v) => to_text(v),
        };
        if !ch.is_empty() && !label.is_empty() {
            out.push(ChannelLabel { ch, label });
        }
    }
    out
}

fn first_wins(list: &[ChannelLabel]) -> HashMap<&str, &[u32]> {
    let mut m: HashMap<&str, &[u32]> = HashMap::new();
    for e in list {
        m.entry(e.label.as_str()).or_insert(e.ch.as_slice());
    }
    m
}

impl Rig {
    /// The rig a host's JSON states; anything unreadable is dropped and the rest stands (250).
    pub fn from_json(v: &Value) -> Rig {
        let channels = v.get("channels");
        let cue_output = match v.get("cueOutput") {
            Some(Value::Array(list)) => Some(channels_of(list))
                .filter(|c| !c.is_empty())
                .map(CueOutput::Channels),
            Some(Value::String(s)) if !s.trim_matches(js_space).is_empty() => {
                Some(CueOutput::Label(s.trim_matches(js_space).to_owned()))
            }
            _ => None,
        };
        Rig {
            channels_in: labels_of(channels.and_then(|c| c.get("in"))),
            channels_out: labels_of(channels.and_then(|c| c.get("out"))),
            cue_output,
        }
    }

    /// The first-wins index of each direction, built once for a pass that resolves many inputs (251).
    pub fn index(&self) -> RigIndex<'_> {
        RigIndex {
            rig: self,
            inputs: first_wins(&self.channels_in),
            outputs: first_wins(&self.channels_out),
        }
    }
}

/// A rig with its labels indexed: the answers of `resolve_input` and `resolve_cue_output` without a walk each.
pub struct RigIndex<'r> {
    rig: &'r Rig,
    inputs: HashMap<&'r str, &'r [u32]>,
    outputs: HashMap<&'r str, &'r [u32]>,
}

fn by_label<'r>(label: &str, find: impl FnOnce() -> Option<&'r [u32]>) -> Channels {
    if label.is_empty() {
        return Channels::default();
    }
    match find() {
        Some(ch) => Channels {
            channels: ch.to_vec(),
            unknown_label: None,
        },
        None => Channels {
            channels: Vec::new(),
            unknown_label: Some(label.to_owned()),
        },
    }
}

/// The channels of the first entry called `label`.
fn first_hit<'r>(list: &'r [ChannelLabel], label: &str) -> Option<&'r [u32]> {
    list.iter().find(|e| e.label == label).map(|e| e.ch.as_slice())
}

fn numbers(list: &[u64]) -> Channels {
    Channels {
        channels: list
            .iter()
            .filter_map(|n| u32::try_from(*n).ok().filter(|n| *n > 0))
            .collect(),
        unknown_label: None,
    }
}

impl RigIndex<'_> {
    /// A module's `channel` against the input labels: numbers as they stand, a label looked up.
    pub fn input(&self, input: Option<&Input>) -> Channels {
        match input {
            None => Channels::default(),
            Some(Input::Channels(list)) => numbers(list),
            Some(Input::Label(l)) => by_label(l, || self.inputs.get(l.as_str()).copied()),
        }
    }

    /// The CUE bus's pair against the output labels; empty means this machine has no headphone pair.
    pub fn cue_output(&self) -> Channels {
        match &self.rig.cue_output {
            None => Channels::default(),
            Some(CueOutput::Channels(c)) => Channels {
                channels: c.clone(),
                unknown_label: None,
            },
            Some(CueOutput::Label(l)) => by_label(l, || self.outputs.get(l.as_str()).copied()),
        }
    }
}

/// A track's `input` against the rig; an unresolvable label reports itself rather than monitoring wrongly (289).
pub fn resolve_input(input: Option<&Input>, rig: &Rig) -> Channels {
    match input {
        None => Channels::default(),
        Some(Input::Channels(list)) => numbers(list),
        Some(Input::Label(l)) => by_label(l, || first_hit(&rig.channels_in, l)),
    }
}

/// The CUE bus's pair against the OUT labels (289).
pub fn resolve_cue_output(rig: &Rig) -> Channels {
    match &rig.cue_output {
        None => Channels::default(),
        Some(CueOutput::Channels(c)) => Channels {
            channels: c.clone(),
            unknown_label: None,
        },
        Some(CueOutput::Label(l)) => by_label(l, || first_hit(&rig.channels_out, l)),
    }
}
