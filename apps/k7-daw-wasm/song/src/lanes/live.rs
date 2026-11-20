// lanes/live.rs — each track's live input: an `io: in` module collapsed onto the track its chain lands on, against this machine's rig.
// A reader would get wrong that the INPUT is found first, then the track: the other way round skipped every chain with no owner.

use super::Lanes;
use super::inputs::Inputs;
use super::ownership::Ownership;
use crate::modules::Monitor;
use crate::routing::Chain;
use indexmap::IndexMap;

/// How an input is monitored: the engine's vocabulary has no "absent", so a file that says nothing is `Auto`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorMode {
    Off,
    Auto,
    On,
}

impl MonitorMode {
    pub fn name(self) -> &'static str {
        match self {
            MonitorMode::Off => "off",
            MonitorMode::Auto => "auto",
            MonitorMode::On => "on",
        }
    }
}

/// An input's `monitor`, read the one way the graph and the compile read it: a boolean is on or off (257).
pub fn monitor_mode(m: Option<&Monitor>) -> MonitorMode {
    match m {
        Some(Monitor::Flag(true)) => MonitorMode::On,
        Some(Monitor::Flag(false)) => MonitorMode::Off,
        Some(Monitor::Mode(s)) => match s.as_str() {
            "off" => MonitorMode::Off,
            "on" => MonitorMode::On,
            _ => MonitorMode::Auto,
        },
        None => MonitorMode::Auto,
    }
}

/// One track's live input.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveInput {
    pub channels: Vec<u32>,
    pub monitor: MonitorMode,
    pub gain: f64,
}

/// The track a chain's live input lands on: its source when the input is a step ON it, else the first lane its run ends at (258).
pub fn input_landing_track<'c>(c: &'c Chain, is_track: &dyn Fn(&str) -> bool) -> Option<&'c str> {
    if let Some(from) = c.from.as_deref().filter(|f| !f.is_empty()) {
        return Some(from);
    }
    c.out.iter().map(|o| o.to.as_str()).find(|to| is_track(to))
}

/// Each track's live input, the first on a track winning (257).
pub fn live_inputs_of(inputs: &Inputs, lanes: &Lanes<'_>, own: &Ownership) -> IndexMap<String, LiveInput> {
    let index = inputs.rig.index();
    let mut out: IndexMap<String, LiveInput> = IndexMap::new();
    for (name, c) in &inputs.chains {
        // The INPUT first, then the track it lands on.
        let Some(b) = c
            .use_steps
            .iter()
            .find(|s| inputs.is_input(&s.module))
            .and_then(|s| inputs.module(&s.module))
        else {
            continue;
        };
        let track = own
            .owner_of(name)
            .or_else(|| input_landing_track(c, &|id| lanes.sound.contains_key(id)));
        let Some(track) = track.filter(|t| !t.is_empty()) else {
            continue;
        };
        if out.contains_key(track) {
            continue;
        }
        out.insert(
            track.to_owned(),
            LiveInput {
                channels: index.input(b.authored.channel.as_ref()).channels,
                monitor: monitor_mode(b.authored.monitor.as_ref()),
                gain: b.authored.gain.filter(|g| g.is_finite()).unwrap_or(0.0),
            },
        );
    }
    out
}
