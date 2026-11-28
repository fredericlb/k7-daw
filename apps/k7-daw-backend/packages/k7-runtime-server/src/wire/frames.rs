// frames.rs — the replies the page decodes (`io-levels`, the plugin parameters) as serde structs; the page's TypeScript
// types are generated from them and from `server/state_frame.rs` (docs/ts-types.md). A reader would get wrong that
// `type` and `session` are not fields: `tagged` and `broadcast_to` add them, and `fixtures/*.txt` hold the bytes.

use crate::engine::machine::InputDevice;
use crate::engine::state::ParamInfo;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[cfg(feature = "ts")]
use super::rec::Rec;

/// `value` as an object carrying its frame's `type`.
pub fn tagged<T: Serialize>(kind: &str, value: &T) -> Value {
    let mut v = serde_json::to_value(value).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut v {
        map.insert("type".into(), Value::String(kind.into()));
    }
    v
}

/// One wave input the engine can route a track FROM, as `io-levels` lists it.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "EngineInputDevice"))]
pub struct InputDeviceFrame {
    /// The interface's own name for the device.
    pub name: String,
    /// 1-based and matched EXACTLY.
    pub channels: Vec<i32>,
    pub enabled: bool,
}

impl From<InputDevice> for InputDeviceFrame {
    fn from(d: InputDevice) -> Self {
        InputDeviceFrame {
            name: d.name,
            channels: d.channels,
            enabled: d.enabled,
        }
    }
}

/// The `io-levels` reply: peak dB per channel set asked for, in order, plus the master output.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "IoLevelsReply"))]
pub struct IoLevelsFrame {
    pub inputs: Vec<f64>,
    /// Whether each set found a channel to meter.
    pub found: Vec<bool>,
    /// The tuner's reading per input, in the same order: Hz, or 0 when nothing convincing was heard.
    pub pitches: Vec<f64>,
    /// How periodic each window was, 0..1 — an uncertain note is shown faded rather than hidden.
    pub clarity: Vec<f64>,
    /// The interface's own grouping — the one thing that explains a channel that exists and meters nothing.
    pub devices: Vec<InputDeviceFrame>,
    /// The master output in dB (-100 == silent).
    pub master: f64,
}

/// One named setting of a discrete parameter, carrying the value that selects it.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParamState {
    pub label: String,
    pub value: f64,
}

fn states_of(p: &ParamInfo) -> Option<Vec<ParamState>> {
    (!p.states.is_empty()).then(|| {
        p.states
            .iter()
            .map(|(label, value)| ParamState {
                label: label.clone(),
                value: *value,
            })
            .collect()
    })
}

/// A live plugin's parameter range, `def` only when the plugin publishes a default.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParamMeta {
    pub min: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub def: Option<f64>,
    /// A DISCRETE parameter's named settings, each carrying the value that selects it — NOT index/(n-1), so send it back as given.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub states: Option<Vec<ParamState>>,
}

impl From<&ParamInfo> for ParamMeta {
    fn from(p: &ParamInfo) -> Self {
        ParamMeta {
            min: p.min,
            max: p.max,
            def: p.default,
            states: states_of(p),
        }
    }
}

/// One parameter of an installed plugin TYPE: its name and its range.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginParamInfo {
    pub name: String,
    pub min: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub def: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub states: Option<Vec<ParamState>>,
}

impl From<&ParamInfo> for PluginParamInfo {
    fn from(p: &ParamInfo) -> Self {
        PluginParamInfo {
            name: p.name.clone(),
            min: p.min,
            max: p.max,
            def: p.default,
            states: states_of(p),
        }
    }
}

/// The `plugin-param-list` reply; `found` says what an empty list cannot: could not load, or loaded and publishes nothing.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginParamList {
    pub format: String,
    pub id: String,
    pub found: bool,
    pub params: Vec<PluginParamInfo>,
}

/// A live plugin's parameters by display name.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginParams {
    /// The PluginDoc id (chain:module).
    pub id: String,
    #[cfg_attr(feature = "ts", ts(as = "Rec<f64>"))]
    pub params: BTreeMap<String, f64>,
    /// Only on the reply to `get-plugin-params`: a range can't change while loaded, so re-sending it 20×/s is waste.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<Rec<ParamMeta>>"))]
    pub meta: Option<BTreeMap<String, ParamMeta>>,
}
