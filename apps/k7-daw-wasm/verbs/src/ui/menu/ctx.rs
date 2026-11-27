// ctx.rs — what the cursor was over when a menu opened, and which entity of the catalogue each context is.
// A reader would get wrong that `id` is a display name: it is the entity as commands name it (a lane id, a clip's ref, a chain name).

use serde::{Deserialize, Serialize};

/// The rectangle a click landed in: the lanes it covers, comma-joined, and its span.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Sel {
    pub tracks: String,
    pub from: String,
    pub to: String,
}

/// What is mapped to a control: a MIDI channel and a controller.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Cc {
    pub ch: f64,
    pub cc: f64,
}

/// What `transport loop` would do right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum LoopMode {
    Set,
    Clear,
}

/// The rectangle a drag left around notes, in keys and beats.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NoteRegion {
    pub lo: f64,
    pub hi: f64,
    pub from: f64,
    pub to: f64,
}

/// What the cursor was over. Optional flags are false when absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MenuCtx {
    /// `picked`: the lanes shift-clicked together with this one, when it is among them.
    Track {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        picked: Option<Vec<String>>,
    },
    Group {
        id: String,
    },
    /// A block in the graph: `missing` has no file, no plugin and no param; `ambient` is the engine's; `model` names a model this machine has not got; `native` is a plugin with its own window; `rack` holds a rack of sounds.
    Module {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        missing: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        ambient: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        model: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        native: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        rack: Option<bool>,
    },
    /// A placed clip: `track` and `tc` say which placement, `sel` the rectangle the click landed in.
    Clip {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        track: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        tc: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        sel: Option<Sel>,
    },
    /// One control on a module: `param` is the widget's bind, `cc` what is mapped to it, `automated` that a curve drives it.
    Param {
        id: String,
        param: String,
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        cc: Option<Cc>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        automated: Option<bool>,
    },
    CurvePoint {
        curve: String,
        param: String,
        tc: String,
    },
    CurveCard {
        curve: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        module: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        param: Option<String>,
    },
    Chain {
        id: String,
    },
    Sink {
        id: String,
    },
    /// A box: `uses` is the template ref, so a row may address either.
    Assembly {
        id: String,
        uses: String,
    },
    MarkerAdd {
        tc: String,
        #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        loop_mode: Option<LoopMode>,
    },
    Marker {
        name: String,
    },
    Timeline {
        id: String,
    },
    /// A stretch point in the warp view: `src` is the file second it pins, already spelled.
    WarpPoint {
        clip: String,
        src: String,
    },
    /// A shot on a picture lane: the lane, and where it starts in seconds.
    PictureShot {
        lane: String,
        at: String,
        name: String,
    },
    PictureLane {
        lane: String,
    },
    /// A rack's pad on a node's card.
    Tap {
        id: String,
        symbol: String,
    },
    /// A note on the roll: key and start as the verbs take them, `at` the caret a split would cut at, `region` the rectangle.
    Note {
        sequence: String,
        note: f64,
        start: f64,
        end: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        at: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        region: Option<NoteRegion>,
    },
}

impl MenuCtx {
    /// The word naming the thing clicked, dropped off the front of every row that starts with it: one word per context.
    pub(crate) fn clicked_head(&self) -> &'static str {
        match self {
            Self::Track { .. } | Self::Group { .. } => "track",
            Self::Module { .. } => "node",
            Self::Clip { .. } => "event",
            Self::CurvePoint { .. } => "curve",
            Self::Assembly { .. } => "assembly",
            Self::MarkerAdd { .. } | Self::Marker { .. } => "project",
            Self::Timeline { .. } => "timeline",
            Self::WarpPoint { .. } => "clip",
            Self::PictureShot { .. } | Self::PictureLane { .. } => "picture",
            Self::Note { .. } => "sequence note",
            Self::Param { .. } | Self::CurveCard { .. } | Self::Chain { .. } | Self::Sink { .. } | Self::Tap { .. } => {
                ""
            }
        }
    }

    /// Which entity each context is, as `commands.yaml`'s `menu:` names it; a chain and a sink open none.
    #[must_use]
    pub fn entity(&self) -> Option<&'static str> {
        Some(match self {
            Self::Track { .. } => "track",
            Self::Group { .. } => "group",
            Self::Module { .. } => "node",
            Self::Clip { .. } => "event",
            Self::Param { .. } | Self::CurveCard { .. } => "param",
            Self::CurvePoint { .. } => "curve",
            Self::Chain { .. } | Self::Sink { .. } => return None,
            Self::Assembly { .. } => "assembly",
            Self::MarkerAdd { .. } => "ruler",
            Self::Marker { .. } => "marker",
            Self::Timeline { .. } => "timeline",
            Self::WarpPoint { .. } => "warp",
            Self::PictureShot { .. } => "shot",
            Self::PictureLane { .. } => "picture",
            Self::Note { .. } => "note",
            Self::Tap { .. } => "tap",
        })
    }
}
