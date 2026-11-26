// ops/timecode.rs — the timecode.* operations: a position or a duration read into beats, and beats written back.
// A reader would get wrong that the meter is checked here: its parts are floats, read as the page's project states them.

use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use song_schema::{SchemaError, beats_to_duration_on, beats_to_position_on, parse_duration_on, parse_position_on};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParseArgs {
    pub tc: String,
    pub num: f64,
    pub den: f64,
    /// A 1-based position when true, a 0-based duration when false.
    pub position: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FormatArgs {
    pub beats: f64,
    pub num: f64,
    pub den: f64,
    pub position: bool,
}

/// The timecode family's requests: `{"op": "timecode.parse", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum TimecodeRequest {
    #[serde(rename = "timecode.parse")]
    Parse(ParseArgs),
    #[serde(rename = "timecode.format")]
    Format(FormatArgs),
}

pub fn parse(a: &ParseArgs) -> Result<f64, WasmError> {
    let parsed = if a.position {
        parse_position_on(&a.tc, a.num, a.den)
    } else {
        parse_duration_on(&a.tc, a.num, a.den)
    };
    let beats = parsed?;
    if beats.is_finite() {
        Ok(beats)
    } else {
        Err(SchemaError::BadTimecode(format!("timecode '{}' is too large", a.tc)).into())
    }
}

pub fn format(a: &FormatArgs) -> Result<String, WasmError> {
    let text = if a.position {
        beats_to_position_on(a.beats, a.num, a.den)
    } else {
        beats_to_duration_on(a.beats, a.num, a.den)
    };
    Ok(text?)
}

pub fn dispatch(request: TimecodeRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        TimecodeRequest::Parse(a) => write(out, &parse(&a)?),
        TimecodeRequest::Format(a) => write(out, &format(&a)?),
    }
}
