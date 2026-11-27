// ops/text_lane.rs — the text.* writers of a picture lane's `events:` and of a sequence's notes.
// A reader would get wrong that a sequence edit runs on the EXPANDED notes: its short form is put back by sugar's rule.

use super::text::{TextAnswer, TextArgs, answer, failure, index, json_of, unreadable};
use crate::error::WasmError;
use crate::ops::k7x_edit::{Seg, path, to_value};
use k7x::edit::{Document, Value};
use serde::{Deserialize, Serialize};
use song_edit::error::Error;
use song_edit::picture_animate;
use song_edit::picture_file::{
    add_shot, list_shots, remove_at, rename_fade_chains, set_field_at, set_field_by_ref, set_lane_rect, set_silent_at,
    set_speed_at, split_at,
};
use song_edit::sequence_file::{
    NoteIn, NoteMove, append_notes, double_sequence, move_note, move_notes, remove_note, set_note_effect,
    set_note_time_range, set_note_velocity, split_note,
};
use song_edit::sugar::sugar_for;
use song_edit::workspace_doc::edit_sugared;

// ---- a picture lane's `events:` ----------------------------------------------------------------

/// A placement of a picture lane, as the editor lists it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VideoPlacement {
    /// The `ref:` value, EMPTY on an ACTIVATION.
    #[serde(rename = "ref")]
    pub reference: String,
    pub start: f64,
    pub length: f64,
    pub source_from: f64,
    /// How fast its source runs, signed: present only when the file states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub speed: Option<f64>,
    /// Its sound is not played: present only when the file says so.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub silent: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VideoListArgs {
    pub text: String,
    /// The key path of the lane inside the file; empty when the file is the lane.
    #[serde(default)]
    pub at: Vec<Seg>,
}

/// One edit of a lane, named as the writer it runs.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum VideoOp {
    SetFieldAt {
        index: f64,
        field: String,
        value: f64,
    },
    SetFieldByRef {
        #[serde(rename = "ref")]
        reference: String,
        field: String,
        value: f64,
    },
    SetSpeedAt {
        index: f64,
        speed: f64,
    },
    SetSilentAt {
        index: f64,
        silent: bool,
    },
    RemoveAt {
        index: f64,
    },
    SplitAt {
        index: f64,
        cut: f64,
    },
    AddShot {
        #[serde(rename = "ref")]
        reference: String,
        start: f64,
        length: f64,
        source_from: f64,
    },
    ShiftAnimationAt {
        index: f64,
        delta: f64,
    },
    RenameFadeChains {
        renames: Vec<(String, String)>,
    },
    SetLaneRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VideoArgs {
    pub text: String,
    #[serde(default)]
    pub at: Vec<Seg>,
    pub ops: Vec<VideoOp>,
}

pub(super) fn video_list(a: &VideoListArgs) -> Vec<VideoPlacement> {
    let mut doc = Document::new(&a.text);
    let scope = doc.at(&path(&a.at));
    list_shots(&scope)
        .into_iter()
        .map(|s| VideoPlacement {
            reference: s.ref_,
            start: s.start,
            length: s.length,
            source_from: s.source_from,
            speed: s.speed,
            silent: s.silent.then_some(true),
        })
        .collect()
}

pub(super) fn video_step(doc: &mut Document, at: &[Seg], op: &VideoOp) -> Result<bool, Error> {
    let mut sc = doc.at(&path(at));
    match op {
        VideoOp::SetFieldAt { index: i, field, value } => {
            index(*i).map_or(Ok(false), |i| set_field_at(&mut sc, i, field, *value))
        }
        VideoOp::SetFieldByRef {
            reference,
            field,
            value,
        } => set_field_by_ref(&mut sc, reference, field, *value),
        VideoOp::SetSpeedAt { index: i, speed } => index(*i).map_or(Ok(false), |i| set_speed_at(&mut sc, i, *speed)),
        VideoOp::SetSilentAt { index: i, silent } => {
            index(*i).map_or(Ok(false), |i| set_silent_at(&mut sc, i, *silent))
        }
        VideoOp::RemoveAt { index: i } => index(*i).map_or(Ok(false), |i| remove_at(&mut sc, i)),
        VideoOp::SplitAt { index: i, cut } => index(*i).map_or(Ok(false), |i| split_at(&mut sc, i, *cut)),
        VideoOp::AddShot {
            reference,
            start,
            length,
            source_from,
        } => add_shot(&mut sc, reference, *start, *length, *source_from),
        VideoOp::ShiftAnimationAt { index: i, delta } => {
            index(*i).map_or(Ok(false), |i| picture_animate::shift_animation_at(&mut sc, i, *delta))
        }
        VideoOp::RenameFadeChains { renames } => rename_fade_chains(&mut sc, renames),
        VideoOp::SetLaneRect { x, y, w, h } => set_lane_rect(&mut sc, [*x, *y, *w, *h]),
    }
}

// ---- a sequence's notes ------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NoteArg {
    pub t: f64,
    pub d: f64,
    pub k: f64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub v: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub repeat: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub skip: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MoveArg {
    pub k: f64,
    pub t: f64,
    pub nk: f64,
    pub nt: f64,
}

/// One edit of a sequence, on its EXPANDED notes: the short form is put back by sugar's rule.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SequenceOp {
    /// The whole `notes` list written over the one in the file, changing only the notes that changed.
    SetNotes {
        #[cfg_attr(feature = "ts", ts(type = "unknown[]"))]
        notes: Vec<serde_json::Value>,
    },
    AppendNotes {
        notes: Vec<NoteArg>,
    },
    SetNoteTimeRange {
        k: f64,
        t: f64,
        new_t: f64,
        new_d: f64,
    },
    RemoveNote {
        k: f64,
        t: f64,
    },
    MoveNote {
        k: f64,
        t: f64,
        new_k: f64,
        new_t: f64,
    },
    MoveNotes {
        moves: Vec<MoveArg>,
    },
    SplitNote {
        k: f64,
        t: f64,
        at: f64,
    },
    SetNoteVelocity {
        k: f64,
        t: f64,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        v: Option<f64>,
    },
    SetNoteEffect {
        k: f64,
        t: f64,
        effect: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        n: Option<f64>,
    },
    DoubleSequence {
        num: f64,
        den: f64,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SequenceArgs {
    pub text: String,
    pub ops: Vec<SequenceOp>,
}

/// The file a sequence short form is judged by: any path under `sequences/`.
const SEQUENCE_PATH: &str = "sequences/sequence.k7x";

fn note_in(n: &NoteArg) -> NoteIn {
    NoteIn {
        t: n.t,
        d: n.d,
        k: n.k,
        v: n.v,
        repeat: n.repeat,
        skip: n.skip,
    }
}

fn sequence_step(doc: &mut Document, op: &SequenceOp) -> Result<(), Error> {
    match op {
        SequenceOp::SetNotes { notes } => {
            let list = Value::List(notes.iter().map(to_value).collect());
            Ok(doc.apply_in(&path(&[Seg::Key("notes".to_owned())]), &list)?)
        }
        SequenceOp::AppendNotes { notes } => append_notes(doc, &notes.iter().map(note_in).collect::<Vec<_>>()),
        SequenceOp::SetNoteTimeRange { k, t, new_t, new_d } => set_note_time_range(doc, *k, *t, *new_t, *new_d),
        SequenceOp::RemoveNote { k, t } => remove_note(doc, *k, *t),
        SequenceOp::MoveNote { k, t, new_k, new_t } => move_note(doc, *k, *t, *new_k, *new_t),
        SequenceOp::MoveNotes { moves } => move_notes(
            doc,
            &moves
                .iter()
                .map(|m| NoteMove {
                    k: m.k,
                    t: m.t,
                    nk: m.nk,
                    nt: m.nt,
                })
                .collect::<Vec<_>>(),
        ),
        SequenceOp::SplitNote { k, t, at } => split_note(doc, *k, *t, *at),
        SequenceOp::SetNoteVelocity { k, t, v } => set_note_velocity(doc, *k, *t, *v),
        SequenceOp::SetNoteEffect { k, t, effect, n } => set_note_effect(doc, *k, *t, effect, *n),
        SequenceOp::DoubleSequence { num, den } => double_sequence(doc, *num, *den),
    }
}

/// Runs the ops as `editSequence` did: on the expanded notes, the short form kept while it holds; a refusal leaves the text alone.
pub(super) fn sequence(a: &SequenceArgs) -> TextAnswer {
    let mut doc = Document::new(&a.text);
    if let Some(e) = unreadable(&doc) {
        return answer(&doc, vec![], Some(e));
    }
    let mut failed = None;
    let done = edit_sugared(&mut doc, SEQUENCE_PATH, false, |d| {
        for (i, op) in a.ops.iter().enumerate() {
            if let Err(e) = sequence_step(d, op) {
                failed = Some(i);
                return Err(e);
            }
        }
        Ok(())
    });
    match done {
        Ok(()) => answer(&doc, vec![true; a.ops.len()], None),
        Err(e) => {
            let fresh = Document::new(&a.text);
            let f = failure(&a.text, failed.unwrap_or(0), &e);
            answer(&fresh, vec![], Some(f))
        }
    }
}

/// A sequence's data with its short form expanded, or the sentence of the short form that does not read.
pub(super) fn sequence_data(a: &TextArgs) -> Result<serde_json::Value, WasmError> {
    let doc = Document::new(&a.text);
    let raw = doc.to_js().map_err(|e| WasmError::bad_request(e.to_string()))?;
    let Value::Map(_) = &raw else {
        return Ok(serde_json::json!({}));
    };
    let expanded = match sugar_for(SEQUENCE_PATH) {
        Some(s) => s.try_expand(&raw).map_err(|e| WasmError::bad_request(e.to_string()))?,
        None => raw,
    };
    Ok(json_of(&expanded))
}
