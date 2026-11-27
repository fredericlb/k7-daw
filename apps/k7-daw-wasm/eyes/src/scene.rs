// scene.rs — the scene the compositor draws: layers of shots and shapes in decimal seconds, GLSL passes and named transitions.
// A reader would get wrong which fields are absent and which are empty: the page tells "no fade" from a zero one by absence.

use crate::schedule::LayerSchedule;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// How a shot arrives or leaves: how long, which program draws it, and the stretch it runs through.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "SceneFade", optional_fields))]
pub struct Fade {
    pub over: f64,
    #[serde(rename = "with")]
    #[cfg_attr(feature = "ts", ts(rename = "with"))]
    pub with_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
}

/// One keyframe, `ease` shaping the segment leaving it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "SceneKey"))]
pub struct Key {
    pub t: f64,
    pub v: f64,
    pub ease: String,
}

/// One subtitle cue, in seconds of the file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "SceneCue"))]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// How a subtitle clip's cues are set; every word optional.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(rename = "SceneCaptionsStyle", optional_fields)
)]
pub struct CaptionStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "\"left\" | \"center\" | \"right\""))]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "\"bottom\" | \"top\" | \"middle\""))]
    pub position: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
}

/// A subtitle clip's cues and how they are set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "SceneCaptions"))]
pub struct Captions {
    pub cues: Vec<Cue>,
    pub style: CaptionStyle,
}

/// One video clip on a layer's timeline, in seconds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SceneClip {
    pub id: String,
    /// A URL the runtime can load; empty on an activation span.
    #[serde(default)]
    pub src: String,
    /// When it appears on the timeline.
    pub start: f64,
    /// 0 is to the end of the source.
    pub length: f64,
    /// The in-point into the source.
    #[serde(default)]
    pub source_from: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<Fade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<Fade>,
    /// How it moves in its lane's frame, each property's keys by name; absent costs nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<std::collections::HashMap<String, Vec<Key>>>"))]
    pub animate: Option<IndexMap<String, Vec<Key>>>,
    /// How fast the source runs under it, signed; absent is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    /// `src` is an IMAGE, decoded once and held while the shot is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "true"))]
    pub still: Option<bool>,
    /// The run this shot alone goes through, a key of `VideoScene.stretches`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    /// A SUBTITLE clip: the cue at its source time is drawn, as a still is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captions: Option<Captions>,
}

/// A transition's two stages, shared by every cut that names it: one program per name.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "SceneTransition"))]
pub struct Transition {
    pub name: String,
    pub vertex: String,
    pub fragment: String,
    /// uP1..uP4, each 0..1.
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number>"))]
    pub params: IndexMap<String, f64>,
}

/// A drawn primitive on a shape layer: a fill or text, carrying its own geometry; the scene holds them as JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub enum SceneShape {
    Square {
        id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: String,
    },
    Circle {
        id: String,
        x: f64,
        y: f64,
        r: f64,
        color: String,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        id: String,
        content: String,
        /// A CSS font-family.
        font: String,
        /// px.
        font_size: f64,
        x: f64,
        y: f64,
        /// The width its lines wrap in; a word longer than it is condensed to fit.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        w: Option<f64>,
        color: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "\"left\" | \"center\" | \"right\""))]
        align: Option<String>,
        /// A box behind the lines, any CSS colour.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background: Option<String>,
        /// A soft shadow under the glyphs, for text over a picture.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<bool>,
        /// A multiple of the font size; 1.2 unsaid.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line_height: Option<f64>,
        /// px between the lines and their box.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        padding: Option<f64>,
    },
}

/// One GLSL pass over a picture: plain source and plain numbers.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ScenePass {
    pub id: String,
    /// The node's name, for a compile error that says which card to look at.
    pub name: String,
    pub vertex: String,
    pub fragment: String,
    pub enabled: bool,
    /// uP1..uP4, each 0..1.
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number>"))]
    pub params: IndexMap<String, f64>,
    /// uAudio, 0..1: replaced every frame by the host.
    pub audio: f64,
    /// A `.cube` file's URL, read as `uLut`: fetched and uploaded once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lut: Option<String>,
    /// The knobs at which this pass changes nothing: there it is skipped, yet kept, so a knob set live has a pass to move.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number>", optional))]
    pub identity: Option<IndexMap<String, f64>>,
}

/// A layer in the draw stack: a VIDEO lane, a SHAPE lane, or a CAMERA, the third told by `camera` being present.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct VideoLayer {
    pub id: String,
    pub hidden: bool,
    /// A LIVE picture fills this layer: the camera's LABEL, or "" for whichever is default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub clips: Vec<SceneClip>,
    /// Which shots show when, as a table: the page looks it up per frame instead of working the rule out.
    pub schedule: LayerSchedule,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<SceneShape>>"))]
    pub shapes: Option<Vec<Json>>,
    /// Passes over THIS layer, in order, before it is composited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passes: Option<Vec<ScenePass>>,
}

/// The renderer's input: a viewport, an fps and a stack of layers.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct VideoScene {
    /// px.
    pub viewport: [f64; 2],
    /// Used by export only.
    pub fps: f64,
    /// ALREADY in draw order, bottom first.
    pub layers: Vec<VideoLayer>,
    /// The last clip's end across all layers (the export's frame count).
    pub duration_sec: f64,
    /// Passes over the FRAME the layers were composited into.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_passes: Option<Vec<ScenePass>>,
    /// Every named transition in the scene, by name: the two built in and the song's own.
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"./SceneTransition\").SceneTransition>")
    )]
    pub transitions: IndexMap<String, Transition>,
    /// Every STRETCH a fade names, by head node: the passes a shot runs through on its way in or out.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, Array<import(\"./ScenePass\").ScenePass>>", optional)
    )]
    pub stretches: Option<IndexMap<String, Vec<ScenePass>>>,
}

impl SceneClip {
    /// Where the shot's `still` flag stands, as the scene says it: only ever `true` or absent.
    pub fn is_still(&self) -> bool {
        self.still == Some(true)
    }
}
