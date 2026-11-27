// warp.rs — a clip's stretch map read both ways: the timeline beat a source second lands on, and the second a beat plays.
// A reader would get wrong that both extrapolate along the first or last segment, and a map under two points is the identity.

/// One point of the map: where a moment of the SOURCE lands on the timeline, as `[beat, second]`.
pub type Marker = [f64; 2];

/// The index of the segment a position falls in along `axis` (0 beats, 1 seconds): the first that reaches it, else the last.
fn segment(markers: &[Marker], axis: usize, at: f64) -> usize {
    if at <= markers[0][axis] {
        return 0;
    }
    (0..markers.len() - 1)
        .find(|i| at <= markers[i + 1][axis])
        .unwrap_or(markers.len() - 2)
}

/// The timeline BEAT a source second lands on, segment by segment and extrapolating past the ends.
#[must_use]
pub fn beat_at_source(markers: &[Marker], sec: f64) -> f64 {
    if markers.len() < 2 {
        return sec;
    }
    let i = segment(markers, 1, sec);
    let ([b0, s0], [b1, s1]) = (markers[i], markers[i + 1]);
    let d = s1 - s0;
    if d > 0.0 { b0 + ((sec - s0) / d) * (b1 - b0) } else { b0 }
}

/// …and back: the source second that lands on `beat`, the inverse of the above segment for segment.
#[must_use]
pub fn source_at_beat(markers: &[Marker], beat: f64) -> f64 {
    if markers.len() < 2 {
        return beat;
    }
    let i = segment(markers, 0, beat);
    let ([b0, s0], [b1, s1]) = (markers[i], markers[i + 1]);
    let d = b1 - b0;
    if d > 0.0 {
        s0 + ((beat - b0) / d) * (s1 - s0)
    } else {
        s0
    }
}
