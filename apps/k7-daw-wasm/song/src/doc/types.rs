// doc/types.rs — the ProjectDoc of engine-protocol/project.ts, field for field: the document the engine plays.
// A reader would get wrong that project.ts is not written by hand: ts-rs generates it from these types (decision 301), so a doc comment here is the page's.

use crate::clips::{ClipDoc, Plays, TimelineDoc};
use crate::files::{Marker, SettingsDoc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// One rack sound: a note playing a whole clip (samples) or a window of the shared source (slice).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct RackSoundDoc {
    pub symbol: String,
    pub note: u8,
    pub clip: String,
    pub start: f64,
    pub length: f64,
}

/// A tap that leaves by an output pair of its own, pair 0 being the main one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct TapOutDoc {
    pub note: u8,
    pub pair: u32,
}

/// The output pair of a multi-output instrument that feeds a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct InputFromDoc {
    pub plugin: String,
    pub pair: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "camelCase"))]
pub struct PluginDoc {
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub plugin_type: String,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nam_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surge_patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfz_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsp_graph: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_catch_all: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_claimed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_plays: Option<Plays>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_banks: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidechain_from: Option<String>,
    pub preset_save: String,
    pub role: String,
    pub enabled: bool,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number>"))]
    pub params: IndexMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rack: Option<Vec<RackSoundDoc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tap_outs: Option<Vec<TapOutDoc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SendDoc {
    pub to: String,
    pub level: f64,
    pub enabled: bool,
}

/// A baked automation lane: `points` are `[beat, output]` in the curve's range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "camelCase"))]
pub struct AutomationDoc {
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bipolar: Option<bool>,
    pub points: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "camelCase"))]
pub struct TrackDoc {
    pub id: String,
    pub name: String,
    pub role: String,
    pub content_type: String,
    pub group: String,
    pub generated: bool,
    pub color: String,
    pub mute: bool,
    pub solo: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    pub disabled: bool,
    pub input_channels: Vec<u32>,
    pub output_channels: Vec<u32>,
    pub monitor: String,
    pub input_gain: f64,
    pub clips: Vec<ClipDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_material: Option<Vec<ClipDoc>>,
    pub plugins: Vec<PluginDoc>,
    pub sends: Vec<SendDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_from: Option<InputFromDoc>,
    pub automation: Vec<AutomationDoc>,
}

/// A group track: it holds no material, surfaces draw it over its members.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct GroupDoc {
    pub id: String,
    pub name: String,
    pub color: String,
    pub mute: bool,
    pub solo: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename_all = "camelCase"))]
pub struct ProjectDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub name: String,
    pub tempo: f64,
    pub sample_rate: u32,
    pub time_sig_numerator: u32,
    pub time_sig_denominator: u32,
    pub master_volume: f64,
    pub master_plugins: Vec<PluginDoc>,
    pub record_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_mute: Option<bool>,
    pub tracks: Vec<TrackDoc>,
    pub groups: Vec<GroupDoc>,
    pub markers: Vec<Marker>,
    pub settings: SettingsDoc,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timelines: Option<Vec<TimelineDoc>>,
}

/// What a string costs to copy, in the budgets' unit: one per 16 bytes.
pub fn text_weight(s: &str) -> usize {
    s.len() / 16
}

impl PluginDoc {
    /// A plugin with `enabled` and every other field empty, as `build.ts`' `pluginDoc` makes one.
    pub fn new(id: &str) -> PluginDoc {
        PluginDoc {
            id: id.to_owned(),
            plugin_type: String::new(),
            name: String::new(),
            path: String::new(),
            ir_file: None,
            nam_file: None,
            surge_patch: None,
            sfz_file: None,
            deck_file: None,
            dsp_graph: None,
            pattern_source: None,
            pattern_label: None,
            pattern_catch_all: None,
            pattern_claimed: None,
            pattern_plays: None,
            pattern_only: None,
            pattern_banks: None,
            sidechain_from: None,
            preset_save: String::new(),
            role: "audioFX".to_owned(),
            enabled: true,
            params: IndexMap::new(),
            rack: None,
            tap_outs: None,
        }
    }

    /// What copying this plugin costs (decision 323): itself, each list entry, and every string at `len / 16`.
    pub fn weight(&self) -> usize {
        let opt = |s: &Option<String>| s.as_deref().map_or(0, text_weight);
        let list = |v: &Option<Vec<String>>| v.iter().flatten().map(|s| 1 + text_weight(s)).sum::<usize>();
        let strings = [
            &self.id,
            &self.plugin_type,
            &self.name,
            &self.path,
            &self.preset_save,
            &self.role,
        ]
        .iter()
        .map(|s| text_weight(s))
        .sum::<usize>();
        let files = [
            &self.ir_file,
            &self.nam_file,
            &self.surge_patch,
            &self.sfz_file,
            &self.deck_file,
            &self.dsp_graph,
            &self.pattern_source,
            &self.pattern_label,
            &self.sidechain_from,
        ]
        .into_iter()
        .map(opt)
        .sum::<usize>();
        let params = self.params.keys().map(|k| 1 + text_weight(k)).sum::<usize>();
        let rack = self
            .rack
            .iter()
            .flatten()
            .map(|r| 1 + text_weight(&r.symbol) + text_weight(&r.clip))
            .sum::<usize>();
        let taps = self.tap_outs.as_ref().map_or(0, Vec::len);
        1 + strings + files + params + rack + taps + list(&self.pattern_claimed) + list(&self.pattern_banks)
    }
}

impl TrackDoc {
    /// A regular audio track named by its id, with nothing in it.
    pub fn new(id: &str) -> TrackDoc {
        TrackDoc {
            id: id.to_owned(),
            name: id.to_owned(),
            role: "regular".to_owned(),
            content_type: "audio".to_owned(),
            group: String::new(),
            generated: false,
            color: String::new(),
            mute: false,
            solo: false,
            highlight: None,
            disabled: false,
            input_channels: Vec::new(),
            output_channels: Vec::new(),
            monitor: "auto".to_owned(),
            input_gain: 0.0,
            clips: Vec::new(),
            frozen_material: None,
            plugins: Vec::new(),
            sends: Vec::new(),
            input_from: None,
            automation: Vec::new(),
        }
    }

    /// What copying the track costs WITHOUT its clips and frozen material: itself, channels, plugins, sends, automation.
    pub fn weight(&self) -> usize {
        let strings = [
            &self.id,
            &self.name,
            &self.role,
            &self.content_type,
            &self.group,
            &self.color,
        ]
        .iter()
        .map(|s| text_weight(s))
        .sum::<usize>();
        let sends = self.sends.iter().map(|s| 1 + text_weight(&s.to)).sum::<usize>();
        let auto = self
            .automation
            .iter()
            .map(|a| 1 + a.points.len() + text_weight(&a.param) + a.plugin_id.as_deref().map_or(0, text_weight))
            .sum::<usize>();
        let plugins = self.plugins.iter().map(PluginDoc::weight).sum::<usize>();
        1 + strings + self.input_channels.len() + self.output_channels.len() + sends + auto + plugins
    }
}

impl ProjectDoc {
    /// A document with nothing in it and every default `compileNodeProject` starts from.
    pub fn empty() -> ProjectDoc {
        ProjectDoc {
            uuid: None,
            name: "untitled".to_owned(),
            tempo: 120.0,
            sample_rate: 48000,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
            master_volume: 0.0,
            master_plugins: Vec::new(),
            record_dir: String::new(),
            record_mute: None,
            tracks: Vec::new(),
            groups: Vec::new(),
            markers: Vec::new(),
            settings: SettingsDoc::default(),
            timelines: None,
        }
    }

    /// What copying the document costs, clips and timelines included.
    pub fn weight(&self) -> usize {
        let clips = |v: &[ClipDoc]| v.iter().map(ClipDoc::weight).sum::<usize>();
        let tracks = self
            .tracks
            .iter()
            .map(|t| t.weight() + clips(&t.clips) + t.frozen_material.as_deref().map_or(0, clips))
            .sum::<usize>();
        let timelines = self
            .timelines
            .iter()
            .flatten()
            .map(|t| 1 + t.lanes.iter().map(|l| 1 + clips(&l.clips)).sum::<usize>())
            .sum::<usize>();
        1 + tracks
            + timelines
            + self.master_plugins.iter().map(PluginDoc::weight).sum::<usize>()
            + self.groups.len()
            + self.markers.len()
    }
}
