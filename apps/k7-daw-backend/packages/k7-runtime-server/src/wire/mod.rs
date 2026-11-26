//! wire — what crosses the engine's socket: the JSON as the C++ server printed it, the audio frame, the
//! verbs and replies by name, and the version this engine states.

pub mod catalogue;
pub mod json;
pub mod stream;
pub mod version;
