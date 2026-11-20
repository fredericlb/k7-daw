//! warp.rs — a clip's stretch map as a pair of lookups: `beatAtSource` and `sourceAtBeat` of `warpPoints.ts`, bisected (2421).
//! What a reader gets wrong: the TypeScript walks "the first segment with x <= the next marker"; a bisect lands there only on a non-decreasing axis.

use std::cell::Cell;

thread_local! {
    // The markers read by this thread's segment searches, so a test counts steps instead of timing them (2421).
    static PROBES: Cell<usize> = const { Cell::new(0) };
}

/// The markers this thread's segment searches have read since the last call; the counter starts again at zero.
pub fn take_probes() -> usize {
    PROBES.with(|c| c.replace(0))
}

fn probe() {
    PROBES.with(|c| c.set(c.get() + 1));
}

/// One marker of the map: `(timeline beat, source second)`.
pub type Marker = (f64, f64);

fn beat_in(m: &[Marker], i: usize, sec: f64) -> f64 {
    let ((b0, s0), (b1, s1)) = (m[i], m[i + 1]);
    let d = s1 - s0;
    if d > 0.0 {
        b0 + ((sec - s0) / d) * (b1 - b0)
    } else {
        b0
    }
}

fn source_in(m: &[Marker], i: usize, beat: f64) -> f64 {
    let ((b0, s0), (b1, s1)) = (m[i], m[i + 1]);
    let d = b1 - b0;
    if d > 0.0 {
        s0 + ((beat - b0) / d) * (s1 - s0)
    } else {
        s0
    }
}

/// The segment the TypeScript's walk picks on `axis`: the first whose far end is at or past `x`, else the last.
fn walked(m: &[Marker], axis: fn(&Marker) -> f64, x: f64) -> usize {
    if x <= axis(&m[0]) {
        return 0;
    }
    (0..m.len() - 1)
        .find(|&i| {
            probe();
            x <= axis(&m[i + 1])
        })
        .unwrap_or(m.len() - 2)
}

/// The same segment by bisection; `m` must be non-decreasing on `axis` and NaN-free there.
fn bisected(m: &[Marker], axis: fn(&Marker) -> f64, x: f64) -> usize {
    if x.is_nan() {
        return m.len() - 2;
    }
    if x <= axis(&m[0]) {
        return 0;
    }
    m[1..]
        .partition_point(|k| {
            probe();
            axis(k) < x
        })
        .min(m.len() - 2)
}

fn by_beat(k: &Marker) -> f64 {
    k.0
}

fn by_source(k: &Marker) -> f64 {
    k.1
}

/// `beatAtSource`, walked exactly as the TypeScript does.
pub fn beat_at_source(m: &[Marker], sec: f64) -> f64 {
    if m.len() < 2 {
        sec
    } else {
        beat_in(m, walked(m, by_source, sec), sec)
    }
}

/// `sourceAtBeat`, walked exactly as the TypeScript does.
pub fn source_at_beat(m: &[Marker], beat: f64) -> f64 {
    if m.len() < 2 {
        beat
    } else {
        source_in(m, walked(m, by_beat, beat), beat)
    }
}

/// A map with its two axes checked once; each lookup bisects its axis when it is ordered, else walks.
#[derive(Clone, Debug)]
pub struct WarpMap {
    markers: Vec<Marker>,
    sources_ordered: bool,
    beats_ordered: bool,
}

fn ordered(m: &[Marker], axis: fn(&Marker) -> f64) -> bool {
    // `!(a <= b)` style: a NaN on the axis reads as unordered.
    m.windows(2).all(|w| axis(&w[0]) <= axis(&w[1]))
}

impl WarpMap {
    pub fn new(markers: &[Marker]) -> WarpMap {
        WarpMap {
            markers: markers.to_vec(),
            sources_ordered: ordered(markers, by_source),
            beats_ordered: ordered(markers, by_beat),
        }
    }

    pub fn beat_at_source(&self, sec: f64) -> f64 {
        let m = &self.markers;
        if m.len() < 2 {
            return sec;
        }
        let i = if self.sources_ordered {
            bisected(m, by_source, sec)
        } else {
            walked(m, by_source, sec)
        };
        beat_in(m, i, sec)
    }

    pub fn source_at_beat(&self, beat: f64) -> f64 {
        let m = &self.markers;
        if m.len() < 2 {
            return beat;
        }
        let i = if self.beats_ordered {
            bisected(m, by_beat, beat)
        } else {
            walked(m, by_beat, beat)
        };
        source_in(m, i, beat)
    }

    /// Whether the source axis is bisected, for a test that wants to know which path it took.
    pub fn bisects_sources(&self) -> bool {
        self.sources_ordered
    }

    pub fn bisects_beats(&self) -> bool {
        self.beats_ordered
    }
}
