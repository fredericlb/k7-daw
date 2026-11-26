//! The supervisor's tests, one binary: the behaviours the C++ suite held (`apps/k7-daw-engine/k7-supervisor/tests`),
//! replayed against the Rust supervisor, and the program run end to end against a fake engine.

mod addresses;
mod engine;
mod files;
mod parts;
mod performance;
mod probe;
mod protocol;
mod registry;
mod rules;
mod scratch;
#[path = "../support/server.rs"]
mod server;
mod settings;
mod supervisor;
mod tls;
mod urls;
