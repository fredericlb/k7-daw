// typed_editors.rs — the clip editors' own typed calls: where a pointer is on the piano roll, how far a note drag has gone, the roll's scale, a bar's seconds.
// The convention is typed.rs's: numbers in, a number back, a few in the scalars.

use crate::abi::set_scalars;
use crate::ops::OK;
use crate::typed::flag;
use gesture::bar_window::bar_sec;
use gesture::roll_drag::{RollMode, roll_drag_delta, roll_point};
use gesture::roll_fit::roll_scale;
use gesture::snap::SnapMode;

/// Scalars: the beat, the key. `dx` and `dy` are the pointer's distance from the lane's top left corner.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_point(dx: f64, dy: f64, origin: f64, ppb: f64, canvas_beats: f64, row_h: f64) -> u32 {
    let (beat, key) = roll_point(dx, dy, origin, ppb, canvas_beats, row_h);
    set_scalars(&[beat, key]);
    OK
}

/// Scalars: beats, keys. `dx` and `dy` are the pointer's distance from the press.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_drag_delta(
    mode: f64,
    time: f64,
    dur: f64,
    dx: f64,
    dy: f64,
    ppb: f64,
    row_h: f64,
    step: f64,
    snap: f64,
    moves_keys: f64,
) -> u32 {
    let (beats, keys) = roll_drag_delta(
        RollMode::from_code(mode),
        time,
        dur,
        dx,
        dy,
        ppb,
        row_h,
        step,
        SnapMode::from_code(snap),
        flag(moves_keys),
    );
    set_scalars(&[beats, keys]);
    OK
}

/// Scalars: the row height, the pixels per beat, the lane width.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_scale(
    avail_w: f64,
    avail_h: f64,
    rows: f64,
    beats: f64,
    canvas_beats: f64,
    zoom_x: f64,
    px_per_beat: f64,
) -> u32 {
    let (row_h, ppb, lane_w) = roll_scale(avail_w, avail_h, rows, beats, canvas_beats, zoom_x, px_per_beat);
    set_scalars(&[row_h, ppb, lane_w]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_bar_sec(tempo: f64, beats_per_bar: f64) -> f64 {
    bar_sec(tempo, beats_per_bar)
}
