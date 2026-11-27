// layout.rs — the timeline's extent and scale: how many whole bars the song spans, and where the zoom puts a beat on screen.
// At rest and fit the beat range fills the pane exactly; zoomed in the lane grows past it, zoomed out the range grows instead.

use crate::js;

/// The song's extent in whole bars (the caret moves within it): a timeline's view reaches at least its fixed end.
pub fn content_beats(ends: &[f64], timeline_length: f64, bar_beats: f64) -> f64 {
    let mut end = js::max(16.0, timeline_length);
    for &e in ends {
        end = js::max(end, e);
    }
    ((end + 1.0) / bar_beats).ceil() * bar_beats
}

/// What a timeline is drawn at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub beat_lo: f64,
    pub beat_hi: f64,
    /// Pixels per beat.
    pub ppb: f64,
    /// The width of the lane in pixels.
    pub lane_w: f64,
    /// The last beat drawn: the zoom's end, or the song's rounded-up extent.
    pub last_beat: f64,
    /// Beats per second: the whole conversion, exact because there is ONE tempo, clamped off zero.
    pub bps: f64,
}

/// The layout for a pane of `pane_w` px with a `gutter` that does not scale, at `zoom_x`, narrowed to `view` (a beat range) or not.
pub fn timeline_layout(
    view: Option<(f64, f64)>,
    content: f64,
    zoom_x: f64,
    pane_w: f64,
    gutter: f64,
    px_per_beat: f64,
    tempo: f64,
) -> Layout {
    let beat_lo = view.map_or(0.0, |v| v.0);
    // Below fit the song is drawn past its end, only at rest: a zoomed rectangle IS its range.
    let beat_hi = view.map_or((content / js::min(1.0, zoom_x)).ceil() - 1.0, |v| v.1);
    let avail_w = js::max(0.0, pane_w - gutter);
    let span_beats = beat_hi - beat_lo + 1.0;
    // zoomX > 1 grows the lane past the pane so it can be panned; at or below 1 the lane is the pane and beat_hi grew instead.
    let zoom_lane = js::max(1.0, zoom_x);
    Layout {
        beat_lo,
        beat_hi,
        ppb: if avail_w > 0.0 {
            (avail_w / span_beats) * zoom_lane
        } else {
            px_per_beat
        },
        lane_w: if avail_w > 0.0 {
            avail_w * zoom_lane
        } else {
            span_beats * px_per_beat
        },
        last_beat: if view.is_some() { beat_hi } else { beat_hi + 1.0 },
        bps: js::max(1e-6, tempo / 60.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extent_is_whole_bars_of_at_least_sixteen_beats_and_the_fixed_end() {
        assert_eq!(content_beats(&[], 0.0, 4.0), 20.0);
        assert_eq!(content_beats(&[30.0, 8.0], 0.0, 4.0), 32.0);
        assert_eq!(content_beats(&[], 40.0, 4.0), 44.0);
    }

    #[test]
    fn at_rest_and_fit_the_beat_range_fills_the_pane() {
        let l = timeline_layout(None, 20.0, 1.0, 1000.0, 200.0, 20.0, 120.0);
        assert_eq!((l.beat_lo, l.beat_hi, l.last_beat), (0.0, 19.0, 20.0));
        assert_eq!((l.ppb, l.lane_w, l.bps), (40.0, 800.0, 2.0));
    }

    #[test]
    fn zoomed_in_the_lane_grows_past_the_pane_and_zoomed_out_the_range_grows() {
        let l = timeline_layout(None, 20.0, 2.0, 1000.0, 200.0, 20.0, 120.0);
        assert_eq!((l.beat_hi, l.ppb, l.lane_w), (19.0, 80.0, 1600.0));
        let l = timeline_layout(None, 20.0, 0.5, 1000.0, 200.0, 20.0, 120.0);
        assert_eq!((l.beat_hi, l.ppb, l.lane_w), (39.0, 20.0, 800.0));
    }

    #[test]
    fn a_zoomed_rectangle_is_its_own_range() {
        let l = timeline_layout(Some((8.0, 15.0)), 20.0, 1.0, 1000.0, 200.0, 20.0, 120.0);
        assert_eq!((l.beat_lo, l.beat_hi, l.last_beat, l.ppb), (8.0, 15.0, 15.0, 100.0));
    }

    #[test]
    fn a_pane_not_yet_measured_falls_back_to_the_default_scale_and_tempo_zero_is_clamped() {
        let l = timeline_layout(None, 20.0, 1.0, 0.0, 200.0, 20.0, 0.0);
        assert_eq!((l.ppb, l.lane_w), (20.0, 400.0));
        assert_eq!(l.bps, 1e-6);
    }
}
