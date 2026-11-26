// take_counter.rs — what the clip panel's header counts: the pre-roll down to zero, then the beats of the take.
// Both results are CLAMPED, so the worst this shows is a number that stops moving.

use crate::js;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    CountIn,
    Recording,
}

/// Where a take is: beats REMAINING in the pre-roll, or beats recorded so far; never negative.
pub fn take_counter(start_beat: f64, cur_beat: f64, count_in_beats: f64) -> (Phase, f64) {
    let pre_roll = js::max(0.0, count_in_beats);
    let elapsed = cur_beat - start_beat;
    if elapsed < pre_roll {
        return (
            Phase::CountIn,
            js::min(pre_roll, js::max(0.0, (pre_roll - elapsed).ceil())),
        );
    }
    (Phase::Recording, js::max(0.0, (elapsed - pre_roll).floor()))
}

/// The beat the take's MATERIAL starts on: the transport's start plus the count-in.
pub fn recording_from(start_beat: f64, count_in_beats: f64) -> f64 {
    start_beat + js::max(0.0, count_in_beats)
}

/// The take's position as 1-based (bar, beat): a live counter, so x/4 is assumed as it always was.
pub fn bar_beat(recorded_beats: f64, beats_per_bar: f64) -> (f64, f64) {
    let bpb = js::max(1.0, beats_per_bar);
    let b = js::max(0.0, recorded_beats.floor());
    ((b / bpb).floor() + 1.0, (b % bpb) + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNT: Phase = Phase::CountIn;
    const REC: Phase = Phase::Recording;

    #[test]
    fn the_pre_roll_counts_down_whole_beats_to_the_take() {
        assert_eq!(take_counter(0.0, 0.0, 4.0), (COUNT, 4.0));
        assert_eq!(take_counter(0.0, 1.0, 4.0), (COUNT, 3.0));
        assert_eq!(take_counter(0.0, 3.5, 4.0), (COUNT, 1.0));
    }

    #[test]
    fn it_measures_from_where_this_take_started_not_from_bar_one() {
        assert_eq!(take_counter(56.0, 56.0, 4.0), (COUNT, 4.0));
        assert_eq!(take_counter(56.0, 58.0, 4.0), (COUNT, 2.0));
        assert_eq!(take_counter(56.0, 60.0, 4.0), (REC, 0.0));
    }

    #[test]
    fn the_pre_roll_is_skipped_when_the_project_asks_for_none_and_never_exceeds_its_length() {
        assert_eq!(take_counter(56.0, 56.0, 0.0), (REC, 0.0));
        assert_eq!(take_counter(500.0, 4.0, 4.0), (COUNT, 4.0));
    }

    #[test]
    fn the_take_counts_beats_from_the_end_of_the_pre_roll_and_never_runs_for_less_than_nothing() {
        assert_eq!(take_counter(0.0, 4.0, 4.0), (REC, 0.0));
        assert_eq!(take_counter(0.0, 9.9, 4.0), (REC, 5.0));
        assert_eq!(take_counter(0.0, -20.0, 0.0).1, 0.0);
    }

    #[test]
    fn bar_beat_is_one_based_and_wraps_on_the_bar() {
        assert_eq!(bar_beat(0.0, 4.0), (1.0, 1.0));
        assert_eq!(bar_beat(3.0, 4.0), (1.0, 4.0));
        assert_eq!(bar_beat(4.0, 4.0), (2.0, 1.0));
        assert_eq!(bar_beat(9.0, 4.0), (3.0, 2.0));
    }

    #[test]
    fn bar_beat_survives_a_nonsense_time_signature() {
        assert_eq!(bar_beat(5.0, 0.0), (6.0, 1.0));
    }

    #[test]
    fn recording_from_is_the_transports_start_plus_the_count_in_never_backwards() {
        assert_eq!(recording_from(16.0, 4.0), 20.0);
        assert_eq!(recording_from(16.0, 0.0), 16.0);
        assert_eq!(recording_from(16.0, -8.0), 16.0);
    }

    #[test]
    fn recording_from_agrees_with_the_counter_about_when_recording_begins() {
        let from = recording_from(16.0, 4.0);
        assert_eq!(take_counter(16.0, from - 1.0, 4.0).0, COUNT);
        assert_eq!(take_counter(16.0, from, 4.0).0, REC);
    }
}
