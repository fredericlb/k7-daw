// ops/text.rs — the text.* writers and readers: a file's k7x text in, the edited text out, as the page's editors ask for it.
// A reader would get wrong that every writer answers the splices it made, not only the text: an editor applies them to a live buffer.

use super::text_desk::{
    ApplyArgs, AssemblyArgs, AssemblyOp, ExpandArgs, MixArgs, MixOp, NameArgs, PinsArgs, PlaylistArgs, PlaylistOp,
    assembly_read, chain_read, mix_cues_read, mix_view_read, pins_of, pins_parsed, sugar_apply, sugar_expand,
};
use super::text_lane::{SequenceArgs, VideoArgs, VideoListArgs, sequence, sequence_data, video_list, video_step};
use crate::error::WasmError;
use crate::ops::k7x::K7xSyntax;
use crate::ops::k7x_edit::{EditFailure, K7Change};
use crate::ops::write;
use k7x::edit::{Document, EditError, Value};
use serde::{Deserialize, Serialize};
use song_edit::error::Error;
use song_edit::library_pins::write_pins;
use song_edit::{assembly_param, mix_cues, playlist_edit};

/// The text a family's ops left, the splices that made it and what each op answered, or the op that failed.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextAnswer {
    pub text: String,
    pub changes: Vec<K7Change>,
    /// Each op's own answer: false for a refused edit, true for one that wrote or has no refusal.
    pub results: Vec<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub error: Option<EditFailure>,
}

pub(super) fn failure(text: &str, op: usize, e: &Error) -> EditFailure {
    let syntax = match e {
        Error::Edit(EditError::Syntax(s)) => Some(K7xSyntax::of(text, s)),
        _ => None,
    };
    EditFailure {
        op,
        message: e.to_string(),
        syntax,
    }
}

pub(super) fn answer(doc: &Document, results: Vec<bool>, error: Option<EditFailure>) -> TextAnswer {
    TextAnswer {
        text: doc.text().to_owned(),
        changes: doc
            .changes()
            .iter()
            .map(|c| K7Change {
                start: c.start16,
                end: c.end16,
                insert: c.insert.clone(),
            })
            .collect(),
        results,
        error,
    }
}

/// A text that does not read, as the failure of the first op: a strict family edits nothing in it.
pub(super) fn unreadable(doc: &Document) -> Option<EditFailure> {
    doc.errors().first().map(|s| EditFailure {
        op: 0,
        message: s.message.clone(),
        syntax: Some(K7xSyntax::of(doc.text(), s)),
    })
}

/// Runs `step` over each op in order on one document; the first failure stops it, the edits before it kept.
pub(super) fn run<Op>(
    text: &str,
    strict: bool,
    ops: &[Op],
    mut step: impl FnMut(&mut Document, &Op) -> Result<bool, Error>,
) -> TextAnswer {
    let mut doc = Document::new(text);
    if strict && let Some(e) = unreadable(&doc) {
        return answer(&doc, vec![], Some(e));
    }
    let mut results = Vec::with_capacity(ops.len());
    for (i, op) in ops.iter().enumerate() {
        match step(&mut doc, op) {
            Ok(r) => results.push(r),
            Err(e) => {
                let f = failure(doc.text(), i, &e);
                return answer(&doc, results, Some(f));
            }
        }
    }
    answer(&doc, results, None)
}

/// A k7x value as JSON, a map in its own order.
pub(super) fn json_of(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Num(n) => serde_json::Number::from_f64(*n).map_or(serde_json::Value::Null, serde_json::Value::Number),
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::List(l) => serde_json::Value::Array(l.iter().map(json_of).collect()),
        Value::Map(m) => serde_json::Value::Object(m.iter().map(|(k, x)| (k.clone(), json_of(x))).collect()),
    }
}

pub(super) fn index(i: i64) -> Option<usize> {
    usize::try_from(i).ok()
}


#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextArgs {
    pub text: String,
}

/// The text family's requests: `{"op": "text.video", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum TextRequest {
    #[serde(rename = "text.video_list")]
    VideoList(VideoListArgs),
    #[serde(rename = "text.video")]
    Video(VideoArgs),
    #[serde(rename = "text.sequence_data")]
    SequenceData(TextArgs),
    #[serde(rename = "text.sequence")]
    Sequence(SequenceArgs),
    #[serde(rename = "text.mix_cues")]
    MixCues(TextArgs),
    #[serde(rename = "text.mix")]
    Mix(MixArgs),
    #[serde(rename = "text.mix_view")]
    MixView(TextArgs),
    #[serde(rename = "text.playlist_stub")]
    PlaylistStub(NameArgs),
    #[serde(rename = "text.playlist")]
    Playlist(PlaylistArgs),
    #[serde(rename = "text.pins_parse")]
    PinsParse(TextArgs),
    #[serde(rename = "text.pins_write")]
    PinsWrite(PinsArgs),
    #[serde(rename = "text.assembly_read")]
    AssemblyRead(TextArgs),
    #[serde(rename = "text.assembly")]
    Assembly(AssemblyArgs),
    #[serde(rename = "text.chain_read")]
    ChainRead(TextArgs),
    #[serde(rename = "text.sugar_expand")]
    SugarExpand(ExpandArgs),
    #[serde(rename = "text.sugar_apply")]
    SugarApply(ApplyArgs),
}

pub fn dispatch(request: TextRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        TextRequest::VideoList(a) => write(out, &video_list(&a)),
        TextRequest::Video(a) => write(out, &run(&a.text, false, &a.ops, |d, op| video_step(d, &a.at, op))),
        TextRequest::SequenceData(a) => write(out, &sequence_data(&a)?),
        TextRequest::Sequence(a) => write(out, &sequence(&a)),
        TextRequest::MixCues(a) => write(out, &mix_cues_read(&a)),
        TextRequest::Mix(a) => write(
            out,
            &run(&a.text, true, &a.ops, |d, op| {
                let MixOp::WithMarkers { reference, markers } = op;
                mix_cues::with_markers(&mut d.at(&[]), reference, markers).map(|()| true)
            }),
        ),
        TextRequest::MixView(a) => write(out, &mix_view_read(&a)),
        TextRequest::PlaylistStub(a) => write(
            out,
            &playlist_edit::playlist_stub(&a.name).map_err(|e| WasmError::bad_request(e.to_string()))?,
        ),
        TextRequest::Playlist(a) => write(
            out,
            &run(&a.text, true, &a.ops, |d, op| {
                let mut sc = d.at(&[]);
                match op {
                    PlaylistOp::Append { song, variant } => playlist_edit::append_item(&mut sc, song, variant.as_deref()),
                    PlaylistOp::Remove { index } => playlist_edit::remove_item(&mut sc, *index),
                    PlaylistOp::Move { from, to } => playlist_edit::move_item(&mut sc, *from, *to),
                }
            }),
        ),
        TextRequest::PinsParse(a) => write(out, &pins_parsed(&a)),
        TextRequest::PinsWrite(a) => write(
            out,
            &write_pins(&pins_of(&a.pins.0)).map_err(|e| WasmError::bad_request(e.to_string()))?,
        ),
        TextRequest::AssemblyRead(a) => write(out, &assembly_read(&a)),
        TextRequest::Assembly(a) => write(
            out,
            &run(&a.text, true, &a.ops, |d, op| {
                let mut sc = d.at(&[]);
                match op {
                    AssemblyOp::SetParam { name, node, param } => {
                        assembly_param::set_param(&mut sc, name, node, param).map(|()| true)
                    }
                    AssemblyOp::DropParam { name } => assembly_param::drop_param(&mut sc, name),
                }
            }),
        ),
        TextRequest::ChainRead(a) => write(out, &chain_read(&a)),
        TextRequest::SugarExpand(a) => write(out, &sugar_expand(&a)?),
        TextRequest::SugarApply(a) => write(out, &sugar_apply(&a)),
    }
}

