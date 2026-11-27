// views/web/project.rs — a compiled ProjectDoc as the browser's WebProject: the tracks, the groups, the markers, each timeline's lanes.
// A reader would get wrong that a track's `disabled` is never set here (no document field says it) and `mixer` is absent, not at unity.

use super::clip::{WebClip, web_clip};
use super::fold::fold_nests;
use crate::clips::{ClipDoc, TimelineLaneDoc};
use crate::doc::{PluginDoc, ProjectDoc, TrackDoc};
use crate::files::Marker;
use crate::resolve::MIXER_PLUGIN;
use crate::source::SourceError;
use crate::views::lengths::Measure;
use serde::Serialize;

/// The strip a track's surfaces read: level and pan only.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebMixer {
    pub module: String,
    pub volume: f64,
    pub pan: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct WebPlugin {
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub plugin_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sfz_file: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct WebTrack {
    pub id: String,
    pub name: String,
    pub group: String,
    pub generated: bool,
    pub content_type: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mixer: Option<WebMixer>,
    pub mute: bool,
    pub solo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub highlight: Option<bool>,
    /// Ignored by the brain; never written here, and read by the page where a client marks a lane so.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub disabled: Option<bool>,
    pub clips: Vec<WebClip>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub frozen_material: Option<Vec<WebClip>>,
    pub plugins: Vec<WebPlugin>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebGroup {
    pub id: String,
    pub name: String,
    pub color: String,
    pub mute: bool,
    pub solo: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebMarker {
    pub beat: f64,
    pub name: String,
    pub tc: String,
}

/// A timeline of `timelines/`: its fixed length in beats and its lanes drawn as a song's are.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebTimeline {
    pub id: String,
    pub length: f64,
    pub lanes: Vec<WebTrack>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct WebProject {
    pub name: String,
    pub tempo: f64,
    pub time_sig_numerator: u32,
    pub time_sig_denominator: u32,
    pub master_volume: f64,
    pub tracks: Vec<WebTrack>,
    pub groups: Vec<WebGroup>,
    pub metronome: bool,
    pub metronome_level: f64,
    pub count_in: u32,
    pub markers: Vec<WebMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub timelines: Option<Vec<WebTimeline>>,
}

/// What `toWebTracks` reads of a track, borrowed: a timeline's lane is one too, without `timelineLaneTrack`'s copy of its clips.
struct TrackView<'a> {
    id: &'a str,
    name: &'a str,
    group: &'a str,
    generated: bool,
    content_type: &'a str,
    color: &'a str,
    mute: bool,
    solo: bool,
    highlight: bool,
    clips: &'a [ClipDoc],
    frozen_material: Option<&'a [ClipDoc]>,
    plugins: &'a [PluginDoc],
}

impl<'a> TrackView<'a> {
    fn of_track(t: &'a TrackDoc) -> TrackView<'a> {
        TrackView {
            id: &t.id,
            name: &t.name,
            group: &t.group,
            generated: t.generated,
            content_type: &t.content_type,
            color: &t.color,
            mute: t.mute,
            solo: t.solo,
            highlight: t.highlight == Some(true),
            clips: &t.clips,
            frozen_material: t.frozen_material.as_deref(),
            plugins: &t.plugins,
        }
    }

    /// A timeline's lane as a track: either kind of clip, no strip, no group.
    fn of_lane(l: &'a TimelineLaneDoc) -> TrackView<'a> {
        TrackView {
            id: &l.id,
            name: &l.name,
            group: "",
            generated: false,
            content_type: "audio notes",
            color: &l.color,
            mute: false,
            solo: false,
            highlight: false,
            clips: &l.clips,
            frozen_material: None,
            plugins: &[],
        }
    }
}

/// The LAST mixer in a track's chain, as its strip.
fn mixer_of(plugins: &[PluginDoc]) -> Option<WebMixer> {
    let p = plugins.iter().rev().find(|p| p.plugin_type == MIXER_PLUGIN)?;
    let v = |name: &str| p.params.get(name).copied().unwrap_or(0.0);
    Some(WebMixer {
        module: p.name.clone(),
        volume: v("Volume"),
        pan: v("Pan"),
    })
}

fn web_plugin(p: &PluginDoc) -> WebPlugin {
    WebPlugin {
        id: p.id.clone(),
        plugin_type: p.plugin_type.clone(),
        name: p.name.clone(),
        sfz_file: p.sfz_file.clone().filter(|s| !s.is_empty()),
    }
}

/// One track as the timeline draws it; a comped event and a timeline event are each ONE clip, folded.
fn web_track(t: &TrackView<'_>, tempo: f64, m: &mut dyn Measure) -> Result<WebTrack, SourceError> {
    let mut one = |c: &ClipDoc| web_clip(c, Some(tempo), m);
    let clips = fold_nests(t.clips, &mut one)?;
    let frozen_material = match t.frozen_material {
        Some(f) if !f.is_empty() => Some(f.iter().map(&mut one).collect::<Result<Vec<_>, _>>()?),
        _ => None,
    };
    Ok(WebTrack {
        id: t.id.to_owned(),
        name: t.name.to_owned(),
        group: t.group.to_owned(),
        generated: t.generated,
        content_type: t.content_type.to_owned(),
        color: t.color.to_owned(),
        mixer: mixer_of(t.plugins),
        mute: t.mute,
        solo: t.solo,
        highlight: t.highlight.then_some(true),
        disabled: None,
        clips,
        frozen_material,
        plugins: t.plugins.iter().map(web_plugin).collect(),
    })
}

fn web_marker(m: &Marker) -> WebMarker {
    WebMarker {
        beat: m.beat,
        name: m.name.clone(),
        tc: m.tc.clone(),
    }
}

/// A compiled document as the browser's `WebProject`, field for field; `m` says how long each audio source is.
pub fn web_project_of(doc: &ProjectDoc, m: &mut dyn Measure) -> Result<WebProject, SourceError> {
    let tracks = doc
        .tracks
        .iter()
        .map(|t| web_track(&TrackView::of_track(t), doc.tempo, m))
        .collect::<Result<Vec<_>, _>>()?;
    let timelines = match doc.timelines.as_deref() {
        Some(list) if !list.is_empty() => {
            let mut out = Vec::with_capacity(list.len());
            for t in list {
                let mut lanes = Vec::with_capacity(t.lanes.len());
                for l in &t.lanes {
                    lanes.push(web_track(&TrackView::of_lane(l), doc.tempo, m)?);
                }
                out.push(WebTimeline {
                    id: t.id.clone(),
                    length: t.length,
                    lanes,
                });
            }
            Some(out)
        }
        _ => None,
    };
    Ok(WebProject {
        name: doc.name.clone(),
        tempo: doc.tempo,
        time_sig_numerator: doc.time_sig_numerator,
        time_sig_denominator: doc.time_sig_denominator,
        master_volume: doc.master_volume,
        tracks,
        groups: doc
            .groups
            .iter()
            .map(|g| WebGroup {
                id: g.id.clone(),
                name: g.name.clone(),
                color: g.color.clone(),
                mute: g.mute,
                solo: g.solo,
            })
            .collect(),
        metronome: doc.settings.metronome,
        metronome_level: doc.settings.metronome_level,
        count_in: doc.settings.count_in,
        markers: doc.markers.iter().map(web_marker).collect(),
        timelines,
    })
}
