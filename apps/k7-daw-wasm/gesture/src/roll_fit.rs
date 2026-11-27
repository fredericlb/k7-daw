// roll_fit.rs — what the piano roll OPENS on and how far its canvas runs.
// A clip that states a window is FRAMED on it with a margin, and the canvas still runs over the whole sequence: a window is judged against what lies outside it.

use crate::js;

/// Breathing room around the fit, as a fraction of the content span.
const MARGIN: f64 = 0.2;

/// A note as the fit reads it.
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub time: f64,
    pub duration: f64,
    pub key: f64,
}

/// The window a clip's loop states, on the roll's own axis.
#[derive(Clone, Copy, Debug)]
pub struct Loop {
    pub from: f64,
    pub to: f64,
    pub win: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollFit {
    /// The pitch range the view frames, clamped to 0..127, and its row count.
    pub lo_key: f64,
    pub hi_key: f64,
    pub rows: f64,
    /// The beat range it frames: the window when the clip states one, else the content and a margin.
    pub lo_beat: f64,
    pub hi_beat: f64,
    /// `hi_beat - lo_beat`, floored at a bar when unwindowed.
    pub beats: f64,
    /// The clip's whole extent on the timeline: the playhead's bound, NOT what the canvas reaches.
    pub placed_end: f64,
    /// How far the canvas runs past `origin`.
    pub canvas_beats: f64,
    /// The beat drawn at the canvas's left edge.
    pub origin: f64,
}

fn end_of(notes: &[Note]) -> f64 {
    notes.iter().fold(0.0, |mx, n| js::max(mx, n.time + n.duration))
}

/// The view for one clip: FRAMED on the window with a margin when there is one, else on what is USED.
pub fn roll_fit(notes: &[Note], clip_length: f64, bar_beats: f64, lp: Option<Loop>) -> RollFit {
    let bar = js::max(1.0, bar_beats);
    let placed_end = if clip_length > 0.0 {
        clip_length
    } else {
        js::max(bar, end_of(notes))
    };

    // The notes the view is fitted to: those the window contains, or all of them without one.
    let fit: Vec<&Note> = match lp {
        Some(l) => notes
            .iter()
            .filter(|n| n.time < l.to && n.time + n.duration > l.from)
            .collect(),
        None => notes.iter().collect(),
    };

    // An empty range frames C3..C5, which is where a hand goes looking for middle C.
    let (lo_note, hi_note) = if fit.is_empty() {
        (48.0, 72.0)
    } else {
        (
            fit.iter().fold(f64::INFINITY, |m, n| js::min(m, n.key)),
            fit.iter().fold(f64::NEG_INFINITY, |m, n| js::max(m, n.key)),
        )
    };
    let p_margin = js::max(2.0, js::round(MARGIN * js::max(1.0, hi_note - lo_note + 1.0)));
    let lo_key = js::max(0.0, lo_note - p_margin);
    let hi_key = js::min(127.0, hi_note + p_margin);
    let rows = js::max(1.0, hi_key - lo_key + 1.0);
    let seq_end = end_of(notes);

    if let Some(l) = lp {
        // The margin is a fraction of the WINDOW: around a two-beat window you want a beat, not a fifth of a long file.
        let t_margin = js::max(bar / 2.0, MARGIN * l.win);
        return RollFit {
            lo_key,
            hi_key,
            rows,
            lo_beat: js::max(0.0, l.from - t_margin),
            hi_beat: l.to + t_margin,
            beats: js::max(bar, l.to - l.from + 2.0 * t_margin),
            placed_end,
            // Everywhere the wheel may reach, never less than the window, rounded up to a bar so the canvas ends on a drawn line.
            canvas_beats: (js::max(js::max(seq_end, l.to), bar) / bar).ceil() * bar,
            origin: 0.0,
        };
    }

    let seq_start = if notes.is_empty() {
        0.0
    } else {
        notes.iter().fold(f64::INFINITY, |m, n| js::min(m, n.time))
    };
    let content_lo = js::max(0.0, seq_start);
    let content_hi = js::max(bar, seq_end);
    let t_margin = MARGIN * js::max(bar, content_hi - content_lo);
    let lo_beat = js::max(0.0, content_lo - t_margin);
    let hi_beat = content_hi + t_margin;
    RollFit {
        lo_key,
        hi_key,
        rows,
        lo_beat,
        hi_beat,
        beats: js::max(bar, hi_beat - lo_beat),
        placed_end,
        // Bounded by what the clip IS: eight bars past the end would put a four-beat clip on a forty-beat canvas.
        canvas_beats: (js::max(js::max(seq_end, placed_end), bar) / bar).ceil() * bar,
        origin: 0.0,
    }
}

/// The roll's scale as (row height, pixels per beat, lane width): the framed rows fill the height, the framed beats the width, and zoom stretches the time axis.
pub fn roll_scale(
    avail_w: f64,
    avail_h: f64,
    rows: f64,
    beats: f64,
    canvas_beats: f64,
    zoom_x: f64,
    px_per_beat: f64,
) -> (f64, f64, f64) {
    let row_h = if avail_h > 0.0 { avail_h / rows } else { 12.0 };
    let ppb0 = if avail_w > 0.0 { avail_w / beats } else { px_per_beat };
    // The canvas always covers the editor's width: zooming out stops where the whole canvas fills it.
    let ppb_fill = if avail_w > 0.0 && canvas_beats > 0.0 {
        avail_w / canvas_beats
    } else {
        0.0
    };
    let ppb = js::max(ppb0 * zoom_x, ppb_fill);
    (row_h, ppb, canvas_beats * ppb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time: f64, duration: f64, key: f64) -> Note {
        Note { time, duration, key }
    }
    /// The fit for a clip the way the roll asks, the loop as `clipLoop` states it for a MIDI clip.
    fn fit(notes: &[Note], length: f64, win: Option<(f64, f64)>) -> RollFit {
        let lp = win.map(|(from, to)| Loop {
            from,
            to,
            win: to - from,
        });
        roll_fit(notes, length, 4.0, lp)
    }
    fn close(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "{a} is not {b}");
    }

    #[test]
    fn frames_the_window_with_a_margin_and_lets_the_canvas_span_the_sequence() {
        let f = fit(
            &[note(0.0, 0.5, 36.0), note(7.0, 0.135056, 38.0)],
            80.0,
            Some((0.0, 8.0)),
        );
        assert_eq!(f.origin, 0.0);
        assert_eq!(f.lo_beat, 0.0);
        assert_eq!(f.hi_beat, 10.0);
        assert_eq!(f.canvas_beats, 8.0);
    }

    #[test]
    fn still_knows_the_placed_extent() {
        let f = fit(&[note(0.0, 1.0, 60.0)], 80.0, Some((0.0, 8.0)));
        assert_eq!(f.placed_end, 80.0);
        assert_eq!(f.canvas_beats, 8.0);
    }

    #[test]
    fn keeps_the_origin_at_the_sequences_zero() {
        let f = fit(&[note(8.0, 1.0, 62.0), note(11.0, 1.0, 64.0)], 4.0, Some((8.0, 12.0)));
        assert_eq!((f.origin, f.lo_beat, f.hi_beat, f.canvas_beats), (0.0, 6.0, 14.0, 12.0));
    }

    #[test]
    fn a_note_before_the_window_does_not_stretch_the_pitch_fit() {
        let f = fit(
            &[note(0.0, 1.0, 20.0), note(8.0, 1.0, 62.0), note(11.0, 1.0, 64.0)],
            4.0,
            Some((8.0, 12.0)),
        );
        assert_eq!(f.lo_beat, 6.0);
        assert!(f.lo_key > 20.0);
    }

    #[test]
    fn a_long_window_gets_a_proportional_margin_and_a_short_one_half_a_bar() {
        close(
            fit(&[note(0.0, 1.0, 60.0)], 16.0, Some((0.0, 16.0))).hi_beat,
            19.2,
            1e-6,
        );
        let f = fit(&[note(4.0, 1.0, 60.0)], 2.0, Some((4.0, 5.0)));
        assert_eq!((f.lo_beat, f.hi_beat), (2.0, 7.0));
    }

    #[test]
    fn a_note_over_the_windows_edge_counts_as_inside_for_the_pitch_fit() {
        let f = fit(&[note(7.0, 2.0, 100.0), note(9.0, 1.0, 60.0)], 4.0, Some((8.0, 12.0)));
        assert!(f.hi_key >= 100.0);
    }

    #[test]
    fn frames_an_unlooped_clip_on_its_content_with_a_margin() {
        let f = fit(&[note(0.0, 1.0, 60.0), note(18.96, 1.0, 60.0)], 20.0, None);
        close((19.96 - f.lo_beat) / f.beats, 1.0 / 1.2, 0.005);
        assert_eq!(f.origin, 0.0);
    }

    #[test]
    fn the_canvas_reaches_the_placed_extent_in_whole_bars() {
        let f = fit(&[note(0.0, 1.0, 60.0)], 80.0, None);
        assert_eq!(f.placed_end, 80.0);
        assert_eq!(f.canvas_beats, 80.0);
    }

    #[test]
    fn the_canvas_stops_at_the_clip_with_nothing_beyond_to_write_in() {
        let f = fit(
            &[note(3.013803, 0.479722, 47.0), note(3.485962, 0.514038, 46.0)],
            4.0,
            None,
        );
        assert_eq!(f.canvas_beats, 4.0);
    }

    #[test]
    fn an_empty_sequence_gets_a_bar_to_look_at() {
        let f = fit(&[], 0.0, None);
        assert!(f.beats >= 4.0);
        assert_eq!(f.lo_beat, 0.0);
    }

    #[test]
    fn a_sequence_file_is_framed_on_its_notes() {
        let f = fit(&[note(0.0, 1.0, 60.0), note(6.0, 2.0, 60.0)], 0.0, None);
        close(f.hi_beat, 8.0 * 1.2, 1e-5);
        assert_eq!(f.placed_end, 8.0);
    }

    #[test]
    fn the_pitch_axis_frames_the_used_range_with_a_margin() {
        let f = fit(&[note(0.0, 1.0, 60.0), note(1.0, 1.0, 64.0)], 0.0, None);
        assert!(f.lo_key < 60.0 && f.hi_key > 64.0);
        assert_eq!(f.rows, f.hi_key - f.lo_key + 1.0);
    }

    #[test]
    fn the_pitch_axis_opens_around_middle_c_and_clamps_to_the_keys_that_exist() {
        let f = fit(&[], 0.0, None);
        assert!(f.lo_key <= 48.0 && f.hi_key >= 72.0);
        let f = fit(&[note(0.0, 1.0, 0.0), note(1.0, 1.0, 127.0)], 0.0, None);
        assert_eq!((f.lo_key, f.hi_key), (0.0, 127.0));
    }

    #[test]
    fn the_scale_fills_the_pane_with_the_framed_span_and_stops_zooming_out_at_the_canvas() {
        // 400 px wide framed on 8 beats is 50 a beat; 240 px tall on 24 rows is 10 a row.
        assert_eq!(
            roll_scale(400.0, 240.0, 24.0, 8.0, 16.0, 1.0, 40.0),
            (10.0, 50.0, 800.0)
        );
        // Zoomed out to a quarter the scale is held where the 16-beat canvas fills the 400 px: 25 a beat.
        assert_eq!(
            roll_scale(400.0, 240.0, 24.0, 8.0, 16.0, 0.25, 40.0),
            (10.0, 25.0, 400.0)
        );
    }

    #[test]
    fn an_unmeasured_pane_falls_back_to_the_default_scale() {
        assert_eq!(roll_scale(0.0, 0.0, 24.0, 8.0, 16.0, 1.0, 40.0), (12.0, 40.0, 640.0));
    }
}
