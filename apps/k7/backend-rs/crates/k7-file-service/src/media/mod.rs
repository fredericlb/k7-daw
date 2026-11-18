//! media — the account's waveforms, downloads, takes and prefetch as pure code (sub-step 1l, 2400 to 2499): the slicing, the
//! answer and its signature, the onset corrections, the model registry's machine and the names. No I/O, no clock of its own.
//! What a reader gets wrong: a number is the TypeScript's f64 and a count is the TypeScript's `Math.*`; apps/k7/backend-rs/docs/file-media.md.

use std::time::Duration;

pub mod answer;
pub mod models;
pub mod names;
pub mod onsets;
pub mod region;
pub mod warp;

/// The route's bucket bounds and default.
pub const BUCKETS_MIN: f64 = 16.0;
pub const BUCKETS_MAX: f64 = 4000.0;
pub const BUCKETS_DEFAULT: f64 = 1000.0;
/// The route's sensitivity bounds.
pub const SENSITIVITY_MIN: f64 = 0.25;
pub const SENSITIVITY_MAX: f64 = 8.0;
/// Decodes at once for an account's prewarm.
pub const WARM_DECODES: usize = 2;
/// A finished download is kept this long for a poll to read.
pub const KEEP: Duration = Duration::from_secs(10 * 60);
/// How long a take written in an engine's mirror may take to reach the project.
pub const LANDING: Duration = Duration::from_secs(20);
/// A grid of more lines than this is refused (2423).
pub const MAX_GRID_LINES: usize = 100_000;

/// `Number(text)` of a query value; an absent one is NaN.
fn number(raw: Option<&str>) -> f64 {
    raw.map_or(f64::NAN, |t| {
        song::lanes::jsvalue::to_number(&serde_json::Value::from(t))
    })
}

/// The route's `buckets`: `Number(raw) || 1000` held to 16..4000, then cut to a whole number (2427, DEPARTS).
pub fn clamp_buckets(raw: Option<&str>) -> usize {
    let n = number(raw);
    let n = if n == 0.0 || n.is_nan() {
        BUCKETS_DEFAULT
    } else {
        n
    };
    n.clamp(BUCKETS_MIN, BUCKETS_MAX).trunc() as usize
}

/// The route's `sensitivity`: `Number(raw)` held to 0.25..8 when it is finite, else none.
pub fn clamp_sensitivity(raw: Option<&str>) -> Option<f64> {
    let n = number(raw);
    n.is_finite()
        .then(|| n.clamp(SENSITIVITY_MIN, SENSITIVITY_MAX))
}

/// The route's `whole`: only `1` and `true` ask for the material's whole span.
pub fn is_whole(raw: Option<&str>) -> bool {
    matches!(raw, Some("1" | "true"))
}
