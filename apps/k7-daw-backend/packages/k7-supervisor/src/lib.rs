//! k7-supervisor — the engine machine's supervisor: starts, adopts and watches `k7-runtime`, and speaks the
//! host protocol to the app that runs it (NDJSON on stdio, or two pipes inside an app). The core owns the
//! engine's lifecycle and the snapshot; every other subsystem is a PART (`parts/`), added by one line.
//! What a reader gets wrong: the wire and the files are the C++ supervisor's, byte for byte, since the
//! hosts and the conformance suite read them (`docs/rust-supervisor-plan.md` at the repo root).

pub mod audio_policy;
pub mod core;
pub mod crash_loop;
pub mod diagnostics;
pub mod engine;
pub mod engine_client;
pub mod frames;
pub mod host_lock;
pub mod identity;
pub mod log;
pub mod parts;
pub mod paths;
pub mod pidfile;
pub mod ports;
pub mod process;
pub mod protocol;
pub mod run;
pub mod separation;
pub mod settings;
pub mod wire;
