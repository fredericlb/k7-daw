//! wire — what crosses the engine's socket: the JSON as nlohmann prints it, the audio frame, the
//! verbs and replies by name, and the version this engine states.

pub mod catalogue;
pub mod frames;
pub mod json;
#[cfg(feature = "ts")]
pub(crate) mod rec;
pub mod stream;
pub mod version;
