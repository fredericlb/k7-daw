//! k7-mediatek — the media core the front shares: what a file's sound and picture are made of. The waveform module in
//! Wasmtime, the analysis and its sidecar, the decodes and transcodes through ffmpeg, the video probe, the whisper
//! command lines and the `JSON.stringify` and ECMAScript coercions the answers are spelt with. No HTTP here.
//! What a reader gets wrong: everything is synchronous and for a blocking thread; the front owns the scheduling.
//! The choices and their proofs: apps/k7/backend-rs/docs/file-media.md.

pub mod analyse;
pub mod decode;
pub mod ecma;
pub mod envelope;
pub mod envelope_memo;
pub mod ffmpeg;
pub mod fingerprint;
pub mod json;
pub mod media;
pub mod programs;
pub mod sidecar;
pub mod stream_copy;
pub mod transcode;
pub mod video_copy;
pub mod video_probe;
pub mod video_read;
mod video_read_mkv;
pub mod waveform;
pub mod whisper;
pub mod write;
