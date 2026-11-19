//! k7-local — a machine's own engine as its own backend runs it: `engine/k7-supervisor` spawned over NDJSON stdio,
//! relaunched on a backoff, stopped with `shutdown` and its pipe, and its engine killed only on pidfile proof.
//! It speaks no HTTP: `k7-front`'s `/api/local*` routes are its pages' way in (`apps/k7/backend-rs/docs/local.md`).
//! What a reader gets wrong: on Windows the supervisor makes its own kill-on-close job and hands the host its only handle.

pub mod engine;
pub mod host;
pub mod pidfile;
pub mod protocol;
pub mod settings;

pub use engine::{LocalEngine, State};
pub use host::Backoff;
