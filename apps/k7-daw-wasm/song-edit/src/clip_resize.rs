// clip_resize.rs — "this event should span these two beats" as the fields the model has: the material's window and the placement's start and loop.
// A reader would get wrong that a placement has no length: a resize is the window and the pass count, not one number.

use crate::start_time::round_beat;

/// The units a window is written in: seconds into a file, or beats into a sequence's notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialKind {
    Audio,
    Sequence,
}

impl MaterialKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            MaterialKind::Audio => "audio",
            MaterialKind::Sequence => "sequence",
        }
    }
}

/// The MATERIAL's window, which the event's is clamped inside.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub from: Option<f64>,
    pub to: Option<f64>,
}

/// The event as it stands: its EFFECTIVE window, its pass count and the cage it may not leave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeBefore {
    pub kind: MaterialKind,
    pub from: f64,
    pub to: Option<f64>,
    pub loops: f64,
    pub start_beats: f64,
    pub bounds: Option<Bounds>,
}

/// Where the drag put the two edges, on the timeline, in beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeTo {
    pub start_beats: f64,
    pub end_beats: f64,
    pub tempo: f64,
}

/// What to write: the placement's start, and the material's window and pass count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeResult {
    pub start_beats: f64,
    pub from: f64,
    pub to: f64,
    pub loops: f64,
}

fn to_material(beats: f64, kind: MaterialKind, tempo: f64) -> f64 {
    if kind == MaterialKind::Sequence {
        beats
    } else {
        beats * 60.0 / tempo
    }
}

fn to_beats(units: f64, kind: MaterialKind, tempo: f64) -> f64 {
    if kind == MaterialKind::Sequence {
        units
    } else {
        units * tempo / 60.0
    }
}

/// The EVENT's window and count spanning `start_beats..end_beats`, clamped inside `bounds` (apps/k7-daw-wasm/song/docs/clipResize.md).
#[must_use]
pub fn resize_to_window(before: &ResizeBefore, to: &ResizeTo) -> ResizeResult {
    let (kind, loops, tempo) = (before.kind, before.loops, to.tempo);
    let mat = |b: f64| to_material(b, kind, tempo);
    let beats = |u: f64| to_beats(u, kind, tempo);
    let bounds = before.bounds.unwrap_or_default();
    let lo = bounds.from.unwrap_or(0.0).max(0.0);
    let hi = bounds.to;

    let mut start_beats = to.start_beats;
    // The left edge moved by this much, so the window's in-point moves with it.
    let mut from = before.from + mat(start_beats - before.start_beats);
    if from < lo {
        // Clamped at the material's window; the event keeps its alignment.
        start_beats += beats(lo - from);
        from = lo;
    }
    let extent = (to.end_beats - start_beats).max(0.0);

    // A looped event keeps its window and changes how many times it comes round.
    let win = before.to.map_or(0.0, |t| t - before.from);
    if loops > 1.0 && win > 0.0 {
        let passes = crate::start_time::js_round(extent / beats(win)).max(1.0);
        return ResizeResult {
            start_beats,
            from: round_beat(from),
            to: round_beat(from + win),
            loops: passes,
        };
    }
    let end = from + mat(extent);
    ResizeResult {
        start_beats,
        from: round_beat(from),
        to: round_beat(hi.map_or(end, |h| end.min(h))),
        loops: 1.0,
    }
}
