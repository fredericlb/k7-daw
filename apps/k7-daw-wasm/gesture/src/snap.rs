// snap.rs — the grid rule every drag follows, and the modifiers that change it.
// A reader would get wrong that a clip drag snaps its OFFSET, not its position: a hand-placed clip keeps its phase.

use crate::js;

/// How a MOVE or a RESIZE lands: on the project's grid, a whole number of steps from where it was, or free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapMode {
    Grid,
    Relative,
    Free,
}

impl SnapMode {
    /// The mode a typed call names: 0 grid, 1 relative, anything else free.
    pub fn from_code(code: f64) -> SnapMode {
        if code == 0.0 {
            SnapMode::Grid
        } else if code == 1.0 {
            SnapMode::Relative
        } else {
            SnapMode::Free
        }
    }

    pub fn code(self) -> f64 {
        match self {
            SnapMode::Grid => 0.0,
            SnapMode::Relative => 1.0,
            SnapMode::Free => 2.0,
        }
    }
}

/// Is this gesture FREE of the grid: Ctrl or Cmd, read live off each event.
pub fn free_snap(ctrl: bool, meta: bool) -> bool {
    ctrl || meta
}

/// Shift frees the gesture, Ctrl or Cmd keeps the event's own offset from the grid.
pub fn snap_mode(shift: bool, ctrl: bool, meta: bool) -> SnapMode {
    if shift {
        SnapMode::Free
    } else if ctrl || meta {
        SnapMode::Relative
    } else {
        SnapMode::Grid
    }
}

/// `v` on the nearest multiple of `step`, or untouched when there is none: a step of 0 IS the free case.
pub fn snap_to(v: f64, step: f64) -> f64 {
    if step > 0.0 { js::round(v / step) * step } else { v }
}

/// How far a clip drag has travelled in beats: the OFFSET snaps, and the floor lands on the step too.
pub fn drag_offset(dx: f64, ppb: f64, step: f64, orig_start: f64) -> f64 {
    let raw = snap_to(dx / js::max(1e-6, ppb), step);
    let floor = if step > 0.0 {
        (-orig_start / step).ceil() * step
    } else {
        -orig_start
    };
    js::max(floor, raw)
}

/// Where a position dragged from `orig` by `delta` beats lands, under `mode`, on a grid of `step` beats.
pub fn snap_position(orig: f64, delta: f64, step: f64, mode: SnapMode) -> f64 {
    if mode == SnapMode::Free || step <= 0.0 {
        return orig + delta;
    }
    if mode == SnapMode::Relative {
        return orig + snap_to(delta, step);
    }
    snap_to(orig + delta, step)
}

/// A clip move's offset in beats under `mode`, never before the song's start.
pub fn move_offset(dx: f64, ppb: f64, step: f64, orig_start: f64, mode: SnapMode) -> f64 {
    let to = snap_position(orig_start, dx / js::max(1e-6, ppb), step, mode);
    let floor = if mode == SnapMode::Relative && step > 0.0 {
        orig_start + (-orig_start / step).ceil() * step
    } else {
        0.0
    };
    js::max(floor, to) - orig_start
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} is not {b}");
    }

    #[test]
    fn free_snap_is_ctrl_and_cmd_for_the_machine_where_ctrl_click_is_the_right_button() {
        assert!(free_snap(true, false));
        assert!(free_snap(false, true));
        assert!(!free_snap(false, false));
    }

    #[test]
    fn snap_to_lands_on_the_nearest_multiple() {
        assert_eq!(snap_to(1.4, 1.0), 1.0);
        assert_eq!(snap_to(1.6, 1.0), 2.0);
        assert_eq!(snap_to(0.3, 0.25), 0.25);
        assert_eq!(snap_to(-1.4, 1.0), -1.0);
    }

    #[test]
    fn snap_to_leaves_the_value_alone_when_there_is_no_step() {
        assert_eq!(snap_to(1.4, 0.0), 1.4);
        assert_eq!(snap_to(-0.37, 0.0), -0.37);
    }

    #[test]
    fn drag_offset_snaps_the_offset_to_the_step() {
        assert_eq!(drag_offset(30.0, 20.0, 1.0, 8.0), 2.0);
        assert_eq!(drag_offset(25.0, 20.0, 0.5, 8.0), 1.5);
    }

    #[test]
    fn drag_offset_keeps_an_off_grid_clip_off_the_grid() {
        close(4.43 + drag_offset(80.0, 20.0, 1.0, 4.43), 8.43);
    }

    #[test]
    fn drag_offset_follows_the_pointer_exactly_when_free() {
        close(drag_offset(30.0, 20.0, 0.0, 8.0), 1.5);
        close(drag_offset(7.0, 20.0, 0.0, 8.0), 0.35);
    }

    #[test]
    fn drag_offset_stops_at_the_start_on_the_step() {
        assert_eq!(3.0 + drag_offset(-500.0, 20.0, 1.0, 3.0), 0.0);
        close(3.4 + drag_offset(-500.0, 20.0, 1.0, 3.4), 0.4);
        assert_eq!(3.4 + drag_offset(-500.0, 20.0, 0.0, 3.4), 0.0);
    }

    #[test]
    fn snap_mode_reads_shift_as_free_ctrl_or_cmd_as_relative_nothing_as_the_grid() {
        assert_eq!(snap_mode(false, false, false), SnapMode::Grid);
        assert_eq!(snap_mode(false, true, false), SnapMode::Relative);
        assert_eq!(snap_mode(false, false, true), SnapMode::Relative);
        assert_eq!(snap_mode(true, true, false), SnapMode::Free);
    }

    #[test]
    fn snap_position_lands_on_the_grid_keeps_the_offset_or_goes_where_the_pointer_is() {
        assert_eq!(snap_position(2.3, 1.4, 1.0, SnapMode::Grid), 4.0);
        close(snap_position(2.3, 1.4, 1.0, SnapMode::Relative), 3.3);
        close(snap_position(2.3, 1.4, 1.0, SnapMode::Free), 3.7);
    }

    #[test]
    fn move_offset_never_moves_an_event_before_the_start() {
        close(move_offset(-100.0, 10.0, 1.0, 2.3, SnapMode::Grid), -2.3);
        close(move_offset(-100.0, 10.0, 1.0, 2.3, SnapMode::Relative), -2.0);
    }

    #[test]
    fn the_mode_code_round_trips() {
        for m in [SnapMode::Grid, SnapMode::Relative, SnapMode::Free] {
            assert_eq!(SnapMode::from_code(m.code()), m);
        }
    }
}
