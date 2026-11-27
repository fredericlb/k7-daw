// graph.rs — the page's `WebGraph` as the build reads it: the lanes, the chains and their steps, the curves, the clips, the boxes.
// A reader would get wrong that a step's fields the build does not read ride in `rest`: they are copied onto the card by name (node.rs).

use crate::node::{AssemblyParam, Port, Signal};
use crate::panel::Panel;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Out {
    #[serde(default)]
    pub to: String,
    #[serde(default = "one")]
    pub gain: f64,
    #[serde(default, rename = "fromPort")]
    pub from_port: Option<String>,
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Automation {
    pub param: String,
    pub curve: String,
    #[serde(default)]
    pub trigger: Option<String>,
}

/// One module of a chain: what the build reads of it, and the rest to carry onto its card.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Step {
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub emits: Option<Signal>,
    #[serde(default)]
    pub automations: Option<Vec<Automation>>,
    #[serde(default)]
    pub bases: Option<Vec<String>>,
    #[serde(default)]
    pub shaders: Option<Vec<String>>,
    #[serde(default)]
    pub ui: Option<Panel>,
    #[serde(default)]
    pub camera: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub monitor: Option<String>,
    #[serde(default)]
    pub strip: Option<bool>,
    #[serde(default)]
    pub inputs: Option<Vec<Port>>,
    #[serde(default)]
    pub outputs: Option<Vec<Port>>,
    #[serde(default)]
    pub events: Option<Vec<String>>,
    #[serde(default)]
    pub params: Option<Map<String, Value>>,
    #[serde(default)]
    pub missing: Option<bool>,
    #[serde(default)]
    pub stub: Option<bool>,
    #[serde(default, rename = "notSupported")]
    pub not_supported: Option<bool>,
    #[serde(default, rename = "unknownPlugin")]
    pub unknown_plugin: Option<bool>,
    #[serde(default, rename = "hasNativePluginUI")]
    pub has_native_plugin_ui: Option<bool>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Chain {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default, rename = "fromPort")]
    pub from_port: Option<String>,
    #[serde(default, rename = "use")]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub out: Vec<Out>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Curve {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub spec: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrackClip {
    #[serde(rename = "ref")]
    pub name: String,
    pub start: f64,
    #[serde(default)]
    pub tc: String,
    #[serde(default)]
    pub len: Option<f64>,
    #[serde(default)]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DocNote {
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TapFeed {
    pub node: String,
    pub tap: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Plug {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Option<Signal>,
    #[serde(default)]
    pub node: String,
    #[serde(default)]
    pub port: Option<String>,
    #[serde(default)]
    pub mandatory: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Assembly {
    #[serde(default)]
    pub uses: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub inputs: Option<Vec<Plug>>,
    #[serde(default)]
    pub outputs: Option<Vec<Plug>>,
    #[serde(default)]
    pub params: Option<IndexMap<String, AssemblyParam>>,
    #[serde(default)]
    pub missing: Option<bool>,
}

/// The part of the page's graph the build reads.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Graph {
    #[serde(default)]
    pub chains: IndexMap<String, Chain>,
    #[serde(default)]
    pub tracks: Vec<String>,
    #[serde(default, rename = "videoTracks")]
    pub video_tracks: Option<Vec<String>>,
    #[serde(default, rename = "trackSignals")]
    pub track_signals: Option<IndexMap<String, Signal>>,
    #[serde(default, rename = "trackColors")]
    pub track_colors: Option<IndexMap<String, String>>,
    #[serde(default, rename = "trackMixers")]
    pub track_mixers: Option<IndexMap<String, String>>,
    #[serde(default, rename = "trackClips")]
    pub track_clips: Option<IndexMap<String, Vec<TrackClip>>>,
    #[serde(default, rename = "trackAlternatives")]
    pub track_alternatives: Option<IndexMap<String, Map<String, Value>>>,
    #[serde(default, rename = "tapFeeds")]
    pub tap_feeds: Option<Vec<TapFeed>>,
    #[serde(default)]
    pub frozen: Option<IndexMap<String, String>>,
    #[serde(default, rename = "autoFrozen")]
    pub auto_frozen: Option<IndexMap<String, String>>,
    #[serde(default)]
    pub modules: Option<IndexMap<String, DocNote>>,
    #[serde(default)]
    pub curves: Option<IndexMap<String, Curve>>,
    #[serde(default)]
    pub assemblies: Option<IndexMap<String, Assembly>>,
    #[serde(default)]
    pub stacks: Option<Vec<Vec<String>>>,
}

/// The scale every mini timeline shares: the end of the last clip in the graph.
pub fn clip_span_of(g: &Graph) -> f64 {
    let mut span = 0.0_f64;
    for clips in g.track_clips.iter().flat_map(IndexMap::values) {
        for c in clips {
            span = span.max(c.start + c.len.unwrap_or(0.0));
        }
    }
    span
}
