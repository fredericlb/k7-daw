// lib.rs — what ui-eyes, the page's WebGL compositor, takes and asks: its scene and the rules the scene's picture follows.
// A reader would get wrong that nothing here draws: the per-frame work stays TypeScript, and this crate hands it tables (docs/eyes.md).

pub mod cameras;
pub mod cube;
pub mod rect;
pub mod scene;
pub mod schedule;
pub mod transitions;

pub use scene::*;

/// `Math.max` over two numbers: a NaN wins, as in JavaScript.
pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) }
}

/// `Math.min` over two numbers: a NaN wins, as in JavaScript.
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) }
}
