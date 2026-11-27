// roll_drag.rs — what a gesture on the piano roll WRITES, as arithmetic with no DOM.
// Every answer is read twice, by the ghost the roll draws and by the command it commits. A press that never leaves its cell is ONE GRID STEP long, not a beat.

use crate::js;
use crate::snap::{SnapMode, snap_position};
use serde::{Deserialize, Serialize};
use song_schema::round_beat;

/// The shortest a stretch may leave a note: a 64th at any tempo.
pub const MIN_NOTE_BEATS: f64 = 1.0 / 16.0;

/// Which end of a note is in hand, or the whole note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollMode {
    Move,
    Start,
    End,
}

impl RollMode {
    /// The mode a typed call names: 0 move, 1 start, 2 end.
    pub fn from_code(code: f64) -> RollMode {
        if code == 1.0 {
            RollMode::Start
        } else if code == 2.0 {
            RollMode::End
        } else {
            RollMode::Move
        }
    }
}

/// A drag on one note, addressed by key and by where it starts NOW; `d_keys` is positive up and a stretch has none.
#[derive(Clone, Copy, Debug)]
pub struct RollDrag {
    pub key: f64,
    pub time: f64,
    pub dur: f64,
    pub mode: RollMode,
    pub d_beats: f64,
    pub d_keys: f64,
}

/// Where a drag has taken the note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollSpan {
    pub key: f64,
    pub start: f64,
    pub end: f64,
}

/// Onto the keyboard rather than off it: a note dragged past either end stops at it.
pub fn clamp_key(k: f64) -> f64 {
    js::max(0.0, js::min(127.0, js::round(k)))
}

pub fn t_span(t: &RollDrag) -> RollSpan {
    if t.mode == RollMode::Move {
        let start = js::max(0.0, t.time + t.d_beats);
        return RollSpan {
            key: clamp_key(t.key + t.d_keys),
            start,
            end: start + t.dur,
        };
    }
    // A stretch keeps its pitch whatever the hand did: an end has no pitch.
    let end = t.time + t.dur;
    if t.mode == RollMode::Start {
        RollSpan {
            key: t.key,
            start: js::max(0.0, js::min(end - MIN_NOTE_BEATS, t.time + t.d_beats)),
            end,
        }
    } else {
        RollSpan {
            key: t.key,
            start: t.time,
            end: js::max(t.time + MIN_NOTE_BEATS, end + t.d_beats),
        }
    }
}

/// Did this drag change the note's PITCH: which verb commits it.
pub fn moves_pitch(t: &RollDrag) -> bool {
    t.mode == RollMode::Move && clamp_key(t.key + t.d_keys) != t.key
}

/// Has this drag anything to write at all: a press that has not travelled commits nothing.
pub fn drag_moved(t: &RollDrag) -> bool {
    t.d_beats.abs() > 1e-6 || moves_pitch(t)
}

/// The span a draw gesture covers in beats: the CELLS it was dragged across, `free_len` being a freed click's worth.
pub fn draw_span(a: f64, b: f64, step: f64, origin: f64, free_len: f64) -> (f64, f64) {
    if step <= 0.0 {
        let from = js::min(a, b);
        let to = js::max(a, b);
        return (
            from,
            if to - from > 1e-6 {
                to
            } else {
                from + js::max(1e-6, free_len)
            },
        );
    }
    let col = |beat: f64| ((beat - origin) / step + 1e-9).floor();
    let lo = js::min(col(a), col(b));
    let hi = js::max(col(a), col(b));
    (origin + lo * step, origin + (hi + 1.0) * step)
}

/// How far a pointer has travelled in ROWS, positive up: snapped to the line and never freed.
pub fn key_offset(dy: f64, row_h: f64) -> f64 {
    if row_h > 0.0 { -js::round(dy / row_h) } else { 0.0 }
}

/// A velocity from where the pointer sits INSIDE a note's row: 0 at its bottom edge, 1 at its top.
pub fn vel_at(y_in_row: f64, row_h: f64) -> f64 {
    if row_h <= 1.0 {
        return 1.0;
    }
    js::max(0.0, js::min(1.0, 1.0 - y_in_row / (row_h - 1.0)))
}

/// Where a pointer is on the roll, as (beat, key): the lane's left edge is the ABSOLUTE `origin`, a row is one key, and a pointer outside is pulled onto the canvas.
pub fn roll_point(dx: f64, dy: f64, origin: f64, ppb: f64, canvas_beats: f64, row_h: f64) -> (f64, f64) {
    let beat = js::clamp(origin + dx / ppb, origin, origin + canvas_beats);
    let key = js::clamp(127.0 - (dy / row_h).floor(), 0.0, 127.0);
    (beat, key)
}

/// How far a note drag has taken the note from the press: (beats, keys). `anchor` is the edge in hand; a stretch has no pitch.
#[allow(clippy::too_many_arguments)]
pub fn roll_drag_delta(
    mode: RollMode,
    time: f64,
    dur: f64,
    dx: f64,
    dy: f64,
    ppb: f64,
    row_h: f64,
    step: f64,
    snap: SnapMode,
    moves_keys: bool,
) -> (f64, f64) {
    let anchor = if mode == RollMode::End { time + dur } else { time };
    let raw = dx / js::max(1.0, ppb);
    let keys = if moves_keys && mode == RollMode::Move {
        key_offset(dy, row_h)
    } else {
        0.0
    };
    (snap_position(anchor, raw, step, snap) - anchor, keys)
}

/// A note as a group drag reads it: where it is now.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GroupNote {
    pub key: f64,
    pub time: f64,
    pub duration: f64,
}

/// The offset a SELECTION drag may take, clamped so the whole group stays on the keyboard and after beat 0.
pub fn group_offset(group: &[GroupNote], d_beats: f64, d_keys: f64) -> (f64, f64) {
    if group.is_empty() {
        return (0.0, 0.0);
    }
    let lo = group.iter().fold(f64::INFINITY, |m, n| js::min(m, n.key));
    let hi = group.iter().fold(f64::NEG_INFINITY, |m, n| js::max(m, n.key));
    let first = group.iter().fold(f64::INFINITY, |m, n| js::min(m, n.time));
    (
        js::max(-first, d_beats),
        js::max(-lo, js::min(127.0 - hi, js::round(d_keys))),
    )
}

/// Why a selection drag writes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Refused {
    Occupied,
    Still,
}

/// `sequence notes move`'s payload, as `[key, time, to_key, to_time]` rows, or why there is none.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum GroupMove {
    Moves {
        moves: Vec<[f64; 4]>,
        d_beats: f64,
        d_keys: f64,
    },
    Refused {
        refused: Refused,
    },
}

/// Where a selection drag sends every note, or why it cannot: a note that STAYS is in the way.
pub fn plan_group_move(group: &[GroupNote], others: &[GroupNote], d_beats: f64, d_keys: f64) -> GroupMove {
    let (off_beats, off_keys) = group_offset(group, d_beats, d_keys);
    if off_beats.abs() < 1e-6 && off_keys == 0.0 {
        return GroupMove::Refused {
            refused: Refused::Still,
        };
    }
    // Rounded so a pixel's float tail never reaches the file.
    let moves: Vec<[f64; 4]> = group
        .iter()
        .map(|n| [n.key, n.time, n.key + off_keys, round_beat(n.time + off_beats)])
        .collect();
    let taken = |k: f64, t: f64| others.iter().any(|o| o.key == k && (o.time - t).abs() < 1e-6);
    if moves.iter().any(|m| taken(m[2], m[3])) {
        return GroupMove::Refused {
            refused: Refused::Occupied,
        };
    }
    GroupMove::Moves {
        moves,
        d_beats: off_beats,
        d_keys: off_keys,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(mode: RollMode, d_beats: f64, d_keys: f64) -> RollDrag {
        RollDrag {
            key: 60.0,
            time: 4.0,
            dur: 1.0,
            mode,
            d_beats,
            d_keys,
        }
    }
    fn span(key: f64, start: f64, end: f64) -> RollSpan {
        RollSpan { key, start, end }
    }

    #[test]
    fn draw_span_covers_the_cell_a_press_never_left() {
        assert_eq!(draw_span(4.1, 4.1, 0.25, 0.0, 0.25), (4.0, 4.25));
    }

    #[test]
    fn draw_span_covers_both_ends_of_a_drag_inclusive_and_reads_backwards_the_same() {
        assert_eq!(draw_span(4.1, 4.9, 0.25, 0.0, 0.25), (4.0, 5.0));
        assert_eq!(
            draw_span(4.9, 4.1, 0.25, 0.0, 0.25),
            draw_span(4.1, 4.9, 0.25, 0.0, 0.25)
        );
    }

    #[test]
    fn draw_span_counts_cells_off_the_origin() {
        assert_eq!(draw_span(9.6, 9.6, 0.5, 8.0, 0.5), (9.5, 10.0));
    }

    #[test]
    fn draw_span_takes_the_raw_beats_when_freed_and_still_gives_a_click_a_length() {
        assert_eq!(draw_span(4.13, 4.87, 0.0, 0.0, 0.25), (4.13, 4.87));
        assert_eq!(draw_span(4.13, 4.13, 0.0, 0.0, 0.25), (4.13, 4.13 + 0.25));
    }

    #[test]
    fn a_move_carries_the_duration_and_the_pitch() {
        assert_eq!(t_span(&drag(RollMode::Move, 2.0, 3.0)), span(63.0, 6.0, 7.0));
    }

    #[test]
    fn a_move_stops_at_beat_zero_and_at_either_end_of_the_keyboard() {
        assert_eq!(t_span(&drag(RollMode::Move, -9.0, 0.0)), span(60.0, 0.0, 1.0));
        assert_eq!(
            t_span(&RollDrag {
                key: 2.0,
                ..drag(RollMode::Move, 0.0, -9.0)
            })
            .key,
            0.0
        );
        assert_eq!(
            t_span(&RollDrag {
                key: 125.0,
                ..drag(RollMode::Move, 0.0, 9.0)
            })
            .key,
            127.0
        );
    }

    #[test]
    fn a_stretch_moves_one_end_and_leaves_the_other() {
        let start = RollDrag {
            dur: 2.0,
            ..drag(RollMode::Start, -1.0, 0.0)
        };
        assert_eq!(t_span(&start), span(60.0, 3.0, 6.0));
    }

    #[test]
    fn a_stretch_never_lets_an_end_cross_the_other() {
        let start = RollDrag {
            dur: 2.0,
            ..drag(RollMode::Start, 5.0, 0.0)
        };
        assert_eq!(t_span(&start).start, 6.0 - MIN_NOTE_BEATS);
        let end = RollDrag {
            dur: 2.0,
            ..drag(RollMode::End, -5.0, 0.0)
        };
        assert_eq!(t_span(&end).end, 4.0 + MIN_NOTE_BEATS);
    }

    #[test]
    fn a_stretch_keeps_its_pitch_when_the_hand_wandered_off_the_row() {
        assert_eq!(t_span(&drag(RollMode::End, 1.0, 7.0)).key, 60.0);
        assert_eq!(t_span(&drag(RollMode::Start, -0.5, -7.0)).key, 60.0);
    }

    #[test]
    fn which_verb_a_drag_commits() {
        assert!(moves_pitch(&drag(RollMode::Move, 0.0, 1.0)));
        assert!(!moves_pitch(&drag(RollMode::Move, 3.0, 0.0)));
        assert!(!moves_pitch(&RollDrag {
            key: 127.0,
            ..drag(RollMode::Move, 0.0, 4.0)
        }));
        assert!(!moves_pitch(&drag(RollMode::End, 0.0, 4.0)));
    }

    #[test]
    fn a_press_that_has_not_travelled_writes_nothing() {
        assert!(!drag_moved(&drag(RollMode::Move, 0.0, 0.0)));
        assert!(drag_moved(&drag(RollMode::Move, 0.0, 1.0)));
        assert!(drag_moved(&drag(RollMode::Move, 0.25, 0.0)));
    }

    #[test]
    fn a_pointer_is_a_beat_off_the_origin_and_a_key_counted_from_the_top() {
        // The lane starts at beat 8; 40 px at 20 px a beat is two beats in, and 30 px at 12 px a row is row 2, key 125.
        assert_eq!(roll_point(40.0, 30.0, 8.0, 20.0, 16.0, 12.0), (10.0, 125.0));
    }

    #[test]
    fn a_pointer_outside_is_pulled_onto_the_canvas() {
        assert_eq!(roll_point(-50.0, -9.0, 8.0, 20.0, 16.0, 12.0), (8.0, 127.0));
        assert_eq!(roll_point(9000.0, 99999.0, 8.0, 20.0, 16.0, 12.0), (24.0, 0.0));
    }

    #[test]
    fn a_body_drag_snaps_its_start_and_counts_rows_and_a_stretch_has_no_pitch() {
        let d = |mode, dx, dy, keys| roll_drag_delta(mode, 4.0, 1.0, dx, dy, 20.0, 12.0, 0.5, SnapMode::Grid, keys);
        assert_eq!(d(RollMode::Move, 25.0, -24.0, true), (1.5, 2.0));
        assert_eq!(d(RollMode::Move, 25.0, -24.0, false), (1.5, 0.0));
        assert_eq!(d(RollMode::End, 25.0, -24.0, true), (1.5, 0.0));
        let free = roll_drag_delta(
            RollMode::Move,
            4.0,
            1.0,
            25.0,
            0.0,
            20.0,
            12.0,
            0.5,
            SnapMode::Free,
            true,
        );
        assert_eq!(free, (1.25, 0.0));
    }

    #[test]
    fn key_offset_is_positive_up_and_snaps_to_the_row() {
        assert_eq!(key_offset(-24.0, 12.0), 2.0);
        assert_eq!(key_offset(24.0, 12.0), -2.0);
        assert_eq!(key_offset(-7.0, 12.0), 1.0);
        assert_eq!(key_offset(-5.0, 12.0), 0.0);
        assert_eq!(key_offset(-24.0, 0.0), 0.0);
    }

    #[test]
    fn vel_at_is_one_at_the_top_zero_at_the_bottom_and_clamped() {
        assert_eq!(vel_at(0.0, 13.0), 1.0);
        assert_eq!(vel_at(12.0, 13.0), 0.0);
        assert_eq!(vel_at(-40.0, 13.0), 1.0);
        assert_eq!(vel_at(40.0, 13.0), 0.0);
        assert_eq!(vel_at(4.0, 1.0), 1.0);
    }

    #[test]
    fn clamp_key_rounds_to_a_whole_semitone() {
        assert_eq!(clamp_key(60.4), 60.0);
        assert_eq!(clamp_key(60.6), 61.0);
    }

    fn chord() -> Vec<GroupNote> {
        vec![
            GroupNote {
                key: 60.0,
                time: 1.0,
                duration: 1.0,
            },
            GroupNote {
                key: 64.0,
                time: 1.0,
                duration: 1.0,
            },
            GroupNote {
                key: 67.0,
                time: 2.0,
                duration: 0.5,
            },
        ]
    }

    #[test]
    fn a_selection_moves_every_note_by_the_same_beats_and_semitones() {
        let plan = plan_group_move(&chord(), &[], 1.0, 2.0);
        let want = GroupMove::Moves {
            moves: vec![[60.0, 1.0, 62.0, 2.0], [64.0, 1.0, 66.0, 2.0], [67.0, 2.0, 69.0, 3.0]],
            d_beats: 1.0,
            d_keys: 2.0,
        };
        assert_eq!(plan, want);
    }

    #[test]
    fn a_selection_stops_the_whole_group_at_an_edge() {
        assert_eq!(group_offset(&chord(), 0.0, 70.0), (0.0, 60.0));
        assert_eq!(group_offset(&chord(), -5.0, 0.0), (-1.0, 0.0));
    }

    #[test]
    fn a_selection_refuses_a_note_in_the_way_and_a_drag_that_ends_where_it_began() {
        let stays = [GroupNote {
            key: 62.0,
            time: 2.0,
            duration: 1.0,
        }];
        assert_eq!(
            plan_group_move(&chord(), &stays, 1.0, 2.0),
            GroupMove::Refused {
                refused: Refused::Occupied
            }
        );
        assert_eq!(
            plan_group_move(&chord(), &[], 0.0000001, 0.0),
            GroupMove::Refused {
                refused: Refused::Still
            }
        );
    }

    #[test]
    fn the_plan_serializes_as_the_verbs_payload() {
        let json = serde_json::to_value(plan_group_move(&chord(), &[], 1.0, 2.0)).unwrap();
        assert_eq!(json["dBeats"], 1.0);
        assert_eq!(json["moves"][0], serde_json::json!([60.0, 1.0, 62.0, 2.0]));
        let still = serde_json::to_value(plan_group_move(&chord(), &[], 0.0, 0.0)).unwrap();
        assert_eq!(still, serde_json::json!({"refused": "still"}));
    }
}
