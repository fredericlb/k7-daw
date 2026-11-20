// crossfade.rs — a CROSSFADE between two neighbouring events of a lane, planned: they overlap by its length and each fades over it.
// A reader would get wrong the order: B's start is pulled earlier first, then A's end later, so what either plays outside it never changes.

use crate::start_time::above;
/// One of the two events, as the compile resolved it: beats on the song, seconds in its file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossfadeSide {
    pub start: f64,
    pub length: f64,
    pub from: f64,
    pub to: Option<f64>,
    pub mat_from: f64,
    pub mat_to: Option<f64>,
}

/// What to write: B's new start and window start, A's new window end (unchanged when absent) and the length that fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossfadePlan {
    pub b_start: f64,
    pub b_from: f64,
    pub a_to: Option<f64>,
    pub length: f64,
}

/// Makes A and B overlap by `length` beats, B moving first; None when there is no material to overlap with.
#[must_use]
pub fn plan_crossfade(a: &CrossfadeSide, b: &CrossfadeSide, length: f64, tempo: f64) -> Option<CrossfadePlan> {
    let spb = 60.0 / tempo;
    let overlap = a.start + a.length - b.start;
    let need = length - overlap;
    let mut from_b = need;
    let mut from_a = 0.0;
    if need > 0.0 {
        // B opens earlier as far as its clip reaches, then A opens later for the rest.
        let room_b = ((b.from - b.mat_from) / spb).max(0.0);
        from_b = need.min(room_b);
        let a_end = a.to.or(a.mat_to);
        let room_a = match (a.mat_to, a_end) {
            (Some(m), Some(e)) => ((m - e) / spb).max(0.0),
            _ => 0.0,
        };
        from_a = (need - from_b).min(room_a);
    }
    let fit = overlap + from_b + from_a;
    if !above(fit, 1e-6) {
        return None;
    }
    let a_end = a.to.or(a.mat_to);
    Some(CrossfadePlan {
        b_start: b.start - from_b,
        b_from: b.from - from_b * spb,
        a_to: a_end.filter(|_| from_a > 0.0).map(|e| e + from_a * spb),
        length: fit,
    })
}
