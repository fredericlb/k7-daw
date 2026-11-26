// typed.rs — the gestures' hot calls: numbers in, numbers out, no JSON, on a pointer move (apps/k7-daw-frontend/packages/k7-wasm/docs/gesture.md).
// A reader would get wrong the answer's place: one number comes back as the return, a few in the scalars, a list in the f64 output, a text in the output bytes.

use crate::abi::{NONE, set_scalars, with_f64_in};
use crate::ops::OK;
use gesture::bar_window::{BarSpan, FRAME_MARGIN_BARS, anchor_bars, bar_window, frame_window, zoom_bars};
use gesture::clip_playhead::{ActWindow, clip_playhead};
use gesture::grid_caret::{Bounds, Cell, cell_box, clamp_cell, col_span, move_cell, sel_box, song_seconds_of_col};
use gesture::roll_drag::{
    GroupNote, RollDrag, RollMode, clamp_key, drag_moved, draw_span, group_offset, key_offset, moves_pitch, t_span,
    vel_at,
};
use gesture::roll_fit::{Loop, Note, roll_fit};
use gesture::snap::{SnapMode, drag_offset, free_snap, move_offset, snap_mode, snap_position, snap_to};
use gesture::take_counter::{Phase, bar_beat, recording_from, take_counter};
use gesture::video_drag::{Shot, VideoMode, VideoSnap, video_drag_edits, video_drag_is_still, video_drag_move};

/// A number a caller may leave out is sent as NaN.
pub(crate) fn opt(v: f64) -> Option<f64> {
    if v.is_nan() { None } else { Some(v) }
}

pub(crate) fn num(v: Option<f64>) -> f64 {
    v.unwrap_or(f64::NAN)
}

pub(crate) fn flag(v: f64) -> bool {
    v != 0.0
}

pub(crate) fn yes(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// The count of numbers in the input buffer a caller said it wrote.
pub(crate) fn len(n: f64) -> usize {
    if n.is_finite() && n > 0.0 { n as usize } else { 0 }
}

fn roll(mode: f64, key: f64, time: f64, dur: f64, d_beats: f64, d_keys: f64) -> RollDrag {
    RollDrag {
        key,
        time,
        dur,
        mode: RollMode::from_code(mode),
        d_beats,
        d_keys,
    }
}

// snap

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_snap_to(v: f64, step: f64) -> f64 {
    snap_to(v, step)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_free_snap(ctrl: f64, meta: f64) -> f64 {
    yes(free_snap(flag(ctrl), flag(meta)))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_snap_mode(shift: f64, ctrl: f64, meta: f64) -> f64 {
    snap_mode(flag(shift), flag(ctrl), flag(meta)).code()
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_drag_offset(dx: f64, ppb: f64, step: f64, orig_start: f64) -> f64 {
    drag_offset(dx, ppb, step, orig_start)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_snap_position(orig: f64, delta: f64, step: f64, mode: f64) -> f64 {
    snap_position(orig, delta, step, SnapMode::from_code(mode))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_move_offset(dx: f64, ppb: f64, step: f64, orig_start: f64, mode: f64) -> f64 {
    move_offset(dx, ppb, step, orig_start, SnapMode::from_code(mode))
}

// the piano roll's drags

/// Scalars: key, start, end.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_span(mode: f64, key: f64, time: f64, dur: f64, d_beats: f64, d_keys: f64) -> u32 {
    let s = t_span(&roll(mode, key, time, dur, d_beats, d_keys));
    set_scalars(&[s.key, s.start, s.end]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_moves_pitch(mode: f64, key: f64, time: f64, dur: f64, d_beats: f64, d_keys: f64) -> f64 {
    yes(moves_pitch(&roll(mode, key, time, dur, d_beats, d_keys)))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_drag_moved(mode: f64, key: f64, time: f64, dur: f64, d_beats: f64, d_keys: f64) -> f64 {
    yes(drag_moved(&roll(mode, key, time, dur, d_beats, d_keys)))
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_clamp_key(k: f64) -> f64 {
    clamp_key(k)
}

/// Scalars: from, to.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_draw_span(a: f64, b: f64, step: f64, origin: f64, free_len: f64) -> u32 {
    let (from, to) = draw_span(a, b, step, origin, free_len);
    set_scalars(&[from, to]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_key_offset(dy: f64, row_h: f64) -> f64 {
    key_offset(dy, row_h)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_vel_at(y_in_row: f64, row_h: f64) -> f64 {
    vel_at(y_in_row, row_h)
}

/// The input holds `n` notes as key, time, duration; scalars: beats, keys.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_group_offset(n: f64, d_beats: f64, d_keys: f64) -> u32 {
    let n = len(n);
    let (b, k) = with_f64_in(n * 3, |v| {
        let group: Vec<GroupNote> = v
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| GroupNote {
                key: c[0],
                time: c[1],
                duration: c[2],
            })
            .collect();
        group_offset(&group, d_beats, d_keys)
    });
    set_scalars(&[b, k]);
    OK
}

/// The input holds `n` notes as time, duration, key; scalars: the fit's nine numbers in the order of its fields.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_roll_fit(
    n: f64,
    clip_length: f64,
    bar_beats: f64,
    loop_from: f64,
    loop_to: f64,
    loop_win: f64,
) -> u32 {
    let n = len(n);
    let lp = opt(loop_from).map(|from| Loop {
        from,
        to: loop_to,
        win: loop_win,
    });
    let f = with_f64_in(n * 3, |v| {
        let notes: Vec<Note> = v
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| Note {
                time: c[0],
                duration: c[1],
                key: c[2],
            })
            .collect();
        roll_fit(&notes, clip_length, bar_beats, lp)
    });
    set_scalars(&[
        f.lo_key,
        f.hi_key,
        f.rows,
        f.lo_beat,
        f.hi_beat,
        f.beats,
        f.placed_end,
        f.canvas_beats,
        f.origin,
    ]);
    OK
}

// the grid's caret and rectangle

/// Scalars: row, col.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_clamp_cell(row: f64, col: f64, rows: f64, cols: f64) -> u32 {
    let c = clamp_cell(Cell { row, col }, Bounds { rows, cols });
    set_scalars(&[c.row, c.col]);
    OK
}

/// Scalars: row, col.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_move_cell(row: f64, col: f64, d_row: f64, d_col: f64, rows: f64, cols: f64) -> u32 {
    let c = move_cell(Cell { row, col }, d_row, d_col, Bounds { rows, cols });
    set_scalars(&[c.row, c.col]);
    OK
}

/// Scalars: row low, row high, col low, col high.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_sel_box(anchor_row: f64, anchor_col: f64, head_row: f64, head_col: f64) -> u32 {
    let b = sel_box(
        Cell {
            row: anchor_row,
            col: anchor_col,
        },
        Cell {
            row: head_row,
            col: head_col,
        },
    );
    set_scalars(&[b.row_lo, b.row_hi, b.col_lo, b.col_hi]);
    OK
}

/// Scalars: row low, row high, col low, col high.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_cell_box(row: f64, col: f64) -> u32 {
    let b = cell_box(Cell { row, col });
    set_scalars(&[b.row_lo, b.row_hi, b.col_lo, b.col_hi]);
    OK
}

/// Scalars: from, to.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_col_span(anchor_row: f64, anchor_col: f64, head_row: f64, head_col: f64) -> u32 {
    let (from, to) = col_span(
        Cell {
            row: anchor_row,
            col: anchor_col,
        },
        Cell {
            row: head_row,
            col: head_col,
        },
    );
    set_scalars(&[from, to]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_song_seconds_of_col(col: f64, unit: f64, clip_start: f64, tempo: f64) -> f64 {
    song_seconds_of_col(col, unit, clip_start, tempo)
}

// the clip panel's window

/// Scalars: bars shown, bars in all, seconds a bar, view start, view end.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_bar_window(
    from: f64,
    to: f64,
    want: f64,
    tempo: f64,
    beats_per_bar: f64,
    start_bar: f64,
) -> u32 {
    let w = bar_window(BarSpan { from, to }, want, tempo, beats_per_bar, start_bar);
    set_scalars(&[w.shown, w.all_bars, w.bar_sec, w.view_start, w.view_end]);
    OK
}

/// Scalars: bars, start bar; a NaN margin is the default one.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_frame_window(
    reach_from: f64,
    reach_to: f64,
    act_from: f64,
    act_to: f64,
    bar_sec: f64,
    margin: f64,
) -> u32 {
    let (bars, start) = frame_window(
        BarSpan {
            from: reach_from,
            to: reach_to,
        },
        BarSpan {
            from: act_from,
            to: act_to,
        },
        bar_sec,
        opt(margin).unwrap_or(FRAME_MARGIN_BARS),
    );
    set_scalars(&[bars, start]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_zoom_bars(bars: f64, delta_y: f64, all_bars: f64) -> f64 {
    zoom_bars(bars, delta_y, all_bars)
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_anchor_bars(
    view_start: f64,
    view_end: f64,
    frac: f64,
    next_bars: f64,
    bar_sec: f64,
    span_from: f64,
) -> f64 {
    anchor_bars(view_start, view_end, frac, next_bars, bar_sec, span_from)
}

/// NaN for a transport outside the event.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_clip_playhead(song_seconds: f64, placed_at: f64, act_from: f64, act_to: f64, tempo: f64) -> f64 {
    num(clip_playhead(
        song_seconds,
        opt(placed_at),
        ActWindow {
            from: act_from,
            to: act_to,
        },
        tempo,
    ))
}

// the take's counter

/// Scalars: the phase (0 count-in, 1 recording), the beats.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_take_counter(start_beat: f64, cur_beat: f64, count_in_beats: f64) -> u32 {
    let (phase, beats) = take_counter(start_beat, cur_beat, count_in_beats);
    set_scalars(&[if phase == Phase::CountIn { 0.0 } else { 1.0 }, beats]);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_recording_from(start_beat: f64, count_in_beats: f64) -> f64 {
    recording_from(start_beat, count_in_beats)
}

/// Scalars: bar, beat.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_bar_beat(recorded_beats: f64, beats_per_bar: f64) -> u32 {
    let (bar, beat) = bar_beat(recorded_beats, beats_per_bar);
    set_scalars(&[bar, beat]);
    OK
}

// a picture drag

fn shot(mode: f64, orig_start: f64, orig_length: f64, orig_from: f64, speed: f64, start: f64, length: f64) -> Shot {
    Shot {
        mode: VideoMode::from_code(mode),
        orig_start,
        orig_length,
        orig_from,
        speed: opt(speed),
        start,
        length,
    }
}

/// The input holds the shot as mode, origStart, origLength, origFrom, speed (NaN for 1), start, length, then the
/// pointer's dx, the px per second, the snap mode, the ruler's unit, the beats per second and the drawn ticks; scalars: start, length.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_video_drag(n: f64) -> u32 {
    const HEAD: usize = 12;
    let n = len(n);
    let answer = with_f64_in(n, |v| {
        if v.len() < HEAD {
            return None;
        }
        let s = shot(v[0], v[1], v[2], v[3], v[4], v[5], v[6]);
        let snap = VideoSnap {
            mode: SnapMode::from_code(v[9]),
            unit: v[10],
            bps: v[11],
            ticks: &v[HEAD..],
        };
        Some(video_drag_move(&s, v[7], v[8], &snap))
    });
    match answer {
        Some((start, length)) => {
            set_scalars(&[start, length]);
            OK
        }
        None => NONE,
    }
}

/// Scalars: how many fields, then a field code (0 start, 1 from, 2 length) and its value for each.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_video_edits(
    mode: f64,
    orig_start: f64,
    orig_from: f64,
    speed: f64,
    start: f64,
    length: f64,
    activation: f64,
) -> u32 {
    let s = shot(mode, orig_start, 0.0, orig_from, speed, start, length);
    let edits = video_drag_edits(&s, flag(activation));
    let mut out = vec![edits.len() as f64];
    for (field, value) in edits {
        out.push(field.code());
        out.push(value);
    }
    set_scalars(&out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_g_video_is_still(mode: f64, orig_start: f64, orig_length: f64, start: f64, length: f64) -> f64 {
    yes(video_drag_is_still(&shot(
        mode,
        orig_start,
        orig_length,
        0.0,
        f64::NAN,
        start,
        length,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{k7_f64_in, k7_scalars_ptr};
    use crate::typed_ticks::k7_g_const;

    fn scalars(n: usize) -> Vec<f64> {
        // SAFETY: the scalars are a fixed 16-number buffer that outlives the call.
        unsafe { std::slice::from_raw_parts(k7_scalars_ptr(), n).to_vec() }
    }

    fn write(values: &[f64]) {
        let at = k7_f64_in(values.len());
        // SAFETY: `k7_f64_in` just sized the buffer to `values.len()` numbers.
        unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), at, values.len()) };
    }

    #[test]
    fn a_single_number_comes_back_as_the_return() {
        assert_eq!(k7_g_snap_to(1.6, 1.0), 2.0);
        assert_eq!(k7_g_snap_mode(0.0, 1.0, 0.0), 1.0);
        assert!(k7_g_clip_playhead(0.0, f64::NAN, 0.0, 1.0, 120.0).is_nan());
    }

    #[test]
    fn a_few_numbers_come_back_in_the_scalars() {
        assert_eq!(k7_g_roll_span(0.0, 60.0, 4.0, 1.0, 2.0, 3.0), OK);
        assert_eq!(scalars(3), vec![63.0, 6.0, 7.0]);
        assert_eq!(k7_g_take_counter(0.0, 1.0, 4.0), OK);
        assert_eq!(scalars(2), vec![0.0, 3.0]);
    }

    #[test]
    fn an_array_goes_in_the_input_and_the_notes_are_read_from_it() {
        write(&[60.0, 1.0, 1.0, 67.0, 2.0, 0.5]);
        assert_eq!(k7_g_group_offset(2.0, -5.0, 70.0), OK);
        assert_eq!(scalars(2), vec![-1.0, 60.0]);
        write(&[0.0, 1.0, 60.0]);
        assert_eq!(k7_g_roll_fit(1.0, 4.0, 4.0, f64::NAN, 0.0, 0.0), OK);
        assert_eq!(scalars(9)[7], 4.0);
    }

    #[test]
    fn a_picture_drag_reads_its_block_and_snaps_to_the_ticks_in_it() {
        let mut block = vec![0.0, 4.0, 6.0, 2.0, f64::NAN, 4.0, 6.0, 130.0, 100.0, 0.0, 1.0, 2.0];
        block.extend((0..=16).map(|i| f64::from(i) * 0.5));
        write(&block);
        assert_eq!(k7_g_video_drag(block.len() as f64), OK);
        assert_eq!(scalars(2), vec![4.0, 6.0]);
        assert_eq!(k7_g_video_drag(3.0), NONE);
    }

    #[test]
    fn the_constants_answer_by_id_and_nan_past_the_list() {
        assert_eq!(k7_g_const(0.0), 50.0);
        assert!(k7_g_const(99.0).is_nan());
    }
}
