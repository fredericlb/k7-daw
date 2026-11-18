// graph/view/web.rs — the WebGraph of `@studio/graph-model` as Rust types: what the Chains tab reads, field for field.
// A reader would get wrong that the frontend reads PRESENCE: `stub` and `hasNativePluginUI` are always there, `missing` is true or absent (437).

use crate::graph::GraphIssue;
use crate::modules::{ParamValue, PortType};
use crate::resolve::ResolvedUi as Ui;
use crate::routing::{Position, TapFeed};
use indexmap::IndexMap;
use serde::Serialize;

macro_rules! web {
    ($($(#[$m:meta])* pub struct $name:ident { $($body:tt)* })+) => {$(
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq, Serialize)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
        pub struct $name { $($body)* }
    )+};
}

pub type Params = IndexMap<String, ParamValue>;

web! {
    /// One destination of a run: its target, the share of the signal it takes and the output it left from.
    pub struct WebOut {
        pub to: String,
        pub gain: f64,
        #[serde(rename = "fromPort", skip_serializing_if = "Option::is_none")]
        pub from_port: Option<String>,
    }

    pub struct WebPortIn {
        pub name: String,
        #[serde(rename = "type")]
        #[cfg_attr(feature = "ts", ts(rename = "type"))]
        pub kind: PortType,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub mandatory: Option<bool>,
    }

    pub struct WebPortOut {
        pub name: String,
        #[serde(rename = "type")]
        #[cfg_attr(feature = "ts", ts(rename = "type"))]
        pub kind: PortType,
    }

    pub struct WebModelDownload {
        pub file: String,
        pub url: String,
    }

    pub struct WebAmpModel {
        #[cfg_attr(feature = "ts", ts(type = "\"nam\" | \"aida-x\" | \"proteus\""))]
        pub kind: String,
        pub knobs: u32,
    }

    /// One sound of a rack, flattened: a pad draws one kind of sound.
    pub struct WebRackSound {
        pub symbol: String,
        pub note: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sample: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub clip: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub from: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub to: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub mute: Option<bool>,
    }

    pub struct WebAutomation {
        pub param: String,
        pub curve: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub trigger: Option<String>,
    }

    pub struct WebTap {
        pub id: String,
        pub side: String,
    }

    /// One alternative's name list and the one that is on, for a lane.
    pub struct WebAltChoice {
        pub alternatives: Vec<String>,
        pub alternative: String,
    }

    pub struct WebDocNote {
        pub content: String,
    }

    pub struct WebTrackClip {
        #[serde(rename = "ref")]
        #[cfg_attr(feature = "ts", ts(rename = "ref"))]
        pub ref_: String,
        pub start: f64,
        pub tc: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub len: Option<f64>,
        #[cfg_attr(feature = "ts", ts(type = "\"audio\" | \"midi\" | \"timeline\" | \"missing\""))]
        pub kind: String,
    }

    /// One plug on an assembly's box: a port, plus the member node whose own port it stands for.
    pub struct WebPlug {
        pub name: String,
        #[serde(rename = "type")]
        #[cfg_attr(feature = "ts", ts(rename = "type"))]
        pub kind: PortType,
        pub node: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub port: Option<String>,
        pub mandatory: bool,
    }

    pub struct WebAssemblyParam {
        pub node: String,
        pub param: String,
        #[serde(rename = "pluginId", skip_serializing_if = "Option::is_none")]
        pub plugin_id: Option<String>,
    }

    pub struct WebNodeAssembly {
        pub uses: String,
        pub label: String,
        pub members: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub inputs: Option<Vec<WebPlug>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub outputs: Option<Vec<WebPlug>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebAssemblyParam>"))]
        pub params: Option<IndexMap<String, WebAssemblyParam>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub missing: Option<bool>,
    }

    /// One curve: its type, an oscillator's shape, a drawn curve's points, and the whole definition the editor opens on.
    pub struct WebCurve {
        #[serde(rename = "type")]
        #[cfg_attr(feature = "ts", ts(rename = "type", type = "\"lfo\" | \"custom\" | \"adsr\" | \"onoff\""))]
        pub kind: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "\"sine\" | \"triangle\" | \"saw\" | \"square\""))]
        pub shape: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "{ time: string; value: number }[]"))]
        pub points: Option<Vec<song_schema::PointSpec>>,
        #[cfg_attr(feature = "ts", ts(type = "CurveSpec"))]
        pub spec: crate::curves::CurveFile,
    }

    /// One module of a chain as the Chains tab draws it.
    pub struct WebChainStep {
        pub module: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub uses: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub place: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub stage: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "WebModuleCategory"))]
        pub category: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub emits: Option<PortType>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub enabled: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub missing: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub inputs: Option<Vec<WebPortIn>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub outputs: Option<Vec<WebPortOut>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub events: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        pub assets: Option<IndexMap<String, String>>,
        #[serde(rename = "assetChoices", skip_serializing_if = "Option::is_none")]
        pub asset_choices: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub base: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub bases: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub shader: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub shaders: Option<Vec<String>>,
        pub stub: bool,
        #[serde(rename = "notSupported", skip_serializing_if = "Option::is_none")]
        pub not_supported: Option<bool>,
        #[serde(rename = "unknownPlugin", skip_serializing_if = "Option::is_none")]
        pub unknown_plugin: Option<bool>,
        #[serde(rename = "hasNativePluginUI")]
        pub has_native_plugin_ui: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub strip: Option<bool>,
        #[serde(rename = "modelDownload", skip_serializing_if = "Option::is_none")]
        pub model_download: Option<WebModelDownload>,
        #[serde(rename = "ampModel", skip_serializing_if = "Option::is_none")]
        pub amp_model: Option<WebAmpModel>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "WebModuleUI"))]
        pub ui: Option<Ui>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebCc>"))]
        pub cc: Option<IndexMap<String, crate::modules::panels::CcBinding>>,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, number | string>"))]
        pub params: Params,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, number>"))]
        pub defaults: Option<IndexMap<String, f64>>,
        #[serde(rename = "pluginId", skip_serializing_if = "Option::is_none")]
        pub plugin_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub alternatives: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub alternative: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub engines: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub engine: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "\"off\" | \"auto\" | \"on\""))]
        pub monitor: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub channel: Option<Vec<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub camera: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub device: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "{ id: string; side: \"input\" | \"output\" }"))]
        pub tap: Option<WebTap>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub automations: Option<Vec<WebAutomation>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub rack: Option<Vec<WebRackSound>>,
        #[serde(rename = "rackKit", skip_serializing_if = "Option::is_none")]
        pub rack_kit: Option<bool>,
    }

    pub struct WebChain {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub from: Option<String>,
        #[serde(rename = "fromPort", skip_serializing_if = "Option::is_none")]
        pub from_port: Option<String>,
        #[serde(rename = "use")]
        #[cfg_attr(feature = "ts", ts(rename = "use"))]
        pub use_steps: Vec<WebChainStep>,
        pub out: Vec<WebOut>,
    }

    pub struct WebGraph {
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebChain>"))]
        pub chains: IndexMap<String, WebChain>,
        pub tracks: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, { x: number; y: number }>"))]
        pub positions: Option<IndexMap<String, Position>>,
        #[serde(rename = "videoTracks")]
        pub video_tracks: Vec<String>,
        #[serde(rename = "trackSignals")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebSignal>"))]
        pub track_signals: IndexMap<String, PortType>,
        #[serde(rename = "trackColors")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        pub track_colors: IndexMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub stacks: Option<Vec<Vec<String>>>,
        #[serde(rename = "trackMixers", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        pub track_mixers: Option<IndexMap<String, String>>,
        #[serde(rename = "trackClips", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebTrackClip[]>"))]
        pub track_clips: Option<IndexMap<String, Vec<WebTrackClip>>>,
        #[serde(rename = "trackAlternatives", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebAltChoice>"))]
        pub track_alternatives: Option<IndexMap<String, WebAltChoice>>,
        #[serde(rename = "tapFeeds", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "{ node: string; tap: string; to: string }[]"))]
        pub tap_feeds: Option<Vec<TapFeed>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, \"at\" | \"inside\">"))]
        pub frozen: Option<IndexMap<String, String>>,
        #[serde(rename = "autoFrozen", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        pub auto_frozen: Option<IndexMap<String, String>>,
        #[serde(rename = "autoFreezeWaiting", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        pub auto_freeze_waiting: Option<IndexMap<String, String>>,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebDocNote>"))]
        pub modules: IndexMap<String, WebDocNote>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebCurve>"))]
        pub curves: Option<IndexMap<String, WebCurve>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, WebNodeAssembly>"))]
        pub assemblies: Option<IndexMap<String, WebNodeAssembly>>,
        pub warnings: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub issues: Option<Vec<GraphIssue>>,
    }
}

/// A step the budget has not paid for: its module, and the flags the frontend reads by presence.
pub fn bare_step(module: &str, enabled: Option<bool>) -> WebChainStep {
    WebChainStep {
        enabled,
        ..blank(module)
    }
}

fn blank(module: &str) -> WebChainStep {
    WebChainStep {
        module: module.to_owned(),
        uses: None,
        place: None,
        stage: None,
        category: None,
        emits: None,
        enabled: None,
        missing: None,
        inputs: None,
        outputs: None,
        events: None,
        assets: None,
        asset_choices: None,
        base: None,
        bases: None,
        shader: None,
        shaders: None,
        stub: false,
        not_supported: None,
        unknown_plugin: None,
        has_native_plugin_ui: false,
        strip: None,
        model_download: None,
        amp_model: None,
        ui: None,
        cc: None,
        params: Params::new(),
        defaults: None,
        plugin_id: None,
        alternatives: None,
        alternative: None,
        engines: None,
        engine: None,
        monitor: None,
        channel: None,
        camera: None,
        device: None,
        tap: None,
        automations: None,
        rack: None,
        rack_kit: None,
    }
}

impl WebChainStep {
    pub fn new(module: &str) -> WebChainStep {
        blank(module)
    }
}
