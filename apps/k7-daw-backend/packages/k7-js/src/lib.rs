//! k7-js — the few JavaScript number formats a file holds, reproduced natively (`number`, `math`, `bases`), and an error
//! type the settle still names; no JavaScript runs here.
//! The choices and the proofs: apps/k7/backend-rs/docs/parity.md.

pub mod bases;
pub mod math;
pub mod number;

/// An operation that failed: the JavaScript error's class name and message, or the engine's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsError {
    pub name: String,
    pub message: String,
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}

impl std::error::Error for JsError {}
