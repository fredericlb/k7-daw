//! The supervisor's tests, one binary: the behaviours the C++ suite held (`apps/k7-daw-engine/k7-supervisor/tests`),
//! replayed against the Rust supervisor, and the program run end to end against a fake engine.

mod engine;
mod files;
mod onnx_model;
mod parts;
mod protocol;
mod rules;
mod scratch;
mod separation;
mod separation_cut;
mod settings;
mod supervisor;
