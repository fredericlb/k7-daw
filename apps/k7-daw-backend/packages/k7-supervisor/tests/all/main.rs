//! The supervisor's tests, one binary: the behaviours the C++ suite held (`apps/k7-daw-engine/k7-supervisor/tests`),
//! replayed against the Rust supervisor, and the program run end to end against a fake engine.

mod addresses;
mod audio_policy;
mod engine;
mod engine_client;
mod files;
#[path = "../support/hub.rs"]
mod hub;
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
mod tunnel_agent;
mod tunnel_files;
mod tunnel_paths;
mod tunnel_queue;
mod tunnel_tls;
mod urls;
