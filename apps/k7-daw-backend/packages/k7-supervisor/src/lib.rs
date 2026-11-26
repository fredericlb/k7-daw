//! k7-supervisor — the engine machine's supervisor: starts, adopts and watches `k7-runtime`, and speaks the
//! host protocol to the app that runs it (NDJSON on stdio, or two pipes inside an app). The core owns the
//! engine's lifecycle and the snapshot; every other subsystem is a PART (`parts/`), added by one line.
//! What a reader gets wrong: the wire and the files are fixed byte for byte, since the hosts and the
//! conformance suite read them (`docs/hostProtocol.md`, `docs/supervisor.md`).

pub mod addresses;
pub mod audio_policy;
pub mod core;
pub mod crash_loop;
pub mod diagnostics;
pub mod download;
pub mod engine;
pub mod engine_client;
pub mod frames;
pub mod host_lock;
pub mod http;
pub mod identity;
pub mod instruments;
pub mod log;
pub mod machine;
pub mod parts;
pub mod paths;
pub mod performance;
pub mod pidfile;
pub mod ports;
pub mod probe;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod run;
pub mod separation;
pub mod settings;
pub mod slots;
pub mod trust;
pub mod tunnel;
pub mod urls;
pub mod win;
pub mod wire;
