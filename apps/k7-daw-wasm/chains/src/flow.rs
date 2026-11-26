// flow.rs — how hard a cable ripples: one number in 0..1 per cable, from the level, the notes or the picture a lane puts out, walked along the cables.
// A reader would get wrong that the walk runs twice (forward from each seed, then backward from it): a live input is fed by the cable behind it.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Nothing is going through: the cable hangs still.
pub const STILL: f64 = 0.0;
/// The quietest sound that still moves a cable.
pub const FLOW_FLOOR_DB: f64 = -80.0;
/// The id prefix of a lane's card in the built graph.
pub const TRACK_NODE_PREFIX: &str = "trk:";
/// The id prefix of a module's card, which a run that materialised its own lane is drawn as.
pub const MODULE_NODE_PREFIX: &str = "mod:";
/// The two sums, the other nodes the walk starts at.
pub const MIX_NODE_ID: &str = "snk:MIX";
pub const RENDER_NODE_ID: &str = "snk:RENDER";

/// What one node puts on each kind of wire leaving it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NodeFlow {
    pub audio: f64,
    pub midi: f64,
    pub video: f64,
}

/// An audio level in dB to 0..1, linear in decibels; the floor is a hair above nothing, not nothing.
pub fn audio_flow(db: f64) -> f64 {
    if !db.is_finite() || db <= FLOW_FLOOR_DB {
        return STILL;
    }
    ((db - FLOW_FLOOR_DB) / -FLOW_FLOOR_DB).min(1.0)
}

/// A MIDI cable's load to 0..1: the velocities sounding now against the busiest moment the track ever has.
pub fn midi_flow(load: f64, peak: f64) -> f64 {
    if load.is_nan() || peak.is_nan() || load <= 0.0 || peak <= 0.0 {
        return STILL;
    }
    (load / peak).min(1.0)
}

/// A video cable's, which is binary: a lane is putting a picture on screen or it is not.
pub fn video_flow(showing: bool) -> f64 {
    if showing { 1.0 } else { STILL }
}

/// What a cable of this kind takes from the node feeding it; an event follows the notes it is.
pub fn flow_of(seed: &NodeFlow, signal: Option<&str>) -> f64 {
    match signal {
        Some("video") => seed.video,
        Some("midi") | Some("event") => seed.midi,
        _ => seed.audio,
    }
}

/// What one lane is putting out: its level, its notes with the busiest they get, whether it shows a picture.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct LaneSource {
    pub db: Option<f64>,
    pub notes: Option<NoteLoad>,
    pub showing: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NoteLoad {
    pub load: f64,
    pub peak: f64,
}

/// The seed each lane contributes, by lane id; a lane that puts out nothing is left out.
pub fn seed_flows(lanes: &IndexMap<String, LaneSource>) -> IndexMap<String, NodeFlow> {
    let mut out = IndexMap::new();
    for (id, l) in lanes {
        let f = NodeFlow {
            audio: l.db.map_or(STILL, audio_flow),
            midi: l.notes.map_or(STILL, |n| midi_flow(n.load, n.peak)),
            video: video_flow(l.showing == Some(true)),
        };
        if f.audio > STILL || f.midi > STILL || f.video > STILL {
            out.insert(id.clone(), f);
        }
    }
    out
}

/// One cable as the walk reads it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct FlowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub signal: Option<String>,
}

fn dominates(a: &NodeFlow, b: &NodeFlow) -> bool {
    a.audio >= b.audio && a.midi >= b.midi && a.video >= b.video
}

fn louder(a: &NodeFlow, b: &NodeFlow) -> NodeFlow {
    NodeFlow { audio: a.audio.max(b.audio), midi: a.midi.max(b.midi), video: a.video.max(b.video) }
}

/// How hard each cable ripples now: forward from the metered lanes and both sums, and backward from each metered card.
pub fn compute_flow(
    edges: &[FlowEdge],
    seeds: &[(String, NodeFlow)],
    folded_at: &HashMap<String, String>,
) -> IndexMap<String, f64> {
    let mut reached: HashMap<String, NodeFlow> = HashMap::new();
    let mut seed = |node: String, f: NodeFlow| {
        let next = match reached.get(&node) {
            Some(had) => louder(had, &f),
            None => f,
        };
        reached.insert(node, next);
    };
    for (id, f) in seeds {
        seed(format!("{TRACK_NODE_PREFIX}{id}"), *f);
        seed(format!("{MODULE_NODE_PREFIX}{id}"), *f);
        if let Some(boxed) = folded_at.get(id) {
            seed(boxed.clone(), *f);
        }
        if f.audio > STILL {
            seed(MIX_NODE_ID.to_owned(), *f);
        }
        if f.video > STILL {
            seed(RENDER_NODE_ID.to_owned(), *f);
        }
    }
    let mut flow: IndexMap<String, f64> = IndexMap::new();
    for back in [false, true] {
        let mut seen = reached.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for g in edges {
                let (here, next) = if back { (&g.target, &g.source) } else { (&g.source, &g.target) };
                if back && next.starts_with(TRACK_NODE_PREFIX) {
                    continue;
                }
                let Some(f) = seen.get(here).copied() else { continue };
                let v = flow_of(&f, g.signal.as_deref());
                let had = flow.get(&g.id).copied().unwrap_or(0.0);
                if v > had {
                    flow.insert(g.id.clone(), v);
                } else if seen.contains_key(next) {
                    continue;
                }
                match seen.get(next).copied() {
                    Some(was) if dominates(&was, &f) => {}
                    Some(was) => {
                        seen.insert(next.clone(), louder(&was, &f));
                        changed = true;
                    }
                    None => {
                        seen.insert(next.clone(), f);
                        changed = true;
                    }
                }
            }
        }
    }
    flow
}

#[cfg(test)]
mod tests;
