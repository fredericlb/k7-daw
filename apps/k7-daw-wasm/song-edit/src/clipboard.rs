// clipboard.rs — what `selection copy` puts aside and `selection paste` puts down: a rectangle's contents, in coordinates RELATIVE to its top-left corner.
// A reader would get wrong that a note or an event is IN a rectangle when it STARTS inside: copying and deleting share `notes_in_region`.

use serde::{Deserialize, Serialize};

/// JavaScript's truthiness of a number: neither 0 nor NaN.
fn js_truthy_num(n: f64) -> bool {
    n != 0.0 && !n.is_nan()
}

/// A start counts as inside a rectangle up to this much past its edge: a drag's float noise.
const EPS: f64 = 1e-9;

/// One copied note: `t` is beats from the rectangle's left edge, `k` semitones below its top row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CopiedNote {
    pub k: f64,
    pub t: f64,
    pub d: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub v: Option<f64>,
    /// A PAD's effects ride along: a copy is of the thing you copied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub repeat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub skip: Option<f64>,
}

/// A window edge as authored: seconds for a clip, a timecode for a sequence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum WindowEdge {
    Seconds(f64),
    Timecode(String),
}

/// The placement fields a copy carries beside `ref` and `start`: the shape a clip's `event` has.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EventAttrs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<WindowEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub to: Option<WindowEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fade_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fade_out: Option<String>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub loops: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warp: Option<bool>,
}

/// The authored fields worth carrying, rebuilt key by key: a newer producer's field is not carried into an `events add`.
fn attrs_of(e: &EventAttrs) -> EventAttrs {
    EventAttrs {
        from: e.from.clone(),
        to: e.to.clone(),
        fade_in: e.fade_in.clone().filter(|s| !s.is_empty()),
        fade_out: e.fade_out.clone().filter(|s| !s.is_empty()),
        loops: e.loops.filter(|n| *n > 1.0),
        disabled: e.disabled.filter(|b| *b),
        warp: e.warp.filter(|b| !*b),
    }
}

/// One copied event: `row` and `start` from the rectangle's corner, then the placement's OWN words, carried whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CopiedEvent {
    pub row: usize,
    #[serde(rename = "ref")]
    pub reference: String,
    pub start: f64,
    #[serde(flatten)]
    pub attrs: EventAttrs,
}

/// The shell's one clipboard, holding either kind: the wrong KIND is refused by name rather than coerced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Clipboard {
    Notes {
        /// The sequence it came from, named so a paste can say when it is landing somewhere else.
        sequence: String,
        notes: Vec<CopiedNote>,
        /// The rectangle's own size, for the status line and for a paste's bounds check.
        keys: f64,
        beats: f64,
        /// The rectangle's top KEY, so a roll's paste can put each note back on its own pitch.
        top: f64,
    },
    Events {
        events: Vec<CopiedEvent>,
        rows: usize,
        beats: f64,
    },
}

/// A note of the page's roll: the fields a copy reads.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RollNote {
    pub key: f64,
    pub time: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub vel: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub repeat: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub skip: Option<f64>,
}

/// The indices of the notes a rectangle HOLDS: a note is in when it STARTS inside.
#[must_use]
pub fn notes_in_region(notes: &[(f64, f64)], lo_key: f64, hi_key: f64, from: f64, to: f64) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, (key, time))| *key >= lo_key && *key <= hi_key && *time >= from - EPS && *time < to - EPS)
        .map(|(i, _)| i)
        .collect()
}

/// The notes inside the rectangle, relative to its corner; None for an empty rectangle.
#[must_use]
pub fn copy_notes(
    notes: &[RollNote],
    sequence: &str,
    lo_key: f64,
    hi_key: f64,
    from: f64,
    to: f64,
) -> Option<Clipboard> {
    let keys: Vec<(f64, f64)> = notes.iter().map(|n| (n.key, n.time)).collect();
    let inside = notes_in_region(&keys, lo_key, hi_key, from, to);
    if inside.is_empty() {
        return None;
    }
    let truthy = |n: Option<f64>| n.filter(|x| js_truthy_num(*x));
    Some(Clipboard::Notes {
        sequence: sequence.to_owned(),
        keys: hi_key - lo_key + 1.0,
        beats: to - from,
        top: hi_key,
        notes: inside
            .into_iter()
            .map(|i| {
                let n = &notes[i];
                CopiedNote {
                    // Rows DOWN from the top, so a paste offsets one way on both axes.
                    k: hi_key - n.key,
                    t: n.time - from,
                    d: n.duration,
                    v: n.vel.filter(|v| *v != 1.0),
                    repeat: truthy(n.repeat),
                    skip: truthy(n.skip),
                }
            })
            .collect(),
    })
}

/// A note put down at a caret.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PastedNote {
    pub t: f64,
    pub d: f64,
    pub k: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub v: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub repeat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub skip: Option<f64>,
}

/// …and back out at a caret: the rectangle's top-left lands under it, and anything pushed off the keyboard is DROPPED, not clamped.
#[must_use]
pub fn paste_notes(clip: &Clipboard, top_key: f64, at: f64) -> Vec<PastedNote> {
    let Clipboard::Notes { notes, .. } = clip else {
        return vec![];
    };
    notes
        .iter()
        .map(|n| PastedNote {
            k: top_key - n.k,
            t: at + n.t,
            d: n.d,
            v: n.v,
            // Unchanged by the offset: `repeat` counts inside the note's own duration and `skip` counts passes.
            repeat: n.repeat.filter(|x| js_truthy_num(*x)),
            skip: n.skip.filter(|x| js_truthy_num(*x)),
        })
        .filter(|n| n.k >= 0.0 && n.k <= 127.0 && n.t >= 0.0)
        .collect()
}

/// A lane of events the page holds: what a copy reads.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneEvents {
    pub events: Vec<LaneEvent>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneEvent {
    #[serde(rename = "ref")]
    pub reference: String,
    pub start: f64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub event: Option<EventAttrs>,
}

/// The events inside a rectangle of lanes × beats, relative to its corner: taken by START like the notes.
#[must_use]
pub fn copy_events(rows: &[LaneEvents], from: f64, to: f64) -> Option<Clipboard> {
    let mut events = Vec::new();
    for (row, lane) in rows.iter().enumerate() {
        for e in &lane.events {
            if e.start >= from - EPS && e.start < to - EPS {
                events.push(CopiedEvent {
                    row,
                    reference: e.reference.clone(),
                    start: e.start - from,
                    attrs: e.event.as_ref().map(attrs_of).unwrap_or_default(),
                });
            }
        }
    }
    (!events.is_empty()).then_some(Clipboard::Events {
        events,
        rows: rows.len(),
        beats: to - from,
    })
}

/// An event put down on a real lane.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PastedEvent {
    pub track: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub start: f64,
    #[serde(flatten)]
    pub attrs: EventAttrs,
}

/// …and back out onto real lanes: as many as the paste can reach, the rest dropped rather than folded onto the last one.
#[must_use]
pub fn paste_events(clip: &Clipboard, lanes: &[String], at: f64) -> Vec<PastedEvent> {
    let Clipboard::Events { events, .. } = clip else {
        return vec![];
    };
    events
        .iter()
        .filter_map(|e| {
            let track = lanes.get(e.row)?;
            let start = at + e.start;
            // The window and the fades ride along UNCHANGED while only `start` moves: they measure into the material.
            (start >= 0.0).then(|| PastedEvent {
                track: track.clone(),
                reference: e.reference.clone(),
                start,
                attrs: attrs_of(&e.attrs),
            })
        })
        .collect()
}

/// How many of a paste's items would be dropped for not fitting: what the status line reports.
#[must_use]
pub fn dropped_count(kept: usize, clip: &Clipboard) -> usize {
    let total = match clip {
        Clipboard::Notes { notes, .. } => notes.len(),
        Clipboard::Events { events, .. } => events.len(),
    };
    total.saturating_sub(kept)
}
