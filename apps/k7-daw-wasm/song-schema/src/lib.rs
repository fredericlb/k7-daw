// lib.rs — a song's value vocabulary in Rust: the timecode, a curve resolved against a time signature, and the page's engine rules (`engine`).
// A reader would get wrong that this crate holds no FFI and no JSON text: the browser facade (apps/k7-daw-frontend/packages/k7-wasm) owns both.
// The ts feature derives the TypeScript types of what crosses the facade; the native backend never builds it.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod curve;
mod curve_spec;
pub mod engine;
mod error;
mod preview;
#[cfg(test)]
mod tests;
mod text_hash;
mod timecode;

pub use curve::{Adsr, Curve, Cut, Kind, Point, Points, Polarity, Shape, cut_output};
pub use curve_spec::{AdsrSpec, CurveSpec, LfoSpec, PointSpec, PointsSpec};
pub use error::SchemaError;
pub use preview::{BeatLine, CurvePreview, Span, XY, playhead, preview};
pub use text_hash::text_hash;
pub use timecode::{
    TICKS_PER_QUARTER, TIMECODE_PATTERN, TimeSig, beats_to_duration, beats_to_duration_on, beats_to_position,
    beats_to_position_on, is_timecode, parse_duration, parse_duration_on, parse_position, parse_position_on,
    round_beat,
};
