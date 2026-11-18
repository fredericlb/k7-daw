//! k7-hub — the file hub's pieces (phase 5a, `docs/rust-phase5-plan.md`): the tickets, the account's names and
//! environment, the engine bindings, the seam to the tunnel hub and the cache sweep. Choices: `apps/k7/backend-rs/docs/files.md`.
//! What a reader gets wrong: no process starts here; every account's files are the front's own runtime.

pub mod account;
pub mod binding;
pub mod env;
mod http;
pub mod json;
pub mod seam;
pub mod stale;
pub mod ticket;

pub use account::Log;
pub use http::call;

/// Lowercase hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
