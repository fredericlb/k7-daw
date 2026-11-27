// loops.rs — a loop as the desk holds it, and the two edges the platter is sent.
// A reader would get wrong that a length in BEATS becomes seconds at the record's OWN tempo, never the fader's (docs/turntables/loop.md).

use crate::js_round;
use crate::vocab::{DEFAULT_LOOP_BEATS, DeckTempo, LOOP_MAX_BEATS, LOOP_MIN_BEATS};
use serde::{Deserialize, Serialize};

/// A loop on one deck. `start` is in source seconds; `beats` is of the record's own grid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeckLoop {
    pub start: f64,
    pub beats: f64,
    /// Momentary: letting go hands the record back where it would have reached.
    pub roll: bool,
}

/// The two edges as fractions of the record, which is what `LoopStart` and `LoopEnd` take.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopEdges {
    pub start: f64,
    pub end: f64,
}

/// A loop in source seconds, for drawing it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopSpan {
    pub start: f64,
    pub end: f64,
}

/// One beat in source seconds: `natural`, not `target`, since the grid is a fact about the file.
pub fn beat_seconds(tempo: Option<&DeckTempo>) -> f64 {
    match tempo {
        Some(t) if t.natural > 0.0 => 60.0 / t.natural,
        _ => 0.0,
    }
}

fn seconds_of(tempo: Option<&DeckTempo>) -> f64 {
    tempo.map_or(0.0, |t| t.seconds)
}

/// Can this deck loop at all? It takes a measured tempo and a length.
pub fn can_loop(tempo: Option<&DeckTempo>) -> bool {
    beat_seconds(tempo) > 0.0 && seconds_of(tempo) > 0.0
}

/// The nearest beat to `at`, never the next one; `at` itself when the deck has no grid.
pub fn snap_to_beat(at: f64, tempo: Option<&DeckTempo>) -> f64 {
    let period = beat_seconds(tempo);
    if !(period > 0.0) {
        return at;
    }
    let phase = tempo.and_then(|t| t.phase).unwrap_or(0.0);
    let snapped = phase + js_round((at - phase) / period) * period;
    // A grid whose first beat is late puts the nearest one before the record starts.
    if snapped >= 0.0 {
        snapped
    } else {
        snapped + (-snapped / period).ceil() * period
    }
}

/// A length halved or doubled, held between the two ends.
pub fn resized(beats: f64, factor: f64) -> f64 {
    LOOP_MAX_BEATS.min(LOOP_MIN_BEATS.max(beats * factor))
}

/// How many beats long a manual IN to OUT is: a whole beat above one, the halving ladder below it.
pub fn beats_between(from: f64, to: f64, tempo: Option<&DeckTempo>) -> f64 {
    let period = beat_seconds(tempo);
    if !(period > 0.0) {
        return DEFAULT_LOOP_BEATS;
    }
    let raw = (to - from) / period;
    if raw >= 1.0 {
        return LOOP_MAX_BEATS.min(js_round(raw));
    }
    let mut beats = 1.0;
    while beats > LOOP_MIN_BEATS && raw < beats * 0.75 {
        beats /= 2.0;
    }
    beats
}

/// The edges to send, or None when they would not be a loop: off the end, or no grid at all.
pub fn loop_edges(lp: &DeckLoop, tempo: Option<&DeckTempo>) -> Option<LoopEdges> {
    let period = beat_seconds(tempo);
    let length = seconds_of(tempo);
    if !(period > 0.0) || !(length > 0.0) {
        return None;
    }
    let start = 0.0_f64.max(length.min(lp.start));
    let end = length.min(start + lp.beats * period);
    // Under a beat of room left is a hang rather than a loop: the platter refuses it too.
    if end - start < period / 16.0 {
        return None;
    }
    Some(LoopEdges {
        start: start / length,
        end: end / length,
    })
}

/// Where a jump of `beats` lands, held inside the record; snapped only when quantising is on.
pub fn beat_jump(needle: f64, beats: f64, tempo: Option<&DeckTempo>, quantise: bool) -> f64 {
    let period = beat_seconds(tempo);
    let length = seconds_of(tempo);
    if !(period > 0.0) || !(length > 0.0) {
        return needle;
    }
    let moved = needle + beats * period;
    let to = if quantise { snap_to_beat(moved, tempo) } else { moved };
    0.0_f64.max(length.min(to))
}

/// One param write: a name and a value, for whichever platters a deck has.
pub type LoopParam = (String, f64);

/// What to send, IN ORDER: turning a loop off moves the END alone, and opening one sends `Roll` first.
pub fn loop_params(edges: Option<LoopEdges>, roll: bool, resize: bool) -> Vec<LoopParam> {
    let p = |n: &str, v: f64| (n.to_owned(), v);
    // An end below any start at all is OFF, and one message can never be half-applied.
    let Some(e) = edges else { return vec![p("LoopEnd", 0.0)] };
    if resize {
        return vec![p("LoopEnd", e.end)];
    }
    vec![
        p("Roll", if roll { 1.0 } else { 0.0 }),
        p("LoopStart", e.start),
        p("LoopEnd", e.end),
    ]
}

/// The loop in source seconds, or None when there is nothing to draw.
pub fn loop_span(lp: Option<&DeckLoop>, tempo: Option<&DeckTempo>) -> Option<LoopSpan> {
    let period = beat_seconds(tempo);
    let lp = lp?;
    if !(period > 0.0) {
        return None;
    }
    let end = seconds_of(tempo).min(lp.start + lp.beats * period);
    (end > lp.start).then_some(LoopSpan { start: lp.start, end })
}
