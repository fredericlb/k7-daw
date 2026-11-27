// edge_drag.rs — the timeline's drags on a clip's EDGE: a resize, which moves start and end and never crosses them, a fade, a length from one end inwards, and a crossfade's size.
// An edge may not cross the other: the floor is the grid's step while there is one, and a 64th once the gesture is free.

use crate::js;
use crate::snap::{SnapMode, snap_position, snap_to};

/// The shortest an event may be left by a FREE resize: a 64th.
const MIN_FREE_BEATS: f64 = 1.0 / 16.0;

/// Which edge of an event is in hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// Where a resize leaves an event's edges, as (start, end): the pointer `dx` px from the grab at `ppb`, on a grid of `unit` beats.
pub fn edge_resize(
    edge: Edge,
    orig_start: f64,
    orig_end: f64,
    dx: f64,
    ppb: f64,
    unit: f64,
    mode: SnapMode,
) -> (f64, f64) {
    let delta = dx / ppb;
    let min = if mode == SnapMode::Free { MIN_FREE_BEATS } else { unit };
    match edge {
        Edge::Start => (
            js::max(
                0.0,
                js::min(orig_end - min, snap_position(orig_start, delta, unit, mode)),
            ),
            orig_end,
        ),
        Edge::End => (
            orig_start,
            js::max(orig_start + min, snap_position(orig_end, delta, unit, mode)),
        ),
    }
}

/// A fade's live length in beats: it grows rightwards for a fade-in and leftwards for a fade-out, whole beats unless free, never past `span`.
pub fn fade_len(fade_in: bool, dx: f64, ppb: f64, span: f64, orig: f64, free: bool) -> f64 {
    let dx_beats = dx / ppb;
    let d = snap_to(if fade_in { dx_beats } else { -dx_beats }, if free { 0.0 } else { 1.0 });
    js::max(0.0, js::min(span, orig + d))
}

/// What a crossfade click makes, in beats: a drag sizes it instead.
pub const CROSSFADE_BEATS: f64 = 0.5;

/// A crossfade's length for a pointer `dx` pixels from where the press began: the click's default under 4 px, else eighths of a beat.
pub fn crossfade_len(dx_px: f64, ppb: f64) -> f64 {
    let dx = dx_px.abs();
    if dx < 4.0 {
        return CROSSFADE_BEATS;
    }
    js::max(0.125, js::round((2.0 * dx) / ppb / 0.125) * 0.125)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_edge_snaps_and_stops_a_step_short_of_the_end() {
        assert_eq!(
            edge_resize(Edge::Start, 4.0, 8.0, 12.0, 10.0, 1.0, SnapMode::Grid),
            (5.0, 8.0)
        );
        assert_eq!(
            edge_resize(Edge::Start, 4.0, 8.0, 90.0, 10.0, 1.0, SnapMode::Grid),
            (7.0, 8.0)
        );
        assert_eq!(
            edge_resize(Edge::Start, 4.0, 8.0, -90.0, 10.0, 1.0, SnapMode::Grid),
            (0.0, 8.0)
        );
    }

    #[test]
    fn the_end_edge_snaps_and_stops_a_step_past_the_start() {
        assert_eq!(
            edge_resize(Edge::End, 4.0, 8.0, 14.0, 10.0, 1.0, SnapMode::Grid),
            (4.0, 9.0)
        );
        assert_eq!(
            edge_resize(Edge::End, 4.0, 8.0, -90.0, 10.0, 1.0, SnapMode::Grid),
            (4.0, 5.0)
        );
    }

    #[test]
    fn a_free_resize_stops_at_a_64th_and_a_relative_one_keeps_the_edges_own_offset() {
        assert_eq!(
            edge_resize(Edge::End, 4.0, 8.0, -90.0, 10.0, 1.0, SnapMode::Free),
            (4.0, 4.0625)
        );
        let (_, end) = edge_resize(Edge::End, 4.0, 8.3, 14.0, 10.0, 1.0, SnapMode::Relative);
        assert!((end - 9.3).abs() < 1e-9);
    }

    #[test]
    fn a_fade_in_grows_rightwards_in_whole_beats_and_is_capped_by_the_clip() {
        assert_eq!(fade_len(true, 16.0, 10.0, 8.0, 1.0, false), 3.0);
        assert_eq!(fade_len(true, 990.0, 10.0, 8.0, 1.0, false), 8.0);
        assert_eq!(fade_len(true, -90.0, 10.0, 8.0, 1.0, false), 0.0);
    }

    #[test]
    fn a_fade_out_grows_leftwards_and_a_free_fade_follows_the_pointer() {
        assert_eq!(fade_len(false, -22.0, 10.0, 8.0, 1.0, false), 3.0);
        assert!((fade_len(true, 13.0, 10.0, 8.0, 1.0, true) - 2.3).abs() < 1e-9);
    }

    #[test]
    fn a_crossfade_click_makes_the_default_and_a_drag_sizes_it_in_eighths() {
        assert_eq!(crossfade_len(3.0, 20.0), CROSSFADE_BEATS);
        assert_eq!(crossfade_len(-3.0, 20.0), CROSSFADE_BEATS);
        assert_eq!(crossfade_len(40.0, 20.0), 4.0);
        assert_eq!(crossfade_len(4.0, 200.0), 0.125);
    }
}
