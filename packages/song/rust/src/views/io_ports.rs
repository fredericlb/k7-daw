// views/io_ports.rs — the project's declared I/O: every `io: in` or `io: out` module it can reach, channels resolved on this rig.
// A reader would get wrong that a node counts only through what it EXTENDS (resolved), and that the output is weighed (decision 624).

use super::places::node_places_of;
use super::{Spend, capped_finding};
use crate::files::Finding;
use crate::lanes::inputs::Inputs;
use crate::lanes::rig::{Channels, MAX_CHANNELS, RigIndex};
use crate::modules::types::split_module_key;
use crate::modules::{Input, IoKind, LoadedModule, ModuleKind};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum IoDir {
    In,
    Out,
}

/// One port of the I/O panel: how a chain names it, and what a rig makes of its channel.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct IoPort {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub name: String,
    pub dir: IoDir,
    pub channels: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub unknown_label: Option<bool>,
    pub namespace: String,
    pub generic: bool,
}

/// The ports, what listing them found, and whether the budget stopped it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IoPorts {
    pub ports: Vec<IoPort>,
    pub findings: Vec<Finding>,
    pub capped: bool,
    pub spent: usize,
}

/// A ref split into its namespace and the part worth showing: `@engine/io/IN:in-1` is `engine` and `IN:in-1`.
fn describe(r: &str) -> (String, String) {
    if !r.starts_with('@') {
        return (r.to_owned(), String::new());
    }
    let ns = r[1..].split('/').next().unwrap_or("");
    let path = r.splitn(3, '/').nth(2).unwrap_or("");
    (if path.is_empty() { r } else { path }.to_owned(), ns.to_owned())
}

/// How many entries of a numbered `channel` are looked at: a channel is one or two numbers, and one past 64 is a typo.
const CHANNEL_WINDOW: usize = 256;

/// A port's channels on this rig, at most `MAX_CHANNELS`, whatever the list a module states (decision 624).
fn channels_of(m: &LoadedModule, rig: &RigIndex<'_>) -> Channels {
    match &m.authored.channel {
        Some(Input::Channels(list)) => Channels {
            channels: list
                .iter()
                .take(CHANNEL_WINDOW)
                .filter_map(|n| u32::try_from(*n).ok().filter(|n| *n > 0))
                .take(MAX_CHANNELS)
                .collect(),
            unknown_label: None,
        },
        other => {
            let mut c = rig.input(other.as_ref());
            c.channels.truncate(MAX_CHANNELS);
            c
        }
    }
}

fn dir_of(m: &LoadedModule) -> Option<IoDir> {
    if m.kind != Some(ModuleKind::Io) {
        return None;
    }
    match m.authored.io {
        Some(IoKind::In) => Some(IoDir::In),
        Some(IoKind::Out) => Some(IoDir::Out),
        _ => None,
    }
}

/// Every io module the project can reach, inputs first; the raw vocabulary, then the project's own resolved nodes.
pub fn io_ports(inputs: &Inputs) -> IoPorts {
    let rig = inputs.rig.index();
    let places = inputs.routing.project.as_ref().map(node_places_of).unwrap_or_default();
    let mut spend = Spend::new();
    let mut ports: Vec<IoPort> = Vec::new();
    let mut port = |r: &str, m: &LoadedModule, dir: IoDir, spend: &mut Spend| -> bool {
        let (mut name, namespace) = describe(r);
        let place = places.get(r);
        if let Some(p) = place {
            name.clone_from(p);
        }
        let channels = channels_of(m, &rig);
        let label = match &m.authored.channel {
            Some(Input::Label(l)) => Some(l.clone()),
            _ => None,
        };
        let weight = r.len() + name.len() + label.as_ref().map_or(0, String::len) + channels.channels.len() * 8;
        if !spend.bytes(weight) {
            return false;
        }
        ports.push(IoPort {
            ref_: r.to_owned(),
            name,
            dir,
            channels: channels.channels,
            label,
            unknown_label: channels.unknown_label.map(|_| true),
            namespace,
            generic: place.is_none() && split_module_key(r).1.is_none(),
        });
        true
    };
    'walk: {
        for (r, m) in &inputs.vocabulary.modules {
            if !spend.step() {
                break 'walk;
            }
            if let Some(dir) = dir_of(m)
                && !port(r, m, dir, &mut spend)
            {
                break 'walk;
            }
        }
        for (key, raw) in &inputs.nodes.nodes {
            if !spend.step() {
                break 'walk;
            }
            if inputs.vocabulary.modules.contains_key(key) {
                continue;
            }
            // An unresolvable node is read as written.
            let m: &LoadedModule = inputs.local.modules.get(key).map_or(raw, |r| &**r);
            if let Some(dir) = dir_of(m)
                && !port(key, m, dir, &mut spend)
            {
                break 'walk;
            }
        }
    }
    ports.sort_by(|a, b| {
        (a.dir == IoDir::Out)
            .cmp(&(b.dir == IoDir::Out))
            .then_with(|| a.ref_.cmp(&b.ref_))
    });
    let findings = if spend.capped() {
        vec![capped_finding("project.k7x", "the I/O ports")]
    } else {
        Vec::new()
    };
    IoPorts {
        ports,
        findings,
        capped: spend.capped(),
        spent: spend.spent(),
    }
}
