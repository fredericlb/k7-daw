// ops/rules.rs — the rules.* operations: what a file's own text states, read where the page has only the text (a window, a project's tags).
// A reader would get wrong that a text which does not parse is an error here: a half-typed file answers null or an empty list, never throws.

use crate::error::WasmError;
use crate::ops::schema::TextArgs;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_edit::project_tags::project_tags_of;
use song_edit::seq_window::{SeqWindow as Window, WindowSpan as Span, seq_window_from_text, window_span};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SeqWindowArgs {
    pub text: String,
    /// The meter, 4/4 when left out.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub num: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub den: Option<f64>,
}

/// The window a sequence states, in beats: `to` is null for "wherever the notes end".
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "SeqWindow"))]
pub struct SeqWindowDto {
    pub from: f64,
    pub to: Option<f64>,
}

/// A window resolved against the notes it windows: both edges, in beats.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WindowSpan {
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WindowSpanArgs {
    pub win: SeqWindowDto,
    pub extent: f64,
}

/// The rules family's requests: `{"op": "rules.seq_window", "args": {"text": "…", "num": 4, "den": 4}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum RulesRequest {
    #[serde(rename = "rules.seq_window")]
    SeqWindow(SeqWindowArgs),
    #[serde(rename = "rules.window_span")]
    WindowSpan(WindowSpanArgs),
    #[serde(rename = "rules.project_tags")]
    ProjectTags(TextArgs),
}

pub fn dispatch(request: RulesRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        RulesRequest::SeqWindow(a) => write(
            out,
            &seq_window_from_text(&a.text, a.num.unwrap_or(4.0), a.den.unwrap_or(4.0)).map(|w| SeqWindowDto { from: w.from, to: w.to }),
        ),
        RulesRequest::WindowSpan(a) => write(
            out,
            &window_span(
                Window {
                    from: a.win.from,
                    to: a.win.to,
                },
                a.extent,
            )
            .map(|Span { from, to }| WindowSpan { from, to }),
        ),
        RulesRequest::ProjectTags(a) => write(out, &project_tags_of(&a.text)),
    }
}
