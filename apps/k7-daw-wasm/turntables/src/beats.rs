// beats.rs — where a song's beat grid actually STARTS, and how far one deck must move to put its beats on the other's.
// A reader would get wrong that the phase is a circular mean of every onset, not the first transient (docs/turntables/bpm.md).

use serde::{Deserialize, Serialize};

/// Where a song's beats fall, and whether the onsets actually said so.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BeatGrid {
    /// Where the first beat falls, in source seconds, inside `[0, 60/bpm)`.
    pub phase: f64,
    /// 0 when the onsets say nothing about a grid, about 1 when they all land on one.
    pub strength: f64,
    /// Is `phase` a measurement or a shrug? The Rayleigh test decides.
    pub confident: bool,
}

/// p ≈ 0.01 for the Rayleigh statistic `n·R²`.
const RAYLEIGH_Z: f64 = 4.6;

/// Nothing to go on: a grid at zero, and saying so.
pub const NO_GRID: BeatGrid = BeatGrid {
    phase: 0.0,
    strength: 0.0,
    confident: false,
};

/// Where the beat grid starts: a circular mean of every onset.
pub fn beat_phase(transients: &[f64], bpm: f64) -> BeatGrid {
    if !(bpm > 0.0) || transients.is_empty() {
        return NO_GRID;
    }
    let period = 60.0 / bpm;
    if !(period > 0.0) || !period.is_finite() {
        return NO_GRID;
    }
    let (mut x, mut y) = (0.0, 0.0);
    for t in transients {
        let angle = (2.0 * std::f64::consts::PI * t) / period;
        x += angle.cos();
        y += angle.sin();
    }
    let n = transients.len() as f64;
    let strength = x.hypot(y) / n;
    // atan2 gives (-π, π]; the grid is a position in [0, period), so bring it round.
    let mut phase = ((y.atan2(x) / (2.0 * std::f64::consts::PI)) * period + period) % period;
    // A hair below zero wraps to a whole period, and that number becomes a clip's `start`.
    if period - phase < period * 1e-9 {
        phase = 0.0;
    }
    BeatGrid {
        phase,
        strength,
        confident: n * strength * strength > RAYLEIGH_Z,
    }
}

/// What a deck's grid says about where it is: a tempo and where its first beat falls.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct GridOf {
    pub natural: f64,
    #[serde(default)]
    pub phase: Option<f64>,
}

/// Where each deck's needle is, in source seconds.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Needles {
    pub master: f64,
    pub slave: f64,
}

/// Position in beats since the deck's own first downbeat.
fn beats_into(deck: GridOf, needle: f64) -> f64 {
    ((needle - deck.phase.unwrap_or(0.0)) * deck.natural) / 60.0
}

/// Where to seek the slave so its beats land on the master's, or None when a grid is unknown.
pub fn align_seek(master: GridOf, slave: GridOf, at: Needles) -> Option<f64> {
    if !(master.natural > 0.0) || !(slave.natural > 0.0) {
        return None;
    }
    master.phase?;
    slave.phase?;
    // The shorter way round: a platter can be pulled back as easily as pushed on.
    let off = beats_into(master, at.master) - beats_into(slave, at.slave);
    let wrapped = off - crate::js_round(off);
    let moved = at.slave + (wrapped * 60.0) / slave.natural;
    // Never before the record starts: half a beat earlier than zero is a place a needle cannot go.
    Some(if moved >= 0.0 {
        moved
    } else {
        moved + 60.0 / slave.natural
    })
}
