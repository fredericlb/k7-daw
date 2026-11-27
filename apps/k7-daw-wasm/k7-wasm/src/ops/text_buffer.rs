// ops/text_buffer.rs — the text.* writers of the open buffer: a clip's window, a slice symbol's window, a sequence's whole notes list.
// A reader would get wrong that a sequence's pad effects ride along: a note written back keeps the `repeat` and `skip` of the note it was.

use super::text::{TextAnswer, TextArgs, answer, run, unreadable};
use super::text_lane::sequence_data;
use crate::ops::k7x_edit::{Seg, path};
use k7x::edit::{Document, Value, js_number};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use song_edit::buffer_text::{ClipGrid, js_number_of, set_clip_window, set_slice_window};
use song_edit::start_time::js_round;
use song_edit::workspace_doc::edit_sugared;
use song_schema::round_beat;
use std::collections::HashMap;

/// A clip file's window as a drag set it: `from` of 0 and a `to` that is null (the file's end) say it by leaving the key out.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipWindowArgs {
    pub text: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub to: Option<f64>,
    pub grid: ClipGrid,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SliceWindowArgs {
    pub text: String,
    pub symbol: String,
    pub from: f64,
    pub to: f64,
}

/// A note as the grids hold it: onset and length in beats, MIDI key, velocity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GridNote {
    pub t: f64,
    pub d: f64,
    pub k: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub v: Option<f64>,
}

/// What the grids read off a sequence: its notes, expanded, and the rack it names.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SequenceRead {
    pub notes: Vec<GridNote>,
    pub use_rack: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NotesWriteArgs {
    pub text: String,
    pub notes: Vec<GridNote>,
}

pub(super) fn clip_window(a: &ClipWindowArgs) -> TextAnswer {
    run(&a.text, true, &[()], |d, ()| {
        set_clip_window(d, a.from.unwrap_or(f64::NAN), a.to, a.grid)?;
        Ok(true)
    })
}

pub(super) fn slice_window(a: &SliceWindowArgs) -> TextAnswer {
    run(&a.text, true, &[()], |d, ()| Ok(set_slice_window(d, &a.symbol, a.from, a.to)?))
}

fn finite(v: &Json) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}

/// The notes of a list as read: anything that is not a `{t, d, k}` map of finite numbers is skipped.
fn notes_of(list: &[Json]) -> Vec<GridNote> {
    list.iter()
        .filter_map(|raw| {
            let m = raw.as_object()?;
            Some(GridNote {
                t: finite(m.get("t")?)?,
                d: finite(m.get("d")?)?,
                k: finite(m.get("k")?)?,
                v: m.get("v").and_then(finite),
            })
        })
        .collect()
}

/// A sequence's notes and rack, or None while the text does not read or has no notes list.
pub(super) fn notes_read(a: &TextArgs) -> Option<SequenceRead> {
    let data = sequence_data(a).ok()?;
    let list = data.get("notes")?.as_array()?;
    let rack = data.get("use_rack").and_then(Json::as_str).map(|s| k7x::edit::js_trim(s).to_owned());
    Some(SequenceRead {
        notes: notes_of(list),
        use_rack: rack.filter(|s| !s.is_empty()),
    })
}

/// A plain value as JavaScript's `Number(…)` reads it.
fn number(v: Option<&Json>) -> f64 {
    match v {
        Some(Json::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Json::String(s)) => js_number_of(&Value::Str(s.clone())),
        Some(Json::Bool(b)) => f64::from(u8::from(*b)),
        Some(Json::Null) => 0.0,
        _ => f64::NAN,
    }
}

/// Round to 6 decimals: onsets come from drags, and an exact float would churn the file on every no-op write.
fn note_key(k: f64, t: f64) -> String {
    format!("{}@{}", js_number(js_round(k)), js_number(round_beat(t)))
}

/// The pad effects the file already holds, by the note they belong to.
fn effects_of(was: &[Json]) -> HashMap<String, Vec<(String, Value)>> {
    let mut out = HashMap::new();
    for n in was {
        let Some(m) = n.as_object() else { continue };
        if m.get("repeat").is_none() && m.get("skip").is_none() {
            continue;
        }
        let keep = |k: &str| m.get(k).map(|v| (k.to_owned(), crate::ops::k7x_edit::to_value(v)));
        out.insert(
            note_key(number(m.get("k")), number(m.get("t"))),
            ["repeat", "skip"].iter().filter_map(|k| keep(k)).collect(),
        );
    }
    out
}

/// The whole list written back, each note's pad effects carried from the note it was; None when the text does not read.
pub(super) fn notes_write(a: &NotesWriteArgs) -> Option<String> {
    let data = sequence_data(&TextArgs { text: a.text.clone() }).ok()?;
    let was = data.get("notes").and_then(Json::as_array).cloned().unwrap_or_default();
    let effects = effects_of(&was);
    let next: Vec<Value> = a
        .notes
        .iter()
        .map(|n| {
            let mut fields = vec![
                ("t".to_owned(), Value::Num(round_beat(n.t))),
                ("d".to_owned(), Value::Num(round_beat(n.d))),
                ("k".to_owned(), Value::Num(js_round(n.k))),
            ];
            if let Some(v) = n.v {
                fields.push(("v".to_owned(), Value::Num(round_beat(v))));
            }
            fields.extend(effects.get(&note_key(n.k, n.t)).cloned().unwrap_or_default());
            Value::object(fields)
        })
        .collect();
    let mut doc = Document::new(&a.text);
    if unreadable(&doc).is_some() {
        return None;
    }
    let done = edit_sugared(&mut doc, "sequences/sequence.k7x", false, |d| {
        Ok(d.apply_in(&path(&[Seg::Key("notes".to_owned())]), &Value::List(next))?)
    });
    done.ok().map(|()| answer(&doc, vec![true], None).text)
}
