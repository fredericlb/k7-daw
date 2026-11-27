// node.rs — the shapes the chains graph is made of: a node, a cable, a jack, a point, and what a node carries from its step.
// A reader would get wrong that a step's other fields ride in `carried`: only the ones a rule reads are typed, the rest come back untouched.

use crate::panel::Panel;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What a wire carries; `event` stays out of the signal path and is typed only to colour a socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WireSignal"))]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Audio,
    Midi,
    Video,
    Event,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "Pt"))]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

/// One declared port of a module: its name, what it carries, and whether a cable must reach it.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct Port {
    pub name: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: Signal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandatory: Option<bool>,
}

/// One socket on a node's edge. `name` is absent for the single port every module has.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct Jack {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: Signal,
    /// Nothing is wired to it. Drawn hollow: an invitation, not a connection.
    pub free: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Jacks {
    #[serde(rename = "in")]
    #[cfg_attr(feature = "ts", ts(rename = "in"))]
    pub inputs: Vec<Jack>,
    #[serde(rename = "out")]
    #[cfg_attr(feature = "ts", ts(rename = "out"))]
    pub outputs: Vec<Jack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Track,
    Module,
    Chain,
    Sink,
    Curve,
    Assembly,
}

/// What a node's curve is bound to, carried rather than parsed back out of the id.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct CurveOf {
    pub curve: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

/// A promoted param of an assembly: the member and param it stands for.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct AssemblyParam {
    pub node: String,
    pub param: String,
    #[serde(default, rename = "pluginId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(rename = "pluginId"))]
    pub plugin_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct PlugAt {
    pub node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
}

/// What a box is, on the box's card: the template, whether it is open, its members and its plugs.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct AssemblyOf {
    pub uses: String,
    pub open: bool,
    pub members: Vec<String>,
    #[cfg_attr(feature = "ts", ts(as = "std::collections::HashMap<String, PlugAt>"))]
    pub plugs: IndexMap<String, PlugAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<std::collections::HashMap<String, AssemblyParam>>"))]
    pub params: Option<IndexMap<String, AssemblyParam>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<bool>,
}

/// One clip of a lane's mini timeline: the clip as the song has it, and where it is drawn.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct NodeClip {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub name: String,
    pub start: f64,
    pub tc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub len: Option<f64>,
    #[cfg_attr(feature = "ts", ts(type = "\"audio\" | \"midi\" | \"timeline\" | \"missing\""))]
    pub kind: String,
    /// Where it sits on the mini timeline, 0..1, against the scale every lane shares.
    pub at: f64,
    /// How wide it is on that timeline, 0..1 — floored, so it is never 0.
    pub w: f64,
}

/// How far a curve moves the param it drives.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct Depth {
    pub amplitude: f64,
    pub bipolar: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").Cut"))]
    pub cut: Option<String>,
}

/// An automation edge's payload: a curve feeding a module's param.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct EdgeAutomation {
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Depth>,
}

/// What a node carries from its step by name, untouched: fields the build reads nothing of.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct Carried {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"category\"]")
    )]
    pub category: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"assets\"]")
    )]
    pub assets: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"assetChoices\"]")
    )]
    pub asset_choices: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"cc\"]"))]
    pub cc: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"defaults\"]")
    )]
    pub defaults: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"pluginId\"]")
    )]
    pub plugin_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"channel\"]")
    )]
    pub channel: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"tap\"]"))]
    pub tap: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"alternatives\"]")
    )]
    pub alternatives: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"alternative\"]")
    )]
    pub alternative: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"engines\"]")
    )]
    pub engines: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"engine\"]")
    )]
    pub engine: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"rack\"]"))]
    pub rack: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"rackKit\"]")
    )]
    pub rack_kit: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"base\"]"))]
    pub base: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"shader\"]")
    )]
    pub shader: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"uses\"]"))]
    pub uses: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"place\"]"))]
    pub place: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebChainStep[\"stage\"]"))]
    pub stage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"modelDownload\"]")
    )]
    pub model_download: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "import(\"@studio/graph-model\").WebChainStep[\"ampModel\"]")
    )]
    pub amp_model: Option<Value>,
}

/// A node as the renderer receives it: its step's fields, the flags, and the geometry.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename = "RawNode"))]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub kind: Kind,
    #[serde(default)]
    pub dim: bool,
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub h: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<f64>,
    #[serde(default, rename = "tabW", skip_serializing_if = "Option::is_none")]
    pub tab_w: Option<Vec<f64>>,
    #[serde(default, rename = "tabH", skip_serializing_if = "Option::is_none")]
    pub tab_h: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, rename = "trackFace", skip_serializing_if = "Option::is_none")]
    pub track_face: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypassed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "\"at\" | \"inside\""))]
    pub frozen: Option<String>,
    #[serde(default, rename = "autoFrozen", skip_serializing_if = "Option::is_none")]
    pub auto_frozen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bare: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jacks: Option<Jacks>,
    #[serde(default, rename = "eventIn", skip_serializing_if = "Option::is_none")]
    pub event_in: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<f64>,
    #[serde(default, rename = "curveOf", skip_serializing_if = "Option::is_none")]
    pub curve_of: Option<CurveOf>,
    #[serde(default, rename = "curveSpec", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").CurveFile"))]
    pub curve_spec: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "\"across\""))]
    pub flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed: Option<Pt>,
    #[serde(default, rename = "assemblyOf", skip_serializing_if = "Option::is_none")]
    pub assembly_of: Option<AssemblyOf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clips: Option<Vec<NodeClip>>,
    #[serde(default, rename = "clipsMore", skip_serializing_if = "Option::is_none")]
    pub clips_more: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<Port>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<Port>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "\"off\" | \"auto\" | \"on\""))]
    pub monitor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").ParamValue>")
    )]
    pub params: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").PageModuleUI"))]
    pub ui: Option<Panel>,
    #[serde(default)]
    pub missing: bool,
    #[serde(default)]
    pub stub: bool,
    #[serde(default, rename = "notSupported")]
    pub not_supported: bool,
    #[serde(default, rename = "unknownPlugin")]
    pub unknown_plugin: bool,
    #[serde(default, rename = "hasNativePluginUI")]
    pub has_native_plugin_ui: bool,
    /// The step's other fields, carried verbatim: its category, assets, cc, alternatives, engines, base, shader, rack, tap...
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub carried: Carried,
}

impl Node {
    /// A card of `kind` with every flag false and the plain size, which is what each built-in starts from.
    pub fn plain(id: &str, label: &str, kind: Kind) -> Node {
        Node {
            id: id.to_owned(),
            label: label.to_owned(),
            kind,
            dim: false,
            w: crate::geom::NODE_W,
            h: crate::geom::NODE_H,
            x: None,
            y: None,
            rank: None,
            tab_w: None,
            tab_h: None,
            sub: None,
            video: None,
            color: None,
            track_face: None,
            chain: None,
            chains: None,
            module: None,
            bypassed: None,
            frozen: None,
            auto_frozen: None,
            pending: None,
            bar: None,
            bare: None,
            jacks: None,
            event_in: None,
            aspect: None,
            curve_of: None,
            curve_spec: None,
            flow: None,
            assembly: None,
            pushed: None,
            assembly_of: None,
            note: None,
            clips: None,
            clips_more: None,
            inputs: None,
            outputs: None,
            events: None,
            monitor: None,
            camera: None,
            device: None,
            strip: None,
            params: None,
            ui: None,
            missing: false,
            stub: false,
            not_supported: false,
            unknown_plugin: false,
            has_native_plugin_ui: false,
            carried: Carried::default(),
        }
    }
}

/// A cable. `points` and `side` are the laid-out graph's; the build leaves them absent.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename = "RawEdge"))]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation: Option<EdgeAutomation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<Signal>,
    #[serde(default, rename = "targetPort", skip_serializing_if = "Option::is_none")]
    pub target_port: Option<String>,
    #[serde(default, rename = "sourcePort", skip_serializing_if = "Option::is_none")]
    pub source_port: Option<String>,
    #[serde(default, rename = "targetSlot", skip_serializing_if = "Option::is_none")]
    pub target_slot: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glued: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<Pt>>,
}

impl Edge {
    pub fn new(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: id.to_owned(),
            source: source.to_owned(),
            target: target.to_owned(),
            ..Edge::default()
        }
    }
}

/// A graph laid out by a Rust arrangement: its cards, its cables and the extent of the drawing.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Laid {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub top: f64,
}

/// The arrow's insert: the two ends a module dropped on it would splice between.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct EdgeInsert {
    pub before: String,
    pub after: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<bool>,
}
