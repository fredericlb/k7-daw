// ticks.rs — choosing a ruler's unit for the zoom it is drawn at, which gridlines get a LABEL, and the tick a click landed on.
// Unit-free: `candidates` is what a scale may subdivide to, bars halving and seconds walking a 1/2/5/10 ladder.

use crate::js;

/// The COARSEST candidate at least `min_px` apart: touching labels are unreadable, sparse ones are not.
pub fn choose_tick(candidates: &[f64], px_per_unit: f64, min_px: f64) -> f64 {
    for &c in candidates {
        if c * px_per_unit >= min_px {
            return c;
        }
    }
    candidates.last().copied().unwrap_or(1.0)
}

/// The default distance between two labels, in pixels.
pub const DEFAULT_MIN_PX: f64 = 56.0;

/// A wall-clock ladder in seconds, NOT powers of two: bars halve and time does not.
pub const SECOND_TICKS: [f64; 17] = [
    0.04, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0,
];

/// The frames a sub-second rung may span: a ladder of them puts every fine tick ON a frame.
const FRAME_RUNGS: [f64; 5] = [1.0, 2.0, 5.0, 10.0, 15.0];

/// The seconds ladder for a song that has a picture at `fps`: below a second it counts FRAMES, since a cut can only fall on one.
pub fn second_ticks(fps: Option<f64>) -> Vec<f64> {
    let Some(fps) = fps.filter(|f| *f > 0.0) else {
        return SECOND_TICKS.to_vec();
    };
    let mut out: Vec<f64> = FRAME_RUNGS
        .iter()
        .map(|n| n / fps)
        .filter(|s| *s < 1.0 - 1e-9)
        .collect();
    out.extend(SECOND_TICKS.iter().copied().filter(|s| *s >= 1.0));
    out
}

/// A seconds tick's label: `m:ss` on whole seconds, and between them the FRAME it is when the ladder counts frames.
pub fn format_second_tick(sec: f64, step: f64, fps: Option<f64>) -> String {
    let Some(fps) = fps.filter(|f| *f > 0.0).filter(|_| step < 1.0) else {
        return format_clock(sec, step);
    };
    let whole = (sec + 1e-9).floor();
    let frame = js::round((sec - whole) * fps);
    if frame == 0.0 {
        format_clock(whole, 1.0)
    } else {
        format!("{frame}f")
    }
}

/// `m:ss`, plus decimals only when the STEP needs them: a 30s ruler does not say "1:30.0".
pub fn format_clock(sec: f64, step: f64) -> String {
    let neg = sec < 0.0;
    // Two decimals below a tenth of a second, one below a second, none at or above it.
    let digits: usize = if step < 0.1 {
        2
    } else if step < 1.0 {
        1
    } else {
        0
    };
    let scale = 10f64.powi(digits as i32);
    // Rounded BEFORE the split, so 59.7 reads 1:00 and never 0:60.
    let s = js::round(sec.abs() * scale) / scale;
    let m = (s / 60.0).floor();
    let rest = s - m * 60.0;
    let width = if digits > 0 { 3 + digits } else { 2 };
    let ss = js::to_fixed(rest, digits);
    format!("{}{m}:{ss:0>width$}", if neg { "-" } else { "" })
}

/// Every tick of `step` from 0 up to and including `end`; empty for a non-positive step.
pub fn ticks_up_to(end: f64, step: f64) -> Vec<f64> {
    if step.is_nan() || step <= 0.0 || end.is_nan() || end < 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    // Counted, not accumulated: adding 0.1 four hundred times lands at 40.000000000000255.
    let mut i = 0.0;
    while i * step <= end + 1e-9 {
        out.push(i * step);
        i += 1.0;
    }
    out
}

/// The first tick strictly after (`dir` 1) or before (-1) `value` in an ascending list, or None past the end.
pub fn adjacent_tick(ticks: &[f64], value: f64, dir: f64) -> Option<f64> {
    // Strictly, by more than float noise, so a playhead already ON a tick moves off it.
    let eps = 1e-6;
    if dir > 0.0 {
        return ticks.iter().copied().find(|t| *t > value + eps);
    }
    ticks.iter().rev().copied().find(|t| *t < value - eps)
}

/// The tick nearest `value`, or None when the nearest is further than `tolerance` away.
pub fn nearest_tick(ticks: &[f64], value: f64, tolerance: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    let mut best_d = f64::INFINITY;
    for &t in ticks {
        let d = (t - value).abs();
        if d < best_d {
            best_d = d;
            best = Some(t);
        }
    }
    best.filter(|_| best_d <= tolerance)
}

/// The floor under any label spacing: what a single-character label needs.
pub const MIN_LABEL_PX: f64 = 10.0;

/// Advance of one character at the 12px monospace the timeline's ruler labels in.
pub const MONO_CHAR_PX: f64 = 7.2;

/// Clear space between one label's last character and the next label's first.
pub const LABEL_GAP_PX: f64 = 8.0;

/// How far apart two gridlines must be for a label of `chars` characters to fit.
pub fn label_min_px(chars: f64, char_px: f64) -> f64 {
    js::max(MIN_LABEL_PX, chars * char_px + LABEL_GAP_PX)
}

/// Label every Nth gridline, N a power of two, so labels sit at least `min_px` apart; 1 when they already fit.
pub fn label_stride(spacing_px: f64, min_px: f64) -> f64 {
    if !spacing_px.is_finite() || spacing_px <= 0.0 {
        return 1.0;
    }
    let mut stride = 1.0;
    // Doubling rather than a log, which lands off by one at a power-of-two boundary for exact values.
    while spacing_px * stride < min_px {
        stride *= 2.0;
    }
    stride
}

/// Whether the gridline at `index` keeps its label; indices count from the ruler's ORIGIN, so panning cannot re-thin it.
pub fn labelled(index: f64, stride: f64) -> bool {
    stride <= 1.0 || index % stride == 0.0
}

/// Above this, a column is wide enough to hold the next unit down inside it.
pub const MAX_COLUMN_PX: f64 = 50.0;

/// The unit a ruler should draw, in BEATS: a musical ladder rather than a binary one, floored at a 64th.
pub fn subdivide_beats(bar_beats: f64, ppb: f64, max_px: f64) -> f64 {
    if !ppb.is_finite() || ppb <= 0.0 || !bar_beats.is_finite() || bar_beats <= 0.0 {
        // `barBeats || 1` of the page: a NaN or a zero reads as one.
        let bar = if bar_beats.is_nan() || bar_beats == 0.0 {
            1.0
        } else {
            bar_beats
        };
        return js::max(1.0, bar);
    }
    if bar_beats * ppb <= max_px {
        return bar_beats;
    }
    let mut unit = 1.0;
    let floor = 1.0 / 64.0;
    while unit * ppb > max_px && unit > floor {
        unit /= 2.0;
    }
    unit
}

/// The finest split of one column this will make: a 16th of the interval it is handed.
pub const MAX_GRID_SPLIT: f64 = 16.0;

/// The EXTRA times between the ticks a ruler was handed, split per PAIR once its columns open up.
pub fn subdivide_grid(times: &[f64], px_per_sec: f64, max_px: f64) -> Vec<f64> {
    let mut out = Vec::new();
    if !px_per_sec.is_finite() || px_per_sec <= 0.0 {
        return out;
    }
    for pair in times.windows(2) {
        let (t0, t1) = (pair[0], pair[1]);
        let gap = (t1 - t0) * px_per_sec;
        if gap.is_nan() || gap <= max_px {
            continue;
        }
        let mut n = 2.0;
        while gap / n > max_px && n < MAX_GRID_SPLIT {
            n *= 2.0;
        }
        let mut j = 1.0;
        while j < n {
            out.push(t0 + ((t1 - t0) * j) / n);
            j += 1.0;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(sec: f64, step: f64) -> String {
        format_clock(sec, step)
    }

    #[test]
    fn choose_tick_takes_the_coarsest_step_that_still_clears_min_px() {
        assert_eq!(choose_tick(&SECOND_TICKS, 10.0, DEFAULT_MIN_PX), 10.0);
        assert_eq!(choose_tick(&SECOND_TICKS, 200.0, DEFAULT_MIN_PX), 0.5);
    }

    #[test]
    fn choose_tick_falls_back_to_the_last_rather_than_failing() {
        assert_eq!(choose_tick(&SECOND_TICKS, 0.0001, DEFAULT_MIN_PX), 3600.0);
        assert_eq!(choose_tick(&[], 10.0, DEFAULT_MIN_PX), 1.0);
    }

    #[test]
    fn ticks_up_to_counts_rather_than_accumulates_so_a_tenth_step_stays_on_the_grid() {
        let t = ticks_up_to(40.0, 0.1);
        assert_eq!(t.last().copied(), Some(40.0));
        assert_eq!(t.len(), 401);
        assert!(ticks_up_to(10.0, 0.0).is_empty());
    }

    #[test]
    fn format_clock_shows_only_the_decimals_the_step_needs() {
        assert_eq!(clock(90.0, 30.0), "1:30");
        assert_eq!(clock(90.5, 0.5), "1:30.5");
        assert_eq!(clock(90.25, 0.04), "1:30.25");
        assert_eq!(clock(5.0, 1.0), "0:05");
    }

    #[test]
    fn format_clock_carries_a_rounded_up_second_into_the_minute() {
        assert_eq!(clock(59.7, 1.0), "1:00");
        assert_eq!(clock(59.97, 0.5), "1:00.0");
        assert_eq!(clock(-59.7, 1.0), "-1:00");
    }

    #[test]
    fn nearest_tick_returns_none_when_nothing_is_within_tolerance() {
        assert_eq!(nearest_tick(&[0.0, 10.0, 20.0], 12.0, 1.0), None);
        assert_eq!(nearest_tick(&[0.0, 10.0, 20.0], 12.0, 3.0), Some(10.0));
    }

    #[test]
    fn second_ticks_with_a_picture_count_frames_below_a_second() {
        let ladder = second_ticks(Some(30.0));
        assert_eq!(
            ladder[..5],
            [1.0 / 30.0, 2.0 / 30.0, 5.0 / 30.0, 10.0 / 30.0, 15.0 / 30.0]
        );
        assert_eq!(ladder[5..7], [1.0, 2.0]);
        for t in ticks_up_to(2.0, ladder[0]) {
            assert!((t * 30.0 - js::round(t * 30.0)).abs() < 1e-9);
        }
    }

    #[test]
    fn second_ticks_keep_the_decimal_ladder_for_a_song_with_no_picture() {
        assert_eq!(second_ticks(None), SECOND_TICKS.to_vec());
        assert_eq!(second_ticks(Some(0.0)), SECOND_TICKS.to_vec());
    }

    #[test]
    fn a_frame_tick_is_labelled_by_its_frame_and_a_whole_second_as_a_clock() {
        assert_eq!(format_second_tick(1.0 + 12.0 / 30.0, 1.0 / 30.0, Some(30.0)), "12f");
        assert_eq!(format_second_tick(2.0, 1.0 / 30.0, Some(30.0)), "0:02");
        assert_eq!(format_second_tick(0.5, 0.5, None), clock(0.5, 0.5));
    }

    #[test]
    fn adjacent_tick_steps_off_the_tick_it_is_on_either_way_and_stops_at_the_ends() {
        let t = [0.0, 1.0, 2.0, 3.0];
        assert_eq!(adjacent_tick(&t, 1.0, 1.0), Some(2.0));
        assert_eq!(adjacent_tick(&t, 1.0, -1.0), Some(0.0));
        assert_eq!(adjacent_tick(&t, 1.4, 1.0), Some(2.0));
        assert_eq!(adjacent_tick(&t, 1.4, -1.0), Some(1.0));
        assert_eq!(adjacent_tick(&t, 3.0, 1.0), None);
        assert_eq!(adjacent_tick(&t, 0.0, -1.0), None);
    }

    fn min_px(label: &str) -> f64 {
        label_min_px(label.chars().count() as f64, MONO_CHAR_PX)
    }

    #[test]
    fn label_min_px_asks_for_the_labels_own_width_plus_a_gap_and_never_drops_below_the_floor() {
        assert!((min_px("129") - (3.0 * MONO_CHAR_PX + LABEL_GAP_PX)).abs() < 1e-9);
        assert!((min_px("12.4") - (4.0 * MONO_CHAR_PX + LABEL_GAP_PX)).abs() < 1e-9);
        assert!(min_px("129") > min_px("7"));
        assert!(min_px("129.4") > min_px("129"));
        assert_eq!(min_px(""), MIN_LABEL_PX);
    }

    #[test]
    fn label_stride_thins_by_powers_of_two() {
        assert_eq!(label_stride(20.0, 30.0), 2.0);
        assert_eq!(label_stride(20.0, 70.0), 4.0);
        assert_eq!(label_stride(20.0, 130.0), 8.0);
        assert_eq!(label_stride(40.0, 30.0), 1.0);
    }

    #[test]
    fn label_stride_gives_a_wide_label_more_room_and_thins_until_labels_clear() {
        assert_eq!(label_stride(16.0, min_px("7")), 1.0);
        assert!(label_stride(16.0, min_px("129")) > 1.0);
        for label in ["7", "42", "129", "1024", "129.4"] {
            for spacing in [3.0, 8.0, 12.0, 16.0, 24.0, 40.0] {
                assert!(spacing * label_stride(spacing, min_px(label)) >= min_px(label));
            }
        }
    }

    #[test]
    fn label_stride_yields_one_on_a_degenerate_zoom() {
        assert_eq!(label_stride(0.0, MIN_LABEL_PX), 1.0);
        assert_eq!(label_stride(f64::NAN, MIN_LABEL_PX), 1.0);
        assert_eq!(label_stride(-5.0, MIN_LABEL_PX), 1.0);
    }

    #[test]
    fn labelled_counts_from_the_origin() {
        assert!(labelled(0.0, 4.0));
        assert!(labelled(4.0, 4.0));
        assert!(!labelled(5.0, 4.0));
        assert!(labelled(7.0, 1.0));
    }

    #[test]
    fn subdivide_beats_draws_bars_while_a_bar_fits_then_descends_to_the_beat_and_halves_below_it() {
        assert_eq!(subdivide_beats(4.0, 10.0, MAX_COLUMN_PX), 4.0);
        assert_eq!(subdivide_beats(4.0, 20.0, MAX_COLUMN_PX), 1.0);
        assert_eq!(subdivide_beats(4.0, 60.0, MAX_COLUMN_PX), 0.5);
        assert!((subdivide_beats(4.0, 200.0, MAX_COLUMN_PX) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn subdivide_beats_floors_rather_than_spinning_on_a_degenerate_zoom() {
        assert_eq!(subdivide_beats(4.0, 0.0, MAX_COLUMN_PX), 4.0);
        assert_eq!(subdivide_beats(4.0, f64::NAN, MAX_COLUMN_PX), 4.0);
        assert!(subdivide_beats(4.0, 1e9, MAX_COLUMN_PX) >= 1.0 / 64.0);
    }

    #[test]
    fn subdivide_grid_adds_nothing_while_the_columns_fit_and_halves_one_that_opened_up() {
        assert!(subdivide_grid(&[0.0, 1.0, 2.0], 40.0, MAX_COLUMN_PX).is_empty());
        assert_eq!(subdivide_grid(&[0.0, 1.0, 2.0], 80.0, MAX_COLUMN_PX), vec![0.5, 1.5]);
        assert_eq!(subdivide_grid(&[0.0, 1.0], 200.0, MAX_COLUMN_PX), vec![0.25, 0.5, 0.75]);
    }

    #[test]
    fn subdivide_grid_splits_each_pair_on_its_own() {
        assert_eq!(subdivide_grid(&[0.0, 1.0, 3.0], 40.0, MAX_COLUMN_PX), vec![2.0]);
    }

    #[test]
    fn subdivide_grid_floors_refuses_a_degenerate_scale_and_needs_two_lines() {
        assert!(subdivide_grid(&[0.0, 1.0], 0.0, MAX_COLUMN_PX).is_empty());
        assert!(subdivide_grid(&[0.0, 1.0], f64::NAN, MAX_COLUMN_PX).is_empty());
        assert_eq!(
            subdivide_grid(&[0.0, 1.0], 1e9, MAX_COLUMN_PX).len(),
            MAX_GRID_SPLIT as usize - 1
        );
        assert!(subdivide_grid(&[], 100.0, MAX_COLUMN_PX).is_empty());
        assert!(subdivide_grid(&[4.0], 100.0, MAX_COLUMN_PX).is_empty());
    }

    #[test]
    fn nearest_tick_with_no_tolerance_takes_the_closest_line_either_side() {
        let ticks = [0.0, 0.5, 1.0, 1.5, 2.0];
        let near = |v: f64| nearest_tick(&ticks, v, f64::INFINITY);
        assert_eq!(near(1.1), Some(1.0));
        assert_eq!(near(1.4), Some(1.5));
        assert_eq!(near(1.5), Some(1.5));
        assert_eq!(near(-1.0), Some(0.0));
        assert_eq!(near(9.0), Some(2.0));
        assert_eq!(near(0.25), Some(0.0));
        assert_eq!(nearest_tick(&[], 1.2, f64::INFINITY), None);
        assert_eq!(nearest_tick(&[2.0, 0.0, 1.0], 0.9, f64::INFINITY), Some(1.0));
    }
}
