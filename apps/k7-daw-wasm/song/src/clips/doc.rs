// clips/doc.rs — the ClipDoc family of engine-protocol/project.ts: what a placement becomes, serialized as that file spells it.
// A reader would get wrong that project.ts is not written by hand: ts-rs generates it from these types (decision 301), so a doc comment here is the page's.

use crate::files::{CompPiece, Crossfade, Take, WindowEdge};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "lowercase"))]
pub enum ClipKind {
    Audio,
    Midi,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct NoteDoc {
    pub time: f64,
    pub duration: f64,
    pub key: u8,
    pub vel: f64,
    pub channel: u8,
    pub rel: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub repeat: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub skip: Option<u64>,
}

/// A stretch map: `[timeline beat, source second]` pairs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct WarpDoc {
    pub algorithm: String,
    pub markers: Vec<(f64, f64)>,
}

/// A placement's own words as authored, unmerged.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "camelCase"))]
pub struct EventDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number | string"))]
    pub from: Option<WindowEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number | string"))]
    pub to: Option<WindowEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<String>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(rename = "loop", type = "number"))]
    pub loop_: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "{ ref: string; start: string }[]"))]
    pub takes: Option<Vec<Take>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "{ take: string; from: string; to: string }[]"))]
    pub comp: Option<Vec<CompPiece>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "{ at: string; length: string }[]"))]
    pub crossfades: Option<Vec<Crossfade>>,
}

impl EventDoc {
    /// Whether the event says nothing, so a clip carries no `event` at all.
    pub fn is_empty(&self) -> bool {
        *self == EventDoc::default()
    }

    /// What the event's own words add to a clip's weight.
    pub fn weight(&self) -> usize {
        let n = |v: Option<usize>| v.unwrap_or(0);
        n(self.takes.as_ref().map(Vec::len))
            + n(self.comp.as_ref().map(Vec::len))
            + n(self.crossfades.as_ref().map(Vec::len))
    }
}

/// A piece of a comped event: the event's own clip id and the take this piece plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct CompMark {
    pub of: String,
    pub take: String,
}

/// Which of a clip's two fades is half of a crossfade; only ever true.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct CrossfadeMark {
    #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(rename = "in"))]
    pub in_: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<bool>,
}

/// The timeline event a clip belongs to, the outermost one when timelines nest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct NestDoc {
    pub of: String,
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub start: f64,
    pub length: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<EventDoc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "camelCase"))]
pub struct ClipDoc {
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: ClipKind,
    pub name: String,
    pub source: String,
    pub start: f64,
    pub length: f64,
    pub offset: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_from: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_to: Option<f64>,
    pub fade_in: f64,
    pub fade_out: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
    pub loop_start: f64,
    pub loop_end: f64,
    pub notes: Vec<NoteDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<WarpDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp_off: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<EventDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comp: Option<CompMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossfade: Option<CrossfadeMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nest: Option<NestDoc>,
}

impl ClipDoc {
    /// What copying this clip costs: itself, its notes, its warp markers and the event's words.
    pub fn weight(&self) -> usize {
        1 + self.notes.len()
            + self.warp.as_ref().map_or(0, |w| w.markers.len())
            + self.event.as_ref().map_or(0, EventDoc::weight)
    }

    /// Whether every number the engine would read is finite; JSON writes the others as `null`.
    pub fn finite(&self) -> bool {
        let own = [
            self.start,
            self.length,
            self.offset,
            self.fade_in,
            self.fade_out,
            self.loop_start,
            self.loop_end,
        ];
        let opt = [self.source_from, self.source_to, self.gain_db];
        own.iter().chain(opt.iter().flatten()).all(|v| v.is_finite())
            && self
                .notes
                .iter()
                .all(|n| [n.time, n.duration, n.vel, n.rel].iter().all(|v| v.is_finite()))
            && self
                .warp
                .as_ref()
                .is_none_or(|w| w.markers.iter().all(|(b, s)| b.is_finite() && s.is_finite()))
            && self
                .nest
                .as_ref()
                .is_none_or(|n| n.start.is_finite() && n.length.is_finite())
            && self.event.as_ref().is_none_or(event_finite)
    }
}

fn event_finite(e: &EventDoc) -> bool {
    let edge = |w: &Option<WindowEdge>| w.as_ref().and_then(WindowEdge::seconds).is_none_or(f64::is_finite);
    edge(&e.from) && edge(&e.to) && e.gain.is_none_or(f64::is_finite)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct TimelineLaneDoc {
    pub id: String,
    pub name: String,
    pub color: String,
    pub disabled: bool,
    pub clips: Vec<ClipDoc>,
}

/// A timeline file compiled for the view that edits it; the engine plays only the tracks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct TimelineDoc {
    pub id: String,
    pub length: f64,
    pub lanes: Vec<TimelineLaneDoc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "lowercase"))]
pub enum Plays {
    Notes,
    Samples,
    Both,
}

/// The plugin heading a pattern lane's half, every key `enginePattern.ts` writes; 2h turns it into a `PluginDoc`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatternPlugin {
    pub id: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub name: String,
    pub path: String,
    pub preset_save: String,
    pub role: String,
    pub enabled: bool,
    pub params: IndexMap<String, f64>,
    pub pattern_source: String,
    pub pattern_label: String,
    pub pattern_catch_all: bool,
    pub pattern_claimed: Vec<String>,
    pub pattern_plays: Plays,
    pub pattern_only: bool,
    pub pattern_banks: Vec<String>,
}
