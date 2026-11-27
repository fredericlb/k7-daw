// video_drag.rs — a picture placement dragged on the timeline: where the shot goes, what a finished drag writes, whether it moved at all.
// In SECONDS throughout, a picture lane being placed on the clock rather than the bar grid.

use crate::js;
use crate::snap::{SnapMode, snap_to};
use crate::ticks::nearest_tick;

/// The shortest shot a drag may leave, in seconds: `length: 0` already MEANS 'to the end of the source'.
const V_MIN: f64 = 0.04;

/// Seconds drawn for a `length: 0` placement, whose real extent needs the source opened to know.
pub const V_NATURAL: f64 = 4.0;

/// Where the press landed: the body moves the shot, an edge trims it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoMode {
    Move,
    In,
    Out,
}

impl VideoMode {
    /// The mode a typed call names: 0 move, 1 in, 2 out.
    pub fn from_code(code: f64) -> VideoMode {
        if code == 1.0 {
            VideoMode::In
        } else if code == 2.0 {
            VideoMode::Out
        } else {
            VideoMode::Move
        }
    }
}

/// A field of a picture placement, in seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Start,
    From,
    Length,
}

impl Field {
    pub fn code(self) -> f64 {
        match self {
            Field::Start => 0.0,
            Field::From => 1.0,
            Field::Length => 2.0,
        }
    }
}

/// A picture placement in hand: what the grab captured, and where the drag has it now.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub mode: VideoMode,
    pub orig_start: f64,
    /// 0 is to the end of the source.
    pub orig_length: f64,
    pub orig_from: f64,
    /// How fast the source runs under the shot, signed; None is 1.
    pub speed: Option<f64>,
    pub start: f64,
    pub length: f64,
}

/// The extent a placement is DRAWN at: its own, or the placeholder a length-0 shot gets.
fn drawn_length(length: f64) -> f64 {
    if length > 0.0 { length } else { V_NATURAL }
}

/// Where a picture drag puts the shot, as (start, length): `snap_sec` answers where a second lands.
pub fn video_drag_to(v: &Shot, delta_sec: f64, snap_sec: &dyn Fn(f64) -> f64) -> (f64, f64) {
    let drawn = drawn_length(v.orig_length);
    match v.mode {
        VideoMode::Move => (snap_sec(v.orig_start + delta_sec), v.length),
        VideoMode::In => {
            // The head follows the pointer and the in-point the head; forwards, the head cannot pass the source's first frame.
            let speed = v.speed.unwrap_or(1.0);
            let floor = if speed > 0.0 {
                v.orig_start - v.orig_from / speed
            } else {
                f64::NEG_INFINITY
            };
            let start = js::max(
                js::min(snap_sec(v.orig_start + delta_sec), v.orig_start + drawn - V_MIN),
                floor,
            );
            (start, drawn - (start - v.orig_start))
        }
        VideoMode::Out => {
            let end = js::max(snap_sec(v.orig_start + drawn + delta_sec), v.orig_start + V_MIN);
            (v.start, end - v.orig_start)
        }
    }
}

/// What a pointer move snaps a picture drag to.
pub struct VideoSnap<'a> {
    pub mode: SnapMode,
    /// The ruler's unit in beats, what a relative drag steps by.
    pub unit: f64,
    /// Beats per second.
    pub bps: f64,
    /// The ruler's drawn ticks in beats.
    pub ticks: &'a [f64],
}

impl VideoSnap<'_> {
    /// A second on the ruler's nearest tick when the grid rules; never before 0.
    fn snap_sec(&self, sec: f64) -> f64 {
        if self.mode != SnapMode::Grid {
            return js::max(0.0, sec);
        }
        match nearest_tick(self.ticks, sec * self.bps, f64::INFINITY) {
            None => js::max(0.0, sec),
            Some(t) => js::max(0.0, t / self.bps),
        }
    }
}

/// One pointer move of a picture drag: `dx` pixels from the grab at `px_per_sec`, snapped as `snap` says.
pub fn video_drag_move(v: &Shot, dx: f64, px_per_sec: f64, snap: &VideoSnap) -> (f64, f64) {
    // Measured from the GRAB and not from the last frame: a re-applied offset makes the edge run away from the cursor.
    let raw = dx / px_per_sec;
    let d = if snap.mode == SnapMode::Relative {
        snap_to(raw, snap.unit / snap.bps)
    } else {
        raw
    };
    video_drag_to(v, d, &|sec| snap.snap_sec(sec))
}

/// The fields a finished picture drag writes, in SECONDS: the left edge writes three, and each is load-bearing.
pub fn video_drag_edits(v: &Shot, activation: bool) -> Vec<(Field, f64)> {
    if v.mode == VideoMode::Out {
        return vec![(Field::Length, v.length)];
    }
    let mut edits = vec![(Field::Start, v.start)];
    if v.mode == VideoMode::In {
        let delta = v.start - v.orig_start;
        // An ACTIVATION has no source: its left edge moves the start and leaves the end where it was.
        if !activation {
            edits.push((Field::From, js::max(0.0, v.orig_from + delta * v.speed.unwrap_or(1.0))));
        }
        edits.push((Field::Length, js::max(V_MIN, v.length)));
    }
    edits
}

/// Whether a finished drag left the placement exactly where it was: then nothing is written.
pub fn video_drag_is_still(v: &Shot) -> bool {
    v.start == v.orig_start && (v.mode != VideoMode::Out || v.length == drawn_length(v.orig_length))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No snapping, which is the REAL snapper the handler passes while Ctrl is held.
    fn free(sec: f64) -> f64 {
        js::max(0.0, sec)
    }
    fn to_second(sec: f64) -> f64 {
        js::max(0.0, js::round(sec))
    }
    fn shot(mode: VideoMode) -> Shot {
        Shot {
            mode,
            orig_start: 4.0,
            orig_length: 6.0,
            orig_from: 2.0,
            speed: None,
            start: 4.0,
            length: 6.0,
        }
    }
    fn to(v: &Shot, delta: f64) -> (f64, f64) {
        video_drag_to(v, delta, &free)
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} is not {b}");
    }

    #[test]
    fn moving_a_shot_writes_only_its_start_cannot_go_before_zero_and_leaves_the_extent() {
        let v = Shot {
            start: 9.0,
            ..shot(VideoMode::Move)
        };
        assert_eq!(video_drag_edits(&v, false), vec![(Field::Start, 9.0)]);
        assert_eq!(to(&shot(VideoMode::Move), -100.0).0, 0.0);
        assert_eq!(to(&shot(VideoMode::Move), 3.0), (7.0, 6.0));
    }

    #[test]
    fn trimming_the_head_of_a_retimed_shot_moves_the_in_point_at_the_shots_speed() {
        let v = Shot {
            speed: Some(2.0),
            start: 5.0,
            length: 5.0,
            ..shot(VideoMode::In)
        };
        assert!(video_drag_edits(&v, false).contains(&(Field::From, 4.0)));
    }

    #[test]
    fn trimming_forwards_stops_at_the_sources_first_frame_reached_sooner_at_2x() {
        let v = Shot {
            speed: Some(2.0),
            ..shot(VideoMode::In)
        };
        assert_eq!(to(&v, -10.0).0, 3.0);
    }

    #[test]
    fn trimming_the_head_keeps_the_tail_where_it_is() {
        let (start, length) = to(&shot(VideoMode::In), 2.0);
        assert_eq!((start, length), (6.0, 4.0));
        assert_eq!(start + length, 4.0 + 6.0);
    }

    #[test]
    fn trimming_the_head_moves_the_in_point_by_the_same_delta() {
        let v = Shot {
            start: 6.0,
            length: 4.0,
            ..shot(VideoMode::In)
        };
        assert_eq!(
            video_drag_edits(&v, false),
            vec![(Field::Start, 6.0), (Field::From, 4.0), (Field::Length, 4.0)]
        );
    }

    #[test]
    fn trimming_refuses_a_frame_before_the_file_starts() {
        assert_eq!(to(&shot(VideoMode::In), -10.0).0, 2.0);
        let v = Shot {
            start: 2.0,
            length: 8.0,
            ..shot(VideoMode::In)
        };
        assert!(video_drag_edits(&v, false).contains(&(Field::From, 0.0)));
    }

    #[test]
    fn trimming_refuses_to_swallow_the_tail() {
        let (start, length) = to(&shot(VideoMode::In), 999.0);
        close(length, 0.04);
        close(start, 4.0 + 6.0 - 0.04);
    }

    #[test]
    fn trimming_pins_the_extent_of_a_length_zero_shot() {
        let v0 = Shot {
            orig_length: 0.0,
            length: 0.0,
            ..shot(VideoMode::In)
        };
        assert_eq!(to(&v0, 1.0), (5.0, 3.0));
        let edits = video_drag_edits(
            &Shot {
                start: 5.0,
                length: 3.0,
                ..v0
            },
            false,
        );
        assert_eq!(
            edits.iter().map(|e| e.0).collect::<Vec<_>>(),
            vec![Field::Start, Field::From, Field::Length]
        );
    }

    #[test]
    fn an_activation_moves_its_head_without_a_source_in_point() {
        let v = Shot {
            start: 6.0,
            length: 4.0,
            ..shot(VideoMode::In)
        };
        assert_eq!(
            video_drag_edits(&v, true),
            vec![(Field::Start, 6.0), (Field::Length, 4.0)]
        );
    }

    #[test]
    fn setting_the_extent_writes_only_the_length_and_never_moves_the_head() {
        assert_eq!(to(&shot(VideoMode::Out), 3.0), (4.0, 9.0));
        let v = Shot {
            length: 9.0,
            ..shot(VideoMode::Out)
        };
        assert_eq!(video_drag_edits(&v, false), vec![(Field::Length, 9.0)]);
        close(to(&shot(VideoMode::Out), -999.0).1, 0.04);
    }

    #[test]
    fn setting_the_extent_of_a_length_zero_shot_starts_from_the_drawn_placeholder() {
        let v = Shot {
            orig_length: 0.0,
            length: 0.0,
            ..shot(VideoMode::Out)
        };
        assert_eq!(to(&v, 1.0), (4.0, 5.0));
    }

    #[test]
    fn a_drag_that_changed_nothing_is_still() {
        assert!(video_drag_is_still(&shot(VideoMode::Move)));
        assert!(!video_drag_is_still(&Shot {
            start: 5.0,
            ..shot(VideoMode::Move)
        }));
    }

    #[test]
    fn a_tail_back_where_it_was_is_still_measured_against_the_drawn_extent() {
        let v = Shot {
            orig_length: 0.0,
            length: 0.0,
            ..shot(VideoMode::Out)
        };
        let (start, length) = to(&v, 0.0);
        assert!(video_drag_is_still(&Shot { start, length, ..v }));
    }

    #[test]
    fn the_head_snaps_in_whatever_unit_the_ruler_reads_and_the_tail_against_the_shots_end() {
        assert_eq!(video_drag_to(&shot(VideoMode::Move), 2.4, &to_second).0, 6.0);
        assert_eq!(video_drag_to(&shot(VideoMode::Move), 2.6, &to_second).0, 7.0);
        assert_eq!(video_drag_to(&shot(VideoMode::Out), 0.4, &to_second).1, 6.0);
        assert_eq!(video_drag_to(&shot(VideoMode::Out), 0.6, &to_second).1, 7.0);
    }

    #[test]
    fn a_pointer_move_snaps_to_the_rulers_ticks_steps_by_the_unit_on_ctrl_and_is_free_on_shift() {
        // Two beats per second; ticks every half beat from 0 to 8.
        let ticks: Vec<f64> = (0..=16).map(|i| f64::from(i) * 0.5).collect();
        let snap = |mode| VideoSnap {
            mode,
            unit: 1.0,
            bps: 2.0,
            ticks: &ticks,
        };
        let v = shot(VideoMode::Move);
        // 130 px at 100 px/s is 1.3 s: from 4 s that is 10.6 beats, past the last tick (8 beats = 4 s).
        assert_eq!(video_drag_move(&v, 130.0, 100.0, &snap(SnapMode::Grid)).0, 4.0);
        // Relative steps the delta by a whole unit (0.5 s): 1.3 s becomes 1.5 s.
        close(video_drag_move(&v, 130.0, 100.0, &snap(SnapMode::Relative)).0, 5.5);
        close(video_drag_move(&v, 130.0, 100.0, &snap(SnapMode::Free)).0, 5.3);
    }
}
