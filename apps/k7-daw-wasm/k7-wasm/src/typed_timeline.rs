// typed_timeline.rs — the timeline's own typed calls: an edge drag, a fade, a crossfade's size, the song's extent and the scale it is drawn at.
// The convention is typed.rs's: numbers in, a number back, a few in the scalars.

use crate::abi::{set_scalars, with_f64_in};
use crate::ops::OK;
use crate::typed::{flag, len, opt};
use gesture::edge_drag::{Edge, crossfade_len, edge_resize, fade_len};
use gesture::layout::{content_beats, timeline_layout};
use gesture::snap::SnapMode;

/// Scalars: start, end. `edge` is 0 for the start edge, 1 for the end edge.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_edge_resize(
    edge: f64,
    orig_start: f64,
    orig_end: f64,
    dx: f64,
    ppb: f64,
    unit: f64,
    mode: f64,
) -> u32 {
    let edge = if edge == 0.0 { Edge::Start } else { Edge::End };
    let (start, end) = edge_resize(edge, orig_start, orig_end, dx, ppb, unit, SnapMode::from_code(mode));
    set_scalars(&[start, end]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_fade_len(fade_in: f64, dx: f64, ppb: f64, span: f64, orig: f64, free: f64) -> f64 {
    fade_len(flag(fade_in), dx, ppb, span, orig, flag(free))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_crossfade_len(dx: f64, ppb: f64) -> f64 {
    crossfade_len(dx, ppb)
}

/// The input holds the `n` end beats of every placement.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_content_beats(n: f64, timeline_length: f64, bar_beats: f64) -> f64 {
    with_f64_in(len(n), |ends| content_beats(ends, timeline_length, bar_beats))
}

/// Scalars: first beat, last beat in the range, pixels per beat, lane width, last beat drawn, beats per second.
/// A NaN `view_start` is no zoomed rectangle.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_timeline_layout(
    view_start: f64,
    view_end: f64,
    content: f64,
    zoom_x: f64,
    pane_w: f64,
    gutter: f64,
    px_per_beat: f64,
    tempo: f64,
) -> u32 {
    let view = opt(view_start).map(|s| (s, view_end));
    let l = timeline_layout(view, content, zoom_x, pane_w, gutter, px_per_beat, tempo);
    set_scalars(&[l.beat_lo, l.beat_hi, l.ppb, l.lane_w, l.last_beat, l.bps]);
    OK
}
