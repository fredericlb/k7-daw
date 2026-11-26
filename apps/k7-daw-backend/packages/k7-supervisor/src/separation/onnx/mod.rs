//! onnx — one HT-Demucs session on ONNX Runtime, on the first execution provider that builds, and which one won
//! is said. ONNX Runtime is LOADED at the first cut, never linked (feature `dynamic-ort`): a supervisor missing it
//! still starts, runs the engine and holds the tunnel, and only the job says what is missing. Android ships no
//! binary, so every open there fails with a sentence saying so (`docs/separation.md`).

pub mod locate;

#[cfg(not(target_os = "android"))]
mod real;
#[cfg(not(target_os = "android"))]
pub use real::{OnnxSession, available};

#[cfg(target_os = "android")]
mod absent;
#[cfg(target_os = "android")]
pub use absent::{OnnxSession, available};

#[cfg(windows)]
mod dml;
