// reads/clip.rs — what a clip says of itself: where its loop sits on an editor's axis, which materials the song places twice, the mixer's modules.
// A reader would get wrong that MIDI states a loop in the NOTES' beats and audio in the SOURCE's: the two panes' axes have different origins.

use super::model::{RClip, RTrack};
use serde::Serialize;
use std::collections::HashSet;

/// Beyond this many passes the repeat lines are noise, and a pane cannot show them apart anyway.
const MAX_MARKS: usize = 256;

/// `Math.round`: halves go up.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipLoop {
    /// Where the window that repeats starts, on the pane's own x axis, in beats.
    pub from: f64,
    /// Where it ends.
    pub to: f64,
    /// `to - from`: how long one pass lasts.
    pub win: f64,
    /// How many times the window plays inside the clip's placed length (at least 1).
    pub passes: f64,
    /// Where each pass after the first begins, in the same beats.
    pub repeats: Vec<f64>,
}

/// The loop a clip states, or none.
pub fn clip_loop(kind: &str, length: f64, loop_start: Option<f64>, loop_end: Option<f64>) -> Option<ClipLoop> {
    let start = loop_start.unwrap_or(0.0);
    let win = loop_end.unwrap_or(0.0) - start;
    if win.is_nan() || win <= 0.0 {
        return None;
    }
    let from = if kind == "midi" { start } else { 0.0 };
    let to = from + win;
    let span = if length > 0.0 { length } else { win };
    let passes = js_round(span / win).max(1.0);
    let mut repeats = Vec::new();
    let mut k = 1usize;
    while (k as f64) < passes && k <= MAX_MARKS {
        repeats.push(from + k as f64 * win);
        k += 1;
    }
    Some(ClipLoop {
        from,
        to,
        win,
        passes,
        repeats,
    })
}

/// What a MIDI event actually plays, on the roll's axis: the window it states, else the placement's extent; none when unplaced.
pub fn played_zone(kind: &str, length: f64, loop_start: Option<f64>, loop_end: Option<f64>) -> Option<(f64, f64)> {
    if let Some(l) = clip_loop(kind, length, loop_start, loop_end) {
        return Some((l.from, l.to));
    }
    (length > 0.0).then_some((0.0, length))
}

/// Where a material's first placement sits, and how many placements it has in all.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Instances {
    pub n: u32,
    pub track: String,
    pub start: f64,
}

/// How a placement names its material, one spelling for the clipboard and the ghost badge.
fn material_of(c: &RClip) -> &str {
    if c.name.is_empty() { &c.id } else { &c.name }
}

/// Every material of the song placed on track lanes, by ref in first-seen order, with how often and where first.
pub fn material_instances(tracks: &[RTrack]) -> Vec<(String, Instances)> {
    let mut out: Vec<(String, Instances)> = Vec::new();
    let mut at: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for t in tracks {
        if t.generated == Some(true) {
            continue;
        }
        for c in &t.clips {
            let m = material_of(c);
            match at.get(m) {
                None => {
                    at.insert(m.to_owned(), out.len());
                    out.push((
                        m.to_owned(),
                        Instances {
                            n: 1,
                            track: t.id.clone(),
                            start: c.start,
                        },
                    ));
                }
                Some(&i) => {
                    let found = &mut out[i].1;
                    found.n += 1;
                    // Strictly earlier, so a tie leaves the one already found.
                    if c.start < found.start - 1e-9 {
                        found.track = t.id.clone();
                        found.start = c.start;
                    }
                }
            }
        }
    }
    out
}

/// The mixer modules to draw, in track order: a track with none contributes nothing and a shared bus is one card.
pub fn mixer_modules(tracks: &[RTrack]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for t in tracks {
        let Some(name) = t
            .mixer
            .as_ref()
            .and_then(|m| m.module.as_deref())
            .filter(|m| !m.is_empty())
        else {
            continue;
        };
        if seen.insert(name.to_owned()) {
            out.push(name.to_owned());
        }
    }
    out
}
