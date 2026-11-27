// bar_window.rs — the window of a clip on screen: how many bars, from which bar, and which span of seconds that is.
// The one clamp lives in `bar_window`: framing and anchoring hand it an unclamped start on purpose.

use crate::js;

/// The interval the clip plays, in seconds of the file.
#[derive(Clone, Copy, Debug)]
pub struct BarSpan {
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarWindow {
    /// Bars actually shown: never more than the interval has, fractional under a pinch.
    pub shown: f64,
    /// How many bars the interval spans, rounded up and at least 1.
    pub all_bars: f64,
    /// Seconds one bar spans.
    pub bar_sec: f64,
    /// The span to draw, in seconds from the start of the file.
    pub view_start: f64,
    pub view_end: f64,
}

/// Seconds one bar spans, guarded on a tempo or a time signature the doc has not answered yet.
pub fn bar_sec(tempo: f64, beats_per_bar: f64) -> f64 {
    (js::max(1.0, beats_per_bar) * 60.0) / js::max(1.0, tempo)
}

/// The window on a clip: `want` bars from `start_bar`, CLAMPED here, and guarded on a tempo the doc has not answered yet.
pub fn bar_window(span: BarSpan, want: f64, tempo: f64, beats_per_bar: f64, start_bar: f64) -> BarWindow {
    let bar_sec = bar_sec(tempo, beats_per_bar);
    let lo = js::max(0.0, span.from);
    let hi = js::max(lo, span.to);
    let all_bars = js::max(1.0, ((hi - lo) / bar_sec).ceil());
    let shown = js::min(js::max(1.0, want), all_bars);
    let start = js::max(0.0, js::min(start_bar, all_bars - shown));
    BarWindow {
        shown,
        all_bars,
        bar_sec,
        view_start: lo + start * bar_sec,
        view_end: js::min(hi, lo + (start + shown) * bar_sec),
    }
}

/// Bars of context kept either side of the event when the panel opens on it.
pub const FRAME_MARGIN_BARS: f64 = 1.0;

/// The window the panel OPENS on: the act span framed inside the reach, as (bars, start bar).
pub fn frame_window(reach: BarSpan, act: BarSpan, bar_sec: f64, margin: f64) -> (f64, f64) {
    if bar_sec.is_nan() || bar_sec <= 0.0 {
        return (1.0, 0.0);
    }
    let from = js::max(reach.from, act.from);
    let to = js::min(reach.to, js::max(act.to, act.from));
    let act_bars = js::max(1.0, ((to - from) / bar_sec).ceil());
    (
        act_bars + 2.0 * margin,
        ((from - reach.from) / bar_sec).floor() - margin,
    )
}

/// The next bar count for one wheel notch, `+1` on the way out so a one-bar window can still grow.
pub fn zoom_bars(bars: f64, delta_y: f64, all_bars: f64) -> f64 {
    let next = if delta_y > 0.0 { bars * 1.3 + 1.0 } else { bars / 1.3 };
    js::max(1.0, js::min(all_bars, js::round(next)))
}

/// The start bar that keeps the moment under the cursor where it is: unclamped, `bar_window` owning the edges.
pub fn anchor_bars(view_start: f64, view_end: f64, frac: f64, next_bars: f64, bar_sec: f64, span_from: f64) -> f64 {
    let anchor_sec = view_start + frac * (view_end - view_start);
    (anchor_sec - span_from) / bar_sec - frac * next_bars
}

#[cfg(test)]
mod tests {
    use super::*;

    // The clip panel shows twelve bars of the interval the clip PLAYS; at 120 bpm in 4/4 a bar is two seconds.
    const WHOLE: BarSpan = BarSpan { from: 0.0, to: 60.0 };

    fn win(shown: f64, all: f64, start: f64, end: f64) -> BarWindow {
        BarWindow {
            shown,
            all_bars: all,
            bar_sec: 2.0,
            view_start: start,
            view_end: end,
        }
    }
    fn view(span: BarSpan, bars: f64, start_bar: f64) -> (f64, f64) {
        let w = bar_window(span, bars, 120.0, 4.0, start_bar);
        (w.view_start, w.view_end)
    }

    #[test]
    fn shows_the_bars_asked_for_from_where_it_is_asked_to() {
        assert_eq!(bar_window(WHOLE, 12.0, 120.0, 4.0, 0.0), win(12.0, 30.0, 0.0, 24.0));
        assert_eq!(view(WHOLE, 12.0, 10.0), (20.0, 44.0));
    }

    #[test]
    fn is_caged_by_the_clips_from_and_to() {
        let span = BarSpan { from: 10.0, to: 30.0 };
        assert_eq!(bar_window(span, 12.0, 120.0, 4.0, 0.0), win(10.0, 10.0, 10.0, 30.0));
        assert_eq!(view(span, 3.0, 2.0), (14.0, 20.0));
    }

    #[test]
    fn keeps_the_window_on_the_interval_wherever_the_start_is_asked() {
        assert_eq!(view(WHOLE, 12.0, 25.0), (36.0, 60.0));
        assert_eq!(view(WHOLE, 12.0, -3.0).0, 0.0);
        assert_eq!(view(BarSpan { from: 10.0, to: 30.0 }, 3.0, -3.0).0, 10.0);
    }

    #[test]
    fn shows_the_whole_interval_when_it_is_shorter_than_the_window() {
        let w = bar_window(BarSpan { from: 0.0, to: 6.0 }, 12.0, 120.0, 4.0, 0.0);
        assert_eq!(w, win(3.0, 3.0, 0.0, 6.0));
    }

    #[test]
    fn never_ends_past_the_interval_even_on_a_part_bar_one() {
        let w = bar_window(BarSpan { from: 0.0, to: 5.0 }, 12.0, 120.0, 4.0, 0.0);
        assert_eq!((w.all_bars, w.view_end), (3.0, 5.0));
    }

    #[test]
    fn survives_a_project_whose_tempo_has_not_arrived() {
        let w = bar_window(WHOLE, 12.0, 0.0, 0.0, 0.0);
        assert!(w.all_bars > 0.0 && w.bar_sec.is_finite());
    }

    #[test]
    fn always_shows_at_least_one_bar_even_of_a_take_with_no_length() {
        let w = bar_window(BarSpan { from: 0.0, to: 0.0 }, 12.0, 120.0, 4.0, 0.0);
        assert_eq!((w.shown, w.all_bars), (1.0, 1.0));
    }

    fn frame(act: BarSpan) -> (f64, f64) {
        frame_window(WHOLE, act, 2.0, FRAME_MARGIN_BARS)
    }

    #[test]
    fn framing_takes_the_events_bars_plus_a_bar_of_context_either_side() {
        assert_eq!(frame(BarSpan { from: 20.0, to: 30.0 }), (7.0, 9.0));
        let (bars, start) = frame(BarSpan { from: 20.0, to: 30.0 });
        assert_eq!(view(WHOLE, bars, start), (18.0, 32.0));
    }

    #[test]
    fn framing_asks_for_a_start_before_the_file_when_the_event_opens_it() {
        let (bars, start) = frame(BarSpan { from: 0.0, to: 4.0 });
        assert_eq!(start, -1.0);
        assert_eq!(view(WHOLE, bars, start).0, 0.0);
    }

    #[test]
    fn framing_a_sliver_asks_for_a_bar_not_none() {
        assert_eq!(frame(BarSpan { from: 10.0, to: 10.3 }).0, 3.0);
    }

    #[test]
    fn framing_does_not_reach_outside_the_file_and_survives_a_missing_tempo() {
        assert_eq!(frame(BarSpan { from: 55.0, to: 90.0 }), (5.0, 26.0));
        assert_eq!(
            frame_window(WHOLE, BarSpan { from: 1.0, to: 2.0 }, 0.0, 1.0),
            (1.0, 0.0)
        );
    }

    #[test]
    fn the_wheel_opens_the_window_out_and_closes_it_back_in() {
        assert!(zoom_bars(12.0, 1.0, 30.0) > 12.0);
        assert!(zoom_bars(12.0, -1.0, 30.0) < 12.0);
        assert_eq!(zoom_bars(28.0, 1.0, 30.0), 30.0);
        assert_eq!(zoom_bars(1.0, -1.0, 30.0), 1.0);
        assert!(zoom_bars(1.0, 1.0, 30.0) > 1.0);
    }

    #[test]
    fn the_anchor_keeps_the_moment_under_the_cursor_where_it_is() {
        let w = bar_window(WHOLE, 12.0, 120.0, 4.0, 10.0);
        let start = anchor_bars(w.view_start, w.view_end, 0.5, 6.0, w.bar_sec, 0.0);
        assert!((start - 13.0).abs() < 1e-9);
        assert_eq!(view(WHOLE, 6.0, start), (26.0, 38.0));
    }

    #[test]
    fn the_anchor_counts_from_the_intervals_from_not_the_files() {
        let span = BarSpan { from: 10.0, to: 30.0 };
        let w = bar_window(span, 3.0, 120.0, 4.0, 2.0);
        let start = anchor_bars(w.view_start, w.view_end, 0.5, 1.0, w.bar_sec, 10.0);
        assert!((start - 3.0).abs() < 1e-9);
        assert_eq!(view(span, 1.0, start), (16.0, 18.0));
    }

    #[test]
    fn the_anchor_at_the_left_edge_is_the_start_and_an_out_of_range_start_is_the_windows_to_clamp() {
        let w = bar_window(WHOLE, 12.0, 120.0, 4.0, 10.0);
        assert!((anchor_bars(w.view_start, w.view_end, 0.0, 6.0, w.bar_sec, 0.0) - 10.0).abs() < 1e-9);
        let w = bar_window(WHOLE, 6.0, 120.0, 4.0, 0.0);
        let start = anchor_bars(w.view_start, w.view_end, 1.0, 30.0, w.bar_sec, 0.0);
        assert!(start < 0.0);
        assert_eq!(view(WHOLE, 30.0, start).0, 0.0);
    }
}
