// clip_playhead.rs — where the transport is ON A CLIP'S OWN PICTURE: the join of the song's seconds and the file's.
// A wrong join does not fail: it draws a line somewhere plausible and wrong.

/// The slice of the file an event plays, in file seconds.
#[derive(Clone, Copy, Debug)]
pub struct ActWindow {
    pub from: f64,
    pub to: f64,
}

/// The transport's position as a second of THIS FILE, or None when it is outside the event.
pub fn clip_playhead(song_seconds: f64, placed_at: Option<f64>, act: ActWindow, tempo: f64) -> Option<f64> {
    let placed_at = placed_at?;
    if tempo.is_nan() || tempo <= 0.0 {
        return None;
    }
    let beats_in = (song_seconds * tempo) / 60.0 - placed_at;
    if beats_in < 0.0 {
        return None;
    }
    let at = act.from + (beats_in * 60.0) / tempo;
    // Inclusive of the far edge by a hair: the last frame of a clip ending on a beat arrives at `act.to`.
    if at <= act.to + 1e-6 { Some(at) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 110 BPM: the event opens at bar 24 playing 52.36 to 69.82 s of its file, verbatim from a song.
    const T: f64 = 110.0;
    const AT: f64 = 92.0;
    const ACT: ActWindow = ActWindow {
        from: 52.36363636363637,
        to: 69.81818181818181,
    };
    fn song_sec(beat: f64) -> f64 {
        (beat * 60.0) / T
    }
    fn near(a: Option<f64>, b: f64) {
        assert!((a.expect("a mark") - b).abs() < 1e-6, "{a:?} is not {b}");
    }

    #[test]
    fn lands_on_the_windows_head_the_instant_the_event_opens() {
        near(clip_playhead(song_sec(AT), Some(AT), ACT, T), ACT.from);
    }

    #[test]
    fn advances_through_the_file_second_for_second() {
        near(
            clip_playhead(song_sec(AT + 4.0), Some(AT), ACT, T),
            ACT.from + (4.0 * 60.0) / T,
        );
    }

    #[test]
    fn reaches_the_far_edge_and_stops_there() {
        let beats = ((ACT.to - ACT.from) * T) / 60.0;
        near(clip_playhead(song_sec(AT + beats), Some(AT), ACT, T), ACT.to);
        assert_eq!(clip_playhead(song_sec(AT + beats + 0.5), Some(AT), ACT, T), None);
    }

    #[test]
    fn says_nothing_before_the_event_for_an_unplaced_clip_or_with_no_tempo() {
        assert_eq!(clip_playhead(song_sec(AT - 1.0), Some(AT), ACT, T), None);
        assert_eq!(clip_playhead(0.0, Some(AT), ACT, T), None);
        assert_eq!(clip_playhead(30.0, None, ACT, T), None);
        assert_eq!(clip_playhead(30.0, Some(AT), ACT, 0.0), None);
    }

    #[test]
    fn works_for_a_clip_that_plays_its_file_from_the_head() {
        near(
            clip_playhead(song_sec(4.0), Some(0.0), ActWindow { from: 0.0, to: 10.0 }, T),
            song_sec(4.0),
        );
    }
}
