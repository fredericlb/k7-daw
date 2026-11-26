//! k7-runtime-server — the engine's WebSocket server in Rust: the wire k7, the previews and the DJ desk speak,
//! over the C++ Tracktion host it reaches through one C ABI (`engine/capi/k7_engine.h` in k7-runtime).
//! `engine` is that ABI made safe, `wire` what goes on the socket; each later part is one more module here.
//! What a reader gets wrong: the wire is the C++ server's byte for byte, nlohmann's sorted keys and number
//! format included, since every client and the conformance suite read it (`docs/rust-engine-server-plan.md`).

pub mod engine;
pub mod entry;
pub mod stub;
pub mod wire;
