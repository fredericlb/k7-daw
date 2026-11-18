//! k7-local — a machine's own engine as its own backend runs it: `engine/k7-supervisor` spawned over NDJSON stdio,
//! relaunched on a backoff, stopped with `shutdown` and its pipe, and its engine killed only on pidfile proof.
//! It speaks no HTTP: `k7-front`'s `/api/local*` routes are its pages' way in (`apps/k7/backend-rs/docs/local.md`).
//! What a reader gets wrong: on Windows the supervisor makes its own kill-on-close job and hands the host its only handle.

pub mod engine;
pub mod host;
pub mod pidfile;
pub mod protocol;
pub mod settings;
pub mod tray_status;

pub use engine::{LocalEngine, State};
pub use host::Backoff;

use serde_json::Value;

/// JavaScript's truthiness of a JSON value.
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
