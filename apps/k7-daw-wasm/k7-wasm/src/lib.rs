// lib.rs — the browser's one WebAssembly module: the C ABI (abi.rs) over a plain-Rust dispatch (ops.rs).
// A reader would get wrong that only abi.rs touches the ABI: everything else is ordinary Rust a native host can call.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod abi;
pub mod curve_ops;
#[cfg(feature = "song")]
pub mod edit_ops;
pub mod engine_ops;
pub mod error;
pub mod gesture_ops;
#[cfg(feature = "song")]
pub mod host_fs;
pub mod ops;
#[cfg(feature = "song")]
pub mod song_ops;
#[cfg(test)]
mod tests;
pub mod typed;
pub mod typed_editors;
pub mod typed_ticks;
pub mod typed_timeline;
#[cfg(feature = "verbs")]
pub mod verb_ops;

pub use error::{ErrorCode, WasmError};
pub use ops::{Request, Session};
