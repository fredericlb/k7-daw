//! separation — splitting a mix into stems on this machine: decode it, run HT-Demucs window by window on ONNX
//! Runtime, fold the overlapping windows back, write 24-bit WAVs. A job is keyed by its WORK, so a second request
//! for the same cut joins the one running rather than starting another. The tunnel's `separate` job (M4) calls
//! `run`; `--separate` calls it from a terminal. What a reader gets wrong: nothing here touches the core, a cut is
//! a thread of its own (`docs/separation.md`).

pub mod audio;
pub mod budget;
pub mod cli;
mod cut;
pub mod decode;
mod job;
pub mod onnx;
pub mod slots;

pub use budget::cuts_in_flight;
pub use job::{Request, run, terminate_all};
