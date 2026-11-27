// typed_ticks.rs — the rulers' and the panes' typed calls: the tick ladder, the labels' spacing, the wheel and the anchored zoom.
// The convention is typed.rs's: numbers in, a number back, a few in the scalars, a list in the f64 output, a text in the output bytes.

use crate::abi::{set_f64_out, set_scalars, set_text, with_f64_in};
use crate::ops::OK;
use crate::typed::{flag, len, num, opt, yes};
use gesture::roll_drag::MIN_NOTE_BEATS;
use gesture::ruler::{RulerMode, ruler_scale};
use gesture::ticks::{self, adjacent_tick, nearest_tick};
use gesture::video_drag::V_NATURAL;
use gesture::{edge_drag, format_at, pan_zoom};

// ticks

/// The input holds the `n` candidates; a NaN `min_px` is the default distance.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_choose_tick(n: f64, px_per_unit: f64, min_px: f64) -> f64 {
    with_f64_in(len(n), |c| {
        ticks::choose_tick(c, px_per_unit, opt(min_px).unwrap_or(ticks::DEFAULT_MIN_PX))
    })
}

/// The ladder in the f64 output; its length. A NaN `fps` is none.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_second_ticks(fps: f64) -> u32 {
    set_f64_out(ticks::second_ticks(opt(fps))) as u32
}

/// The ticks in the f64 output; their count.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_ticks_up_to(end: f64, step: f64) -> u32 {
    set_f64_out(ticks::ticks_up_to(end, step)) as u32
}

/// The input holds the `n` ticks; NaN for none.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_adjacent_tick(n: f64, value: f64, dir: f64) -> f64 {
    with_f64_in(len(n), |t| num(adjacent_tick(t, value, dir)))
}

/// The input holds the `n` ticks; NaN for none.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_nearest_tick(n: f64, value: f64, tolerance: f64) -> f64 {
    with_f64_in(len(n), |t| num(nearest_tick(t, value, tolerance)))
}

/// A NaN `char_px` is the ruler's monospace.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_label_min_px(chars: f64, char_px: f64) -> f64 {
    ticks::label_min_px(chars, opt(char_px).unwrap_or(ticks::MONO_CHAR_PX))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_label_stride(spacing_px: f64, min_px: f64) -> f64 {
    ticks::label_stride(spacing_px, opt(min_px).unwrap_or(ticks::MIN_LABEL_PX))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_labelled(index: f64, stride: f64) -> f64 {
    yes(ticks::labelled(index, stride))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_subdivide_beats(bar_beats: f64, ppb: f64, max_px: f64) -> f64 {
    ticks::subdivide_beats(bar_beats, ppb, opt(max_px).unwrap_or(ticks::MAX_COLUMN_PX))
}

/// The input holds the `n` times; the extra times in the f64 output, and their count.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_subdivide_grid(n: f64, px_per_sec: f64, max_px: f64) -> u32 {
    let out = with_f64_in(len(n), |t| {
        ticks::subdivide_grid(t, px_per_sec, opt(max_px).unwrap_or(ticks::MAX_COLUMN_PX))
    });
    set_f64_out(out) as u32
}

/// The text in the output bytes; its length.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_format_clock(sec: f64, step: f64) -> u32 {
    set_text(&ticks::format_clock(sec, step)) as u32
}

/// The text in the output bytes; its length. A NaN `fps` is none.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_format_second_tick(sec: f64, step: f64, fps: f64) -> u32 {
    set_text(&ticks::format_second_tick(sec, step, opt(fps))) as u32
}

/// The text in the output bytes; its length.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_format_at(seconds: f64) -> u32 {
    set_text(&format_at::format_at(seconds)) as u32
}

/// Scalars: the unit in beats, the seconds step, the bar label stride. `mode` is 0 for bars; a NaN `fps` is none.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_ruler_scale(mode: f64, ppb: f64, bar_beats: f64, bps: f64, last_beat: f64, fps: f64) -> u32 {
    let mode = if mode == 0.0 {
        RulerMode::Bars
    } else {
        RulerMode::Seconds
    };
    let (unit, sec_step, stride) = ruler_scale(mode, ppb, bar_beats, bps, last_beat, opt(fps));
    set_scalars(&[unit, sec_step, stride]);
    OK
}

// the zoom of a pane

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_is_zoom_wheel(ctrl: f64, meta: f64) -> f64 {
    yes(pan_zoom::is_zoom_wheel(flag(ctrl), flag(meta)))
}

/// A NaN `wheel_delta_y` is an event without the legacy field.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_is_mouse_wheel(delta_x: f64, delta_y: f64, delta_mode: f64, wheel_delta_y: f64) -> f64 {
    yes(pan_zoom::is_mouse_wheel(
        delta_x,
        delta_y,
        delta_mode,
        opt(wheel_delta_y),
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_wheel_factor(delta: f64, pinch: f64) -> f64 {
    pan_zoom::wheel_factor(delta, flag(pinch))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_zoom_ceiling(width: f64) -> f64 {
    pan_zoom::zoom_ceiling(width)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_wheel_zoom_to(cur: f64, delta: f64, pinch: f64, min: f64, ceiling: f64) -> f64 {
    pan_zoom::wheel_zoom_to(cur, delta, flag(pinch), min, ceiling)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_pinch_zoom_to(start_zoom: f64, dist: f64, start_dist: f64, min: f64, ceiling: f64) -> f64 {
    pan_zoom::pinch_zoom_to(start_zoom, dist, start_dist, min, ceiling)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_anchored_scroll_left(scroll_left: f64, offset_x: f64, gutter: f64, cur: f64, next: f64) -> f64 {
    pan_zoom::anchored_scroll_left(scroll_left, offset_x, gutter, cur, next)
}

/// A constant the page reads once: 0 the widest column, 1 the finest grid split, 2 the label floor, 3 a mono
/// character, 4 a label gap, 5 the picture placeholder, 6 the widest lane, 7 the shortest note, 8 a crossfade, 9 the label distance.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_const(id: f64) -> f64 {
    match id as u32 {
        0 => ticks::MAX_COLUMN_PX,
        1 => ticks::MAX_GRID_SPLIT,
        2 => ticks::MIN_LABEL_PX,
        3 => ticks::MONO_CHAR_PX,
        4 => ticks::LABEL_GAP_PX,
        5 => V_NATURAL,
        6 => pan_zoom::MAX_LANE_PX,
        7 => MIN_NOTE_BEATS,
        8 => edge_drag::CROSSFADE_BEATS,
        9 => ticks::DEFAULT_MIN_PX,
        _ => f64::NAN,
    }
}
