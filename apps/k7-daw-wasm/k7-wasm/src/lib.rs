// lib.rs — the browser's one WebAssembly module: the C ABI (abi.rs) over a plain-Rust dispatch split by family (ops/).
// A reader would get wrong that only abi.rs touches the ABI: everything else is ordinary Rust a native host can call.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod abi;
pub mod error;
pub mod ops;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_file;
#[cfg(test)]
mod tests_rules;
mod tests_k7x;

pub use error::{ErrorCode, WasmError};
pub use ops::Session;
