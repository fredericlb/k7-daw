// pictures/shot.rs — a shot as the picture rule reads it: a borrow of its event and its seconds, then its span, fades and speed.
// A reader would get wrong that a shot's seconds are numbers JavaScript's `Number` makes: a bare "2.5" is 2.5 and "2:1" is NaN.

use crate::clips::js_min;
use crate::files::{Fade, Pitch, Placement, WindowEdge};

/// One event of a picture lane with the seconds it starts and runs; a shot a timeline places copies no field of the event (501).
#[derive(Clone, Copy, Debug)]
pub struct ShotIn<'p> {
    pub event: &'p Placement,
    pub start: f64,
    pub length: f64,
    pub timeline: Option<&'p str>,
    /// Whether a timeline inside another placed it (518).
    pub nested: bool,
}

impl<'p> ShotIn<'p> {
    /// A lane's own event, at the seconds it states.
    pub fn own(event: &'p Placement) -> ShotIn<'p> {
        ShotIn {
            event,
            start: seconds_of(event.start.as_ref()),
            length: event.length.map_or(0.0, |l| l.0),
            timeline: None,
            nested: false,
        }
    }
}

/// `Number(x ?? 0)` over a window edge: absent is 0, seconds as they stand, a colon-free timecode its number, any other NaN (502).
pub fn seconds_of(edge: Option<&WindowEdge>) -> f64 {
    match edge {
        None => 0.0,
        Some(WindowEdge::Seconds(s)) => *s,
        Some(WindowEdge::Timecode(t)) if t.contains(':') => f64::NAN,
        Some(WindowEdge::Timecode(t)) => t.parse().unwrap_or(f64::NAN),
    }
}

/// What a timecode's text weighs to parse beyond the visit: one for each 64 bytes, none for a number (decision 554).
pub(crate) fn edge_weight(edge: Option<&WindowEdge>) -> usize {
    match edge {
        Some(WindowEdge::Timecode(t)) => t.len() / 64,
        _ => 0,
    }
}

/// A fade's length in seconds: a map's `over` when finite and positive, else 0 (503).
pub fn fade_over(fade: Option<&Fade>) -> f64 {
    match fade {
        Some(Fade::Map(m)) if m.over.0.is_finite() && m.over.0 > 0.0 => m.over.0,
        _ => 0.0,
    }
}

/// One shot as the picture rule reads it, in seconds; `end` is `+inf` for an open last one.
#[derive(Clone, Copy, Debug)]
pub struct Shot<'p> {
    /// Its index in the list given, which a verb edits it by.
    pub index: usize,
    pub ref_: &'p str,
    pub start: f64,
    pub end: f64,
    pub source_from: f64,
    pub speed: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    pub silent: bool,
    pub keep_pitch: bool,
}

/// The picture's interval rule: a length, else the next entry's start, else open; every event times its neighbour (504).
pub fn shots_of<'p>(shots: &[ShotIn<'p>]) -> Vec<Shot<'p>> {
    (0..shots.len())
        .map(|i| {
            let s = &shots[i];
            let p = s.event;
            let end = if s.length > 0.0 {
                s.start + s.length
            } else {
                shots.get(i + 1).map_or(f64::INFINITY, |n| n.start)
            };
            let span = end - s.start;
            let speed = p.speed.filter(|v| v.is_finite()).unwrap_or(1.0);
            Shot {
                index: i,
                ref_: p.ref_.as_ref().map_or("", |r| r.0.as_str()),
                start: s.start,
                end,
                source_from: seconds_of(p.from.as_ref()),
                speed,
                fade_in: js_min(fade_over(p.fade_in.as_ref()), span),
                fade_out: if end.is_finite() {
                    js_min(fade_over(p.fade_out.as_ref()), span)
                } else {
                    0.0
                },
                silent: p.silent == Some(true),
                keep_pitch: p.pitch == Some(Pitch::Keep),
            }
        })
        .collect()
}
