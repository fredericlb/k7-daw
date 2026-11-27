// ops/eyes.rs — the eyes.* operations: what the page's compositor asks of Rust, from a layer's schedule to a `.cube` file.
// A reader would get wrong where a LUT's floats go: not in the JSON answer but in `floats`, which the page reads as a view.

use crate::error::WasmError;
use crate::ops::write;
use ::eyes::SceneClip;
use ::eyes::cameras::{Device, match_camera};
use ::eyes::cube::parse_cube;
use ::eyes::schedule::schedule;
use ::eyes::transitions::builtins;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScheduleArgs {
    pub clips: Vec<SceneClip>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LensArgs {
    pub label: String,
    pub devices: Vec<Device>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CubeArgs {
    pub text: String,
}

/// A `.cube` read: its edge size; the entries are `size`³ RGBA floats in `Session::floats`, valid until the next call.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CubeAnswer {
    pub size: u32,
}

/// The eyes family's requests: `{"op": "eyes.schedule", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum EyesRequest {
    /// A layer's clips as the table of spans the page looks up per frame.
    #[serde(rename = "eyes.schedule")]
    Schedule(ScheduleArgs),
    /// The two transitions the compositor owns, by name.
    #[serde(rename = "eyes.builtins")]
    Builtins {},
    /// Which lens a label names among the machine's devices, and why a fallback was taken.
    #[serde(rename = "eyes.match_camera")]
    MatchCamera(LensArgs),
    /// A `.cube` file read into a 3D texture's floats.
    #[serde(rename = "eyes.cube")]
    Cube(CubeArgs),
}

pub fn dispatch(floats: &mut Vec<f32>, request: EyesRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        EyesRequest::Schedule(a) => write(out, &schedule(&a.clips)),
        EyesRequest::Builtins {} => write(out, &builtins()),
        EyesRequest::MatchCamera(a) => write(out, &match_camera(&a.label, &a.devices)),
        EyesRequest::Cube(a) => {
            let cube = parse_cube(&a.text).map_err(WasmError::bad_request)?;
            *floats = cube.data;
            write(out, &CubeAnswer { size: cube.size as u32 })
        }
    }
}
