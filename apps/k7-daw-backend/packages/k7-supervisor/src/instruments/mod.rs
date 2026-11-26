//! instruments — the sampled instruments this machine's engine plays, fetched by the SUPERVISOR rather than a
//! server: hundreds of MB each, straight from their origin onto the machine that plays them, once, pinned by
//! hash. A module names one as `@instruments/<name>/<file>.sfz`, which the engine resolves under
//! `<state>/instruments`. The archive is verified BEFORE it is unpacked and a folder counts as installed only
//! once its marker is written last (`docs/instruments.md`); the queue and the commands are `parts/instruments`.

mod catalogue;
mod install;
mod store;

pub use catalogue::{Instrument, all, find};
pub use install::{Phase, Progress, install};
pub use store::{installed, present, remove, sha256_of, unpack_zip};
