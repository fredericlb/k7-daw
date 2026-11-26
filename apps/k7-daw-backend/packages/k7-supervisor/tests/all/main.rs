//! The supervisor's tests, one binary: the behaviours the C++ suite held (`apps/k7-daw-engine/k7-supervisor/tests`),
//! replayed against the Rust supervisor, and the program run end to end against a fake engine.

mod audio_policy;
mod engine;
mod engine_client;
mod files;
mod onnx_model;
mod parts;
mod protocol;
mod queries;
mod rules;
mod scratch;
mod separation;
mod separation_cut;
mod settings;
mod supervisor;
