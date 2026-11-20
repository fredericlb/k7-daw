// lanes/mod.rs — which nodes are lanes and what each carries: the one projection `lane_of`, borrowed from the resolved module.
// A reader would get wrong that a lane is decided on the project's OWN nodes: a step naming `@meta/track` is not a lane (244).
// Everything else of sub-step 2e (inputs, ownership, live inputs, the check) stands on these types; docs/rustCompilerLanes.md.

pub mod cycles;
pub mod fault;
pub mod inputs;
pub mod jsvalue;
pub mod live;
pub mod ownership;
pub mod references;
pub mod rig;
pub mod video;
pub mod wiring;

use crate::files::Placement;
use crate::modules::ModuleKind;
use crate::resolve::Resolved;
use indexmap::IndexMap;
use serde::Serialize;

/// The reserved owner of the master chain, which no lane or chain can be called.
pub const MASTER_OWNER: &str = "@master";
/// The built-in outputs a run ends at by name.
pub const OUT_NODE: &str = "OUT";
pub const CUE_NODE: &str = "CUE";
pub const VIDEO_OUT_NODE: &str = "VIDEO_OUT";

/// The five built-in outputs; a run ending at one is never a bus.
pub fn is_builtin_output(id: &str) -> bool {
    matches!(
        id,
        crate::routing::MIX_NODE | OUT_NODE | crate::routing::RENDER_NODE | VIDEO_OUT_NODE | CUE_NODE
    )
}

/// The four kinds of lane, as `laneOf` names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LaneKind {
    Single,
    Group,
    Strudel,
    Picture,
}

/// A resolved lane module as an arrangement; it borrows the module, so the events are never copied (241).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Lane<'m> {
    #[serde(rename = "type")]
    pub kind: LaneKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'m str>,
    pub events: &'m [Placement],
    pub tracks: &'m [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strudel: Option<&'m str>,
    #[serde(rename = "videoSource", skip_serializing_if = "Option::is_none")]
    pub video_source: Option<&'m str>,
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<[f64; 4]>,
}

/// The lanes of a resolved map by node name, in that map's order: sound lanes and picture lanes apart.
#[derive(Debug, Default)]
pub struct Lanes<'m> {
    pub sound: IndexMap<&'m str, Lane<'m>>,
    pub picture: IndexMap<&'m str, Lane<'m>>,
}

fn is_track(m: &Resolved) -> bool {
    m.kind == Some(ModuleKind::Track)
}

/// A group lane is asked by its PORTS, so a node of one is still a group.
pub fn is_group_track(m: &Resolved) -> bool {
    is_track(m) && m.outputs().is_empty()
}

/// A picture lane is asked of the resolved module, so a node extending one is still one.
pub fn is_picture_track(m: &Resolved) -> bool {
    is_track(m) && m.picture == Some(true)
}

fn non_empty(s: Option<&str>) -> Option<&str> {
    s.filter(|s| !s.is_empty())
}

/// `m` seen as a lane, or None when it is not one (241).
pub fn lane_of(m: &Resolved) -> Option<Lane<'_>> {
    if !is_track(m) {
        return None;
    }
    let (group, strudel, picture) = (is_group_track(m), m.generated == Some(true), is_picture_track(m));
    let kind = if group {
        LaneKind::Group
    } else if strudel {
        LaneKind::Strudel
    } else if picture {
        LaneKind::Picture
    } else {
        LaneKind::Single
    };
    let a = &m.authored;
    Some(Lane {
        kind,
        color: non_empty(a.color.as_deref()),
        events: if group || strudel {
            &[]
        } else {
            a.events.as_deref().unwrap_or_default()
        },
        tracks: if group {
            a.tracks.as_deref().unwrap_or_default()
        } else {
            &[]
        },
        strudel: if strudel {
            non_empty(a.strudel.as_ref().map(|s| s.0.as_str()))
        } else {
            None
        },
        video_source: if group || strudel {
            None
        } else {
            non_empty(a.video_source.as_ref().map(|s| s.0.as_str()))
        },
        disabled: a.disabled == Some(true),
        frame: if picture { a.frame } else { None },
    })
}

/// Every lane of a RESOLVED module map, sound and picture apart, in that map's order.
pub fn lanes_of(resolved: &IndexMap<String, Resolved>) -> Lanes<'_> {
    let mut out = Lanes::default();
    for (name, m) in resolved {
        let Some(lane) = lane_of(m) else { continue };
        if lane.kind == LaneKind::Picture {
            out.picture.insert(name.as_str(), lane);
        } else {
            out.sound.insert(name.as_str(), lane);
        }
    }
    out
}

/// A lane the engine plays a pattern on (242).
pub fn is_pattern_lane(l: &Lane<'_>) -> bool {
    l.kind == LaneKind::Strudel && l.strudel.is_some() && !l.disabled
}

/// The pattern lanes as `(lane, strudel)`, in the map's order: what `pattern_shares` takes.
pub fn pattern_lanes<'a>(lanes: &'a Lanes<'a>) -> Vec<(&'a str, &'a str)> {
    lanes
        .sound
        .iter()
        .filter(|(_, l)| is_pattern_lane(l))
        .filter_map(|(name, l)| l.strudel.map(|s| (*name, s)))
        .collect()
}

/// What one event's ref resolves to; `Missing` names nothing the project holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Audio,
    Midi,
    Missing,
}

/// Which materials a list of events holds: a summary nothing stores.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaneMaterials {
    pub audio: bool,
    pub midi: bool,
    pub missing: bool,
}

/// What a lane holds, event by event; an absent or empty `ref` asks `material_of("")` (243).
pub fn lane_materials(events: &[Placement], material_of: &dyn Fn(&str) -> Material) -> LaneMaterials {
    let mut out = LaneMaterials::default();
    for p in events {
        match material_of(p.ref_.as_ref().map_or("", |r| r.0.as_str())) {
            Material::Audio => out.audio = true,
            Material::Midi => out.midi = true,
            Material::Missing => out.missing = true,
        }
    }
    out
}

/// The ONE signal a lane puts on a wire, None when it cannot be said (a generated lane answers none).
pub fn lane_signal(m: &LaneMaterials) -> Option<Material> {
    if m.missing || (m.audio && m.midi) {
        return None;
    }
    if m.audio {
        Some(Material::Audio)
    } else if m.midi {
        Some(Material::Midi)
    } else {
        None
    }
}
