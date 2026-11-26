// pan_zoom.rs — the arithmetic of the media panes' zoom: what one wheel event is worth, how far a pane may zoom, where the scroll lands so the point under the cursor stays put.
// Unit-free on purpose: the caller multiplies its own lane width, in beats or in seconds.

use crate::js;

/// A wheel event that zooms whatever the DEVICE is: a pinch (which the browser sends as Ctrl), or the modifier.
pub fn is_zoom_wheel(ctrl: bool, meta: bool) -> bool {
    ctrl || meta
}

/// Below this, a pixel-mode delta is a finger rather than a detent. A notch is ~100 (Chrome).
const MOUSE_NOTCH_MIN: f64 = 40.0;

/// A mouse WHEEL rather than a surface, from THREE signals and wrong in one direction only: towards the surface.
pub fn is_mouse_wheel(delta_x: f64, delta_y: f64, delta_mode: f64, wheel_delta_y: Option<f64>) -> bool {
    if delta_mode != 0.0 {
        return true;
    }
    if let Some(legacy) = wheel_delta_y.filter(|l| *l != 0.0) {
        return legacy.abs() % 120.0 == 0.0;
    }
    if delta_x != 0.0 {
        return false;
    }
    delta_y.fract() == 0.0 && delta_y.abs() >= MOUSE_NOTCH_MIN
}

/// One wheel notch's worth of zoom: a whole step, because a notch is a discrete decision.
const WHEEL_STEP: f64 = 1.15;

/// How much one wheel event zooms, a pinch being unable to share the notch's answer.
pub fn wheel_factor(delta: f64, pinch: bool) -> f64 {
    if !pinch {
        return if delta < 0.0 { WHEEL_STEP } else { 1.0 / WHEEL_STEP };
    }
    (-js::clamp(delta, -50.0, 50.0) * 0.01).exp()
}

/// The widest a scaled lane may become: half the browser's ~33.5M px layout limit.
pub const MAX_LANE_PX: f64 = 16_000_000.0;

/// How far a pane of `width` px may zoom before it meets that: DERIVED, not chosen.
pub fn zoom_ceiling(width: f64) -> f64 {
    if width > 0.0 {
        js::max(1.0, MAX_LANE_PX / width)
    } else {
        32.0
    }
}

/// The zoom a wheel event asks for from `cur`, kept between `min` and `ceiling`.
pub fn wheel_zoom_to(cur: f64, delta: f64, pinch: bool, min: f64, ceiling: f64) -> f64 {
    js::clamp(cur * wheel_factor(delta, pinch), min, ceiling)
}

/// The zoom two fingers ask for: the spread now over the spread at the start, times the zoom then.
pub fn pinch_zoom_to(start_zoom: f64, dist: f64, start_dist: f64, min: f64, ceiling: f64) -> f64 {
    js::clamp(start_zoom * (dist / start_dist), min, ceiling)
}

/// The scrollLeft to apply AFTER the width changes from `cur` to `next`, keeping what is under `offset_x` fixed.
pub fn anchored_scroll_left(scroll_left: f64, offset_x: f64, gutter: f64, cur: f64, next: f64) -> f64 {
    // The gutter does not scale, so it is subtracted and re-added around the ratio.
    let lane_x = scroll_left + offset_x - gutter;
    gutter + lane_x * (next / cur) - offset_x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "{a} is not {b}");
    }

    #[test]
    fn a_notch_takes_a_whole_step_whatever_the_delta_says() {
        close(wheel_factor(-100.0, false), 1.15, 1e-5);
        close(wheel_factor(-3.0, false), 1.15, 1e-5);
        close(wheel_factor(100.0, false), 1.0 / 1.15, 1e-5);
    }

    #[test]
    fn a_pinch_tracks_the_fingers_instead_of_stepping() {
        assert!(wheel_factor(-2.0, true) < wheel_factor(-2.0, false));
        close(wheel_factor(-2.0, true), 1.02, 0.005);
    }

    #[test]
    fn a_pinch_is_proportional_and_a_notch_is_not() {
        assert!(wheel_factor(-10.0, true) > wheel_factor(-5.0, true));
        assert_eq!(wheel_factor(-10.0, false), wheel_factor(-5.0, false));
    }

    #[test]
    fn a_pinch_zooms_in_on_a_spread_and_out_on_a_close_and_cannot_jump_a_decade() {
        assert!(wheel_factor(-5.0, true) > 1.0);
        assert!(wheel_factor(5.0, true) < 1.0);
        assert!(wheel_factor(-5000.0, true) < 1.7);
        assert!(wheel_factor(5000.0, true) > 0.6);
        assert_eq!(wheel_factor(0.0, true), 1.0);
    }

    #[test]
    fn a_pinch_or_the_modifier_zooms_and_a_plain_wheel_pans() {
        assert!(is_zoom_wheel(true, false));
        assert!(is_zoom_wheel(false, true));
        assert!(!is_zoom_wheel(false, false));
    }

    #[test]
    fn lines_or_pages_are_a_wheel() {
        assert!(is_mouse_wheel(0.0, 3.0, 1.0, None));
        assert!(is_mouse_wheel(0.0, 1.0, 2.0, None));
    }

    #[test]
    fn a_mouse_is_told_from_a_trackpad_by_the_legacy_120s() {
        assert!(is_mouse_wheel(0.0, 100.0, 0.0, Some(-120.0)));
        assert!(!is_mouse_wheel(0.0, 4.0, 0.0, Some(-12.0)));
        assert!(!is_mouse_wheel(0.0, 1.5, 0.0, Some(-4.5)));
    }

    #[test]
    fn the_deltas_shape_decides_where_the_legacy_field_is_absent_and_errs_towards_the_surface() {
        assert!(is_mouse_wheel(0.0, 120.0, 0.0, None));
        assert!(!is_mouse_wheel(0.0, 6.0, 0.0, None));
        assert!(!is_mouse_wheel(0.0, 60.5, 0.0, None));
        assert!(!is_mouse_wheel(8.0, 100.0, 0.0, None));
        assert!(!is_mouse_wheel(0.0, 0.0, 0.0, None));
    }

    #[test]
    fn the_ceiling_is_what_the_browser_can_still_lay_out() {
        assert!(zoom_ceiling(1200.0) > 10_000.0);
        assert!(1200.0 * zoom_ceiling(1200.0) <= MAX_LANE_PX);
        assert!(zoom_ceiling(400.0) > zoom_ceiling(1200.0));
        close(400.0 * zoom_ceiling(400.0), 1200.0 * zoom_ceiling(1200.0), 0.5);
    }

    #[test]
    fn the_ceiling_never_answers_less_than_fit() {
        assert!(zoom_ceiling(0.0) >= 1.0);
        assert!(zoom_ceiling(-5.0) >= 1.0);
        assert_eq!(zoom_ceiling(9_007_199_254_740_991.0), 1.0);
    }

    #[test]
    fn a_wheel_zoom_stays_between_the_floor_and_the_ceiling() {
        assert_eq!(wheel_zoom_to(1.0, 100.0, false, 1.0, 50.0), 1.0);
        assert_eq!(wheel_zoom_to(49.0, -100.0, false, 1.0, 50.0), 50.0);
        close(wheel_zoom_to(2.0, -100.0, false, 1.0, 50.0), 2.3, 1e-9);
        assert_eq!(pinch_zoom_to(2.0, 300.0, 100.0, 1.0, 5.0), 5.0);
        assert_eq!(pinch_zoom_to(2.0, 150.0, 100.0, 1.0, 5.0), 3.0);
    }

    #[test]
    fn the_scroll_keeps_the_point_under_the_cursor_where_it_is() {
        // At zoom 1 a lane point 400 px in sits under a cursor 100 px from the pane's edge: after doubling it stays there.
        let left = anchored_scroll_left(300.0, 100.0, 0.0, 1.0, 2.0);
        assert_eq!(left, 700.0);
        // The gutter does not scale.
        let left = anchored_scroll_left(0.0, 150.0, 50.0, 1.0, 2.0);
        assert_eq!(50.0 + (0.0 + 150.0 - 50.0) * 2.0 - 150.0, left);
    }
}
