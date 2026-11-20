// lanes/wiring.rs — every chain must feed each module the input ports its type declares; the type of a source id, said once per lane.
// A reader would get wrong that nothing is charged (268): each step's sources are compared with its ports up to the shorter list.
// A lane's signal is memoised by its name, so 100,000 ports naming one lane of 100,000 events walk it once (269).

use super::references::{Fault, FaultKind, Holder, MAX_FAULTS};
use super::{Lane, LaneKind, Material, lane_materials, lane_signal};
use crate::files::scalar::{CAPTION_EXTS, IMAGE_EXTS, VIDEO_EXTS};
use crate::files::{Clip, Sequence, has_ext};
use crate::modules::{Port, PortType};
use crate::routing::{Chain, is_parked, port_ref, split_port};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// The signal a port carries: audio, midi or video.
pub type Signal = PortType;

/// What the wiring check reads; `inputs_of` is a closure so 2k can pass its own map (268).
pub struct WiringInput<'a> {
    pub chains: &'a IndexMap<String, Chain>,
    pub inputs_of: &'a dyn Fn(&str) -> Option<&'a [Port]>,
    pub sound: &'a IndexMap<&'a str, Lane<'a>>,
    pub clips: &'a HashMap<&'a str, &'a Clip>,
    pub sequences: &'a HashMap<&'a str, &'a Sequence>,
    pub picture: &'a dyn Fn(&str) -> bool,
    pub tap_fed: &'a HashSet<String>,
}

/// A lane's signal by its name, kept for the pass: a lane is asked about once however many ports name it.
pub type SignalMemo = HashMap<String, Option<Signal>>;

/// The file a clip's source names, whatever kind of clip it is.
pub fn clip_source(c: &Clip) -> &str {
    match c {
        Clip::Audio(a) => &a.source.0,
        Clip::Video(v) => &v.source.0,
        Clip::Caption(c) => &c.source.0,
    }
}

/// A source the picture reads: a video, a still or captions (song-video docs/media.md).
pub fn reads_as_picture(source: &str) -> bool {
    has_ext(source, VIDEO_EXTS) || has_ext(source, IMAGE_EXTS) || has_ext(source, CAPTION_EXTS)
}

/// The signal a lane puts on a wire, None when it cannot be said (a group, a generated lane, a mixed or unknown one).
fn track_type(
    node: &str,
    lane: &Lane<'_>,
    port: Option<&str>,
    w: &WiringInput<'_>,
    memo: &mut SignalMemo,
) -> Option<Signal> {
    if lane.kind == LaneKind::Group {
        return None;
    }
    match port {
        Some("midi") => return Some(Signal::Midi),
        Some("audio") => return Some(Signal::Audio),
        _ => {}
    }
    if let Some(known) = memo.get(node) {
        return *known;
    }
    let material = |r: &str| {
        if w.sequences.contains_key(r) {
            Material::Midi
        } else if w.clips.contains_key(r) {
            Material::Audio
        } else {
            Material::Missing
        }
    };
    let said = match lane_signal(&lane_materials(lane.events, &material)) {
        Some(Material::Audio) => Some(Signal::Audio),
        Some(Material::Midi) => Some(Signal::Midi),
        _ => None,
    };
    memo.insert(node.to_owned(), said);
    said
}

/// The type a chain-input source id resolves to, or None when unknowable (269).
pub fn source_type(id: &str, w: &WiringInput<'_>, memo: &mut SignalMemo) -> Option<Signal> {
    if w.sequences.contains_key(id) {
        return Some(Signal::Midi);
    }
    if let Some(clip) = w.clips.get(id) {
        return Some(if reads_as_picture(clip_source(clip)) {
            Signal::Video
        } else {
            Signal::Audio
        });
    }
    let (node, port) = split_port(id);
    if let Some(lane) = w.sound.get(node) {
        return track_type(node, lane, port, w, memo);
    }
    if (w.picture)(node) {
        return Some(if port == Some("audio") {
            Signal::Audio
        } else {
            Signal::Video
        });
    }
    None
}

/// Whether the list is past `MAX_FAULTS`: it is then cut there and closed with one `More` (260, 290).
fn stopped(faults: &mut Vec<Fault>) -> bool {
    if faults.len() <= MAX_FAULTS {
        return false;
    }
    faults.truncate(MAX_FAULTS);
    faults.push(Fault::new(FaultKind::More, Holder::File(String::new()), ""));
    true
}

/// Every chain must feed each module the input ports its type declares (docs/graphModel.md); at most `MAX_FAULTS`, then one `More`.
pub fn check_chain_inputs(w: &WiringInput<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut memo = SignalMemo::new();
    let mut mandatory: HashMap<&str, usize> = HashMap::new();
    // Chains routed into by another chain's `out`, so their first module has a flow even without a `from`.
    let routed_into: HashSet<&str> = w
        .chains
        .values()
        .flat_map(|c| c.out.iter().map(|o| o.to.as_str()))
        .collect();
    for (name, c) in w.chains {
        if is_parked(c) {
            continue;
        }
        let from = c.from.as_deref().filter(|f| !f.is_empty());
        if let Some(f) = from
            && w.sound.get(f).is_some_and(|l| l.kind == LaneKind::Group)
        {
            faults.push(Fault::at_chain(FaultKind::GroupSource, name, None, f));
            if stopped(&mut faults) {
                return faults;
            }
        }
        for (idx, step) in c.use_steps.iter().enumerate() {
            if step.enabled == Some(false) {
                continue;
            }
            let Some(ports) = (w.inputs_of)(&step.module).filter(|p| !p.is_empty()) else {
                continue;
            };
            let tapped = idx == 0 && w.tap_fed.contains(&step.module);
            let shot = idx == 0
                && from.is_none()
                && !routed_into.contains(name.as_str())
                && ports.first().is_some_and(|p| p.kind == Signal::Video)
                && c.out.iter().any(|o| (w.picture)(split_port(&o.to).0));
            let has_flow = idx > 0 || from.is_some() || routed_into.contains(name.as_str()) || tapped || shot;
            let own = usize::from(has_flow);
            let extra: &[String] = step.inputs.as_deref().unwrap_or_default();
            let count = own + extra.len();
            let required = *mandatory
                .entry(step.module.as_str())
                .or_insert_with(|| ports.iter().filter(|p| p.mandatory != Some(false)).count());
            let fault = |kind: FaultKind, target: &str| Fault::at_chain(kind, name, Some(idx + 1), target);
            if count < required {
                faults.push(fault(FaultKind::TooFew, &step.module));
            } else if count > ports.len() {
                faults.push(fault(FaultKind::TooMany, &step.module));
            }
            // Positional type check, where the source's type is knowable: up to the shorter list.
            for (i, port) in ports.iter().enumerate().take(count) {
                let kind = if has_flow && i == 0 {
                    match (idx == 0, from, tapped, shot) {
                        (true, Some(f), _, _) => {
                            let id = match c.from_port.as_deref() {
                                Some(p) if !p.is_empty() => port_ref(f, Some(p)),
                                _ => f.to_owned(),
                            };
                            source_type(&id, w, &mut memo)
                        }
                        (true, None, true, _) => Some(Signal::Audio),
                        (true, None, false, true) => Some(Signal::Video),
                        _ => None,
                    }
                } else {
                    extra.get(i - own).and_then(|s| source_type(s, w, &mut memo))
                };
                if kind.is_some_and(|k| k != port.kind) {
                    faults.push(fault(FaultKind::PortType, &port.name));
                }
            }
            if stopped(&mut faults) {
                return faults;
            }
        }
    }
    faults
}
