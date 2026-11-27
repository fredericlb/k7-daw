// ops/media.rs — the media.* operations: a WebM's duration written into its head, a take's trim window, a waveform tile's colours.
// A reader would get wrong that bytes cross as base64 inside the JSON: a head is 64 kB at most, and a strip a few kilobytes, once per take or tile.

use crate::error::WasmError;
use crate::ops::write;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use waveform::spectrum;
use waveform::trim::{self, TrimWindow};
use waveform::webm_duration;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WebmArgs {
    /// The first bytes of the take, base64.
    pub head: String,
    pub duration_ms: f64,
}

/// What replaces the take's first `consumed` bytes; `consumed` 0 means the take already had a duration.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebmPatch {
    /// The replacement bytes, base64.
    pub bytes: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub consumed: usize,
}

/// Seconds into a file: where a clip starts and stops sounding.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TimeSpan {
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrimArgs {
    pub env: Vec<f64>,
    pub seconds: f64,
    pub gate_db: f64,
    #[serde(default)]
    pub within: Option<TimeSpan>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StripArgs {
    /// One entry a column: its low, mid and high peaks, or null when the file has no bands.
    pub columns: Vec<Option<[f64; 3]>>,
    /// The colour of a column with no bands, as red, green and blue.
    pub plain: [u8; 3],
}

/// The media family's requests: `{"op": "media.trimWindow", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum MediaRequest {
    #[serde(rename = "media.webmPatch")]
    WebmPatch(WebmArgs),
    #[serde(rename = "media.trimWindow")]
    TrimWindow(TrimArgs),
    /// An RGBA strip, four bytes a column, base64.
    #[serde(rename = "media.spectrumStrip")]
    SpectrumStrip(StripArgs),
}

fn webm_patch(a: &WebmArgs) -> Result<Option<WebmPatch>, WasmError> {
    let head = STANDARD
        .decode(&a.head)
        .map_err(|e| WasmError::bad_request(e.to_string()))?;
    Ok(webm_duration::patch_head(&head, a.duration_ms).map(|p| WebmPatch {
        bytes: STANDARD.encode(p.bytes),
        consumed: p.consumed,
    }))
}

pub fn dispatch(request: MediaRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        MediaRequest::WebmPatch(a) => write(out, &webm_patch(&a)?),
        MediaRequest::TrimWindow(a) => {
            let within = a.within.map(|w| TrimWindow { from: w.from, to: w.to });
            let found = trim::trim_window(&a.env, a.seconds, a.gate_db, within);
            write(out, &found.map(|w| TimeSpan { from: w.from, to: w.to }))
        }
        MediaRequest::SpectrumStrip(a) => write(out, &STANDARD.encode(spectrum::strip(&a.columns, a.plain))),
    }
}
