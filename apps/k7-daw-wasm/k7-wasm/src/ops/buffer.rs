// ops/buffer.rs — the buffer.* operations: what the editor reads off the open buffer's text, half-typed or not.
// A reader would get wrong that every span is in UTF-16 units of the text sent: the page maps it through the edits made since (apps/k7-daw-frontend/packages/ui-editor/docs/k7xTree.md).

use crate::error::WasmError;
use crate::ops::write;
use k7x::cursor::cursor;
use k7x::scan::scalar_field;
use k7x::tokens::{PreviewReq as Req, key_tokens, preview_triggers, value_tokens};
use k7x::{Bump, parse_tree};
use serde::{Deserialize, Serialize};
use song_edit::buffer_text::{
    BufferReads, ClipGrid, WindowBand, buffer_reads, clip_window_band, scan_clip_window, scan_slices,
};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BufferTextArgs {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BufferReadsArgs {
    pub text: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BufferScalarArgs {
    pub text: String,
    pub key: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BufferBandArgs {
    pub text: String,
    pub grid: ClipGrid,
    #[serde(default)]
    pub editable: bool,
}

/// Where the cursor sits in key terms.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct K7xCursor {
    /// The keys of the mapping the cursor is in (or whose value it fills), outermost first; list items add no key.
    pub path: Vec<String>,
    /// Is the cursor where a key goes: a statement's start, after `.` in a chain, or an object's key?
    pub in_key: bool,
    /// The key whose value the cursor is filling, when it is filling one.
    pub value_of: Option<String>,
    /// Is the cursor at an item of a list, which may be a value or a chain of keys?
    pub in_seq_item: bool,
    /// Is the cursor inside a string it is typing?
    pub in_string: bool,
}

/// A string value of the buffer with the field it belongs to; the span is the tree's, quotes included.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ValueToken {
    pub from: usize,
    pub to: usize,
    /// The page takes one character off each end once it has mapped the span.
    pub quoted: bool,
    pub field: Option<String>,
    pub key_path: Vec<String>,
    pub value: String,
}

/// A map key of the buffer with the keys above it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyToken {
    pub from: usize,
    pub to: usize,
    pub quoted: bool,
    pub key_path: Vec<String>,
    pub key: String,
}

/// What a ▶ plays: a file, a clip, or a window of a source in seconds.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PreviewReq {
    Source { source: String },
    Clip { clip: String },
    Slice { source: String, from: f64, to: f64 },
}

/// A line that earns a ▶: the span the page anchors it on, in the tree's coordinates.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewTrigger {
    pub from: usize,
    pub to: usize,
    pub req: PreviewReq,
}

/// A slice rack's symbol: its window and where its numbers and name sit, in the tree's coordinates.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SliceSym {
    pub symbol: String,
    pub from: f64,
    pub to: f64,
    pub from_range: (usize, usize),
    pub to_range: (usize, usize),
    pub key_range: (usize, usize),
}

/// The edges a clip file states at its root, as written.
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipWindow {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number | string"))]
    pub from: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number | string"))]
    pub to: Option<serde_json::Value>,
}

/// The buffer family's requests: `{"op": "buffer.cursor", "args": {"text": "…"}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum BufferRequest {
    #[serde(rename = "buffer.reads")]
    Reads(BufferReadsArgs),
    #[serde(rename = "buffer.cursor")]
    Cursor(BufferTextArgs),
    #[serde(rename = "buffer.scalar")]
    Scalar(BufferScalarArgs),
    #[serde(rename = "buffer.value_tokens")]
    ValueTokens(BufferTextArgs),
    #[serde(rename = "buffer.key_tokens")]
    KeyTokens(BufferTextArgs),
    #[serde(rename = "buffer.preview_triggers")]
    PreviewTriggers(BufferTextArgs),
    #[serde(rename = "buffer.slices")]
    Slices(BufferTextArgs),
    #[serde(rename = "buffer.clip_window")]
    ClipWindow(BufferTextArgs),
    #[serde(rename = "buffer.clip_band")]
    ClipBand(BufferBandArgs),
}

/// The tree of a text, or None where it does not read.
fn with_tree<T>(text: &str, f: impl FnOnce(&k7x::MapNode) -> T) -> Option<T> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).ok()?;
    Some(f(&root))
}

fn edge(e: &song_edit::buffer_text::Edge) -> serde_json::Value {
    match e {
        song_edit::buffer_text::Edge::Seconds(n) => serde_json::json!(n),
        song_edit::buffer_text::Edge::Timecode(s) => serde_json::json!(s),
    }
}

pub fn dispatch(request: BufferRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use BufferRequest as R;
    match request {
        R::Reads(a) => write(out, &{
            let r: BufferReads = buffer_reads(&a.text, &a.path);
            r
        }),
        R::Cursor(a) => {
            let c = cursor(&a.text);
            write(
                out,
                &K7xCursor {
                    path: c.path,
                    in_key: c.in_key,
                    value_of: c.value_of,
                    in_seq_item: c.in_seq_item,
                    in_string: c.in_string,
                },
            )
        }
        R::Scalar(a) => write(out, &scalar_field(&a.text, &a.key)),
        R::ValueTokens(a) => write(
            out,
            &with_tree(&a.text, |root| value_tokens(root, &a.text))
                .unwrap_or_default()
                .into_iter()
                .map(|t| ValueToken {
                    from: t.from,
                    to: t.to,
                    quoted: t.quoted,
                    field: t.field,
                    key_path: t.key_path,
                    value: t.value,
                })
                .collect::<Vec<_>>(),
        ),
        R::KeyTokens(a) => write(
            out,
            &with_tree(&a.text, |root| key_tokens(root, &a.text))
                .unwrap_or_default()
                .into_iter()
                .map(|t| KeyToken {
                    from: t.from,
                    to: t.to,
                    quoted: t.quoted,
                    key_path: t.key_path,
                    key: t.key,
                })
                .collect::<Vec<_>>(),
        ),
        R::PreviewTriggers(a) => write(
            out,
            &with_tree(&a.text, |root| preview_triggers(root, &a.text))
                .unwrap_or_default()
                .into_iter()
                .map(|t| PreviewTrigger {
                    from: t.from,
                    to: t.to,
                    req: match t.req {
                        Req::Source { source } => PreviewReq::Source { source },
                        Req::Clip { clip } => PreviewReq::Clip { clip },
                        Req::Slice { source, from, to } => PreviewReq::Slice { source, from, to },
                    },
                })
                .collect::<Vec<_>>(),
        ),
        R::Slices(a) => write(
            out,
            &scan_slices(&a.text).map(|v| {
                v.into_iter()
                    .map(|s| SliceSym {
                        symbol: s.symbol,
                        from: s.from,
                        to: s.to,
                        from_range: s.from_range,
                        to_range: s.to_range,
                        key_range: s.key_range,
                    })
                    .collect::<Vec<_>>()
            }),
        ),
        R::ClipWindow(a) => {
            let (from, to) = scan_clip_window(&a.text);
            write(
                out,
                &ClipWindow {
                    from: from.as_ref().map(edge),
                    to: to.as_ref().map(edge),
                },
            )
        }
        R::ClipBand(a) => {
            let band: Option<WindowBand> = clip_window_band(&a.text, a.grid, a.editable);
            write(out, &band)
        }
    }
}
