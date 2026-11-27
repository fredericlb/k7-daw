// reads/model.rs — the slim shapes the reads take: only the fields a rule reads, each optional as the page's types are.
// A reader would get wrong that a missing number is 0 here and `undefined` there: `Option` is kept wherever JavaScript tests for it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RNote {
    pub time: f64,
    pub duration: f64,
    pub key: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vel: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RTake {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub start: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RCompSpan {
    pub take: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RCrossfade {
    pub at: String,
    pub length: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct REvent {
    pub takes: Vec<RTake>,
    pub comp: Vec<RCompSpan>,
    pub crossfades: Vec<RCrossfade>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RComp {
    pub of: String,
    pub take: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RClip {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub start: f64,
    pub length: f64,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
    pub disabled: Option<bool>,
    pub notes: Vec<RNote>,
    pub pieces: Option<Vec<RClip>>,
    pub comp: Option<RComp>,
    pub event: Option<REvent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RMixer {
    pub module: Option<String>,
    pub volume: Option<f64>,
    pub pan: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RGroup {
    pub id: String,
    pub mute: Option<bool>,
    pub solo: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RIdOnly {
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RTrack {
    pub id: String,
    pub name: String,
    pub group: String,
    pub solo: Option<bool>,
    pub highlight: Option<bool>,
    pub frozen_material: Option<Vec<RIdOnly>>,
    pub mute: Option<bool>,
    pub disabled: Option<bool>,
    pub generated: Option<bool>,
    pub mixer: Option<RMixer>,
    pub clips: Vec<RClip>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RVideoClip {
    pub start: f64,
    pub length: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RVideoTrack {
    pub id: String,
    pub mute: Option<bool>,
    pub solo: Option<bool>,
    pub clips: Vec<RVideoClip>,
    pub shapes: Vec<Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ROut {
    pub to: String,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, Value>,
}

/// One rack sound; the fields beyond the three a rule reads ride along so the answer is the page's own object.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RRack {
    pub note: f64,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RStep {
    pub module: String,
    pub device: Option<String>,
    pub camera: Option<Value>,
    pub monitor: Option<String>,
    pub channel: Option<Vec<f64>>,
    pub rack: Option<Vec<RRack>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RChain {
    pub from: Option<String>,
    pub out: Option<Vec<ROut>>,
    pub r#use: Vec<RStep>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RGraph {
    pub chains: indexmap::IndexMap<String, RChain>,
    pub tracks: Option<Vec<String>>,
}
