//! The supervisor's tests, one binary: the behaviours the C++ suite held (`apps/k7-daw-engine/k7-supervisor/tests`),
//! replayed against the Rust supervisor, and the program run end to end against a fake engine.

mod addresses;
mod audio_policy;
#[path = "../support/blob_server.rs"]
mod blob_server;
mod engine;
mod engine_client;
mod files;
mod instruments;
mod instruments_part;
mod onnx_model;
mod parts;
mod performance;
mod probe;
mod protocol;
mod queries;
mod registry;
mod rules;
mod scratch;
mod separation;
mod separation_cut;
#[path = "../support/server.rs"]
mod server;
mod settings;
mod supervisor;
mod tls;
mod urls;
