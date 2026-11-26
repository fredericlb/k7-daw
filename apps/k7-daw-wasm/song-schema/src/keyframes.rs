// keyframes.rs — a shot's animated property as keys, and its value at an instant: the page's compositor asks it every frame.
// A reader would get wrong that a key's `ease` shapes the segment that LEAVES it, not the one arriving.

use serde::{Deserialize, Serialize};

/// How a segment travels from its key to the next; `hold` stays put and jumps at the next key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    #[default]
    Linear,
    In,
    Out,
    InOut,
    Hold,
}

/// One key, normalised: when (seconds from the shot's start), what, and how it leaves.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Key {
    pub t: f64,
    pub v: f64,
    #[serde(default)]
    pub ease: Ease,
}

/// The eased fraction of a segment, `u` in 0..1; quadratic, which is as much curve as a hand asks of a pan.
fn eased(ease: Ease, u: f64) -> f64 {
    match ease {
        Ease::Hold => 0.0,
        Ease::In => u * u,
        Ease::Out => u * (2.0 - u),
        Ease::InOut if u < 0.5 => 2.0 * u * u,
        Ease::InOut => 1.0 - 2.0 * (1.0 - u) * (1.0 - u),
        Ease::Linear => u,
    }
}

/// The value of `keys`, sorted by time, at `t`: held before the first and after the last, eased in between.
#[must_use]
pub fn value_at(keys: &[Key], t: f64) -> f64 {
    let Some(first) = keys.first() else {
        return 0.0;
    };
    if t <= first.t {
        return first.v;
    }
    for pair in keys.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t < b.t {
            let span = b.t - a.t;
            let u = if span > 0.0 { (t - a.t) / span } else { 1.0 };
            return a.v + (b.v - a.v) * eased(a.ease, u);
        }
    }
    keys.last().map_or(0.0, |k| k.v)
}
