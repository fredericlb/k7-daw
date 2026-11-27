// lib.rs — a song's value vocabulary in Rust: the timecode, a curve resolved against a time signature, and the page's engine rules (`engine`).
// A reader would get wrong that this crate holds no FFI and no JSON text: the browser facade (apps/k7-daw-frontend/packages/k7-wasm) owns both.
// The ts feature derives the TypeScript types of what crosses the facade; the native backend never builds it.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod address;
mod aspect;
mod audio_files;
mod curve;
mod curve_spec;
pub mod engine;
mod error;
mod keyframes;
mod preview;
mod render_settings;
mod stems;
#[cfg(test)]
mod tests;
mod text_hash;
mod timecode;
mod warp;

pub use aspect::{ASPECTS, Aspect, aspect_of};
pub use audio_files::{ext_of, is_audio};
pub use curve::{
    ADSR_DEFAULT_LEVEL, Adsr, CURVE_CUTS, Curve, Cut, Kind, LFO_SHAPES, Point, Points, Polarity, Shape, cut_output,
};
pub use curve_spec::{AdsrSpec, CurveSpec, LfoSpec, PointSpec, PointsSpec};
pub use error::SchemaError;
pub use keyframes::{Ease, Key, value_at};
pub use preview::{BeatLine, CurvePreview, Span, XY, playhead, preview};
pub use render_settings::{
    EXPORT_FORMATS, EXPORT_QUALITIES, ExportFormat, ExportQuality, RenderScene, ResolvedVideo, VideoSettings,
    VideoSize, parse_video_settings, resolve_video_settings,
};
pub use stems::{
    STEM_NEUTRAL, STEM_RANGE, STEMS, STEMS_DIR, Stem, deck_platter, stem_control, stem_file, stem_lane, stem_lane_of,
    stem_platter,
};
pub use text_hash::text_hash;
pub use timecode::{
    TICKS_PER_QUARTER, TIMECODE_PATTERN, TimeSig, beats_to_duration, beats_to_duration_on, beats_to_position,
    beats_to_position_on, is_timecode, parse_duration, parse_duration_on, parse_position, parse_position_on,
    round_beat,
};
pub use warp::{Marker, beat_at_source, source_at_beat};
