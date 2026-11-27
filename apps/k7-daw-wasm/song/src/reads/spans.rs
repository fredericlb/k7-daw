// reads/spans.rs — when each lane is SOUNDING a note or SHOWING a picture, in seconds, so the graph's cables ripple with them.
// A reader would get wrong that an open end is `None` here: JSON cannot carry an infinity, and the page reads null as one.

use super::model::{RTrack, RVideoTrack};
use serde::Serialize;
use indexmap::IndexMap;

/// The schema's default when a note states no velocity.
const FULL: f64 = 1.0;

/// One note as the timeline's measure sees it: when it sounds, in seconds, and how hard.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NoteSpan {
    pub from: f64,
    pub to: f64,
    pub vel: f64,
}

/// One lane's notes and the busiest instant they reach.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneNotes {
    pub spans: Vec<NoteSpan>,
    pub peak: f64,
}

/// The velocities sounding at `t`, summed; half-open, in the spans' order.
fn load_at(spans: &[NoteSpan], t: f64) -> f64 {
    let mut sum = 0.0;
    for n in spans {
        if t >= n.from && t < n.to {
            sum += n.vel;
        }
    }
    sum
}

/// Every lane's notes and the busiest instant each reaches, by lane id; a lane with no notes is left out.
pub fn note_spans(tracks: &[RTrack], tempo: Option<f64>) -> IndexMap<String, LaneNotes> {
    let bpm = tempo.filter(|t| *t > 0.0).unwrap_or(120.0);
    let per_beat = 60.0 / bpm;
    let mut out = IndexMap::new();
    for t in tracks {
        if t.mute == Some(true) || t.disabled == Some(true) {
            continue;
        }
        let mut spans = Vec::new();
        for c in t.clips.iter().filter(|c| c.kind == "midi" && c.disabled != Some(true)) {
            for n in &c.notes {
                let from = c.start + n.time;
                // A zero-length note still sounds: it gets an instant.
                let to = from + if n.duration > 0.0 { n.duration } else { 0.125 };
                spans.push(NoteSpan { from: from * per_beat, to: to * per_beat, vel: n.vel.unwrap_or(FULL) });
            }
        }
        if spans.is_empty() {
            continue;
        }
        let mut peak = 0.0;
        for at in &spans {
            let sum = load_at(&spans, at.from);
            if sum > peak {
                peak = sum;
            }
        }
        out.insert(t.id.clone(), LaneNotes { spans, peak });
    }
    out
}

/// A span of a picture lane in seconds; `to` is `None` for no end.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneSpan {
    pub from: f64,
    pub to: Option<f64>,
}

/// Each picture lane's spans by lane id: hidden, shape and a shot with no length are the realtime driver's three rules.
pub fn video_spans(tracks: &[RVideoTrack]) -> IndexMap<String, Vec<LaneSpan>> {
    let mut out = IndexMap::new();
    for t in tracks {
        if t.mute == Some(true) {
            continue;
        }
        let spans: Vec<LaneSpan> = if t.clips.is_empty() {
            if t.shapes.is_empty() { Vec::new() } else { vec![LaneSpan { from: 0.0, to: None }] }
        } else {
            t.clips
                .iter()
                .enumerate()
                .map(|(i, c)| LaneSpan {
                    from: c.start,
                    to: if c.length > 0.0 {
                        Some(c.start + c.length)
                    } else {
                        t.clips.get(i + 1).map(|n| n.start)
                    },
                })
                .collect()
        };
        if !spans.is_empty() {
            out.insert(t.id.clone(), spans);
        }
    }
    out
}
