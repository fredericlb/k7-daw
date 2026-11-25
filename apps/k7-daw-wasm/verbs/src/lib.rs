// lib.rs — k7's verbs in Rust: a command line run over a project's files, answering its lines, its refusal and its effects.
// A reader would get wrong that a verb writes the disk: it writes the `Fs` it is given, whose writes the caller lands (docs/verbs-port.md).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod api;
pub mod catalogue;
pub mod complete;
pub mod host;
pub mod run;
mod verbs;

pub use api::{Answer, Replay, VerbRequest, run_answered, run_in_env, run_replayed, run_request, run_request_cached};
pub use complete::{CompleteRequest, Item, complete_in_env, complete_request};
pub use host::{Failed, Host, Thrown};
pub use run::run_line;
pub use verbs::ported;
