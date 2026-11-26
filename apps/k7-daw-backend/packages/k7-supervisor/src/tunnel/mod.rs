//! tunnel — the engine machine's end of the engine tunnel, the agent half of `engine-tunnel` in Rust. The hub
//! is the front's (`k7-engine`'s `tunnel/`), whose frames this reuses; the path map, the file requests, the
//! per-channel queues and the backpressure marks are this crate's. The jobs a jobs channel may ask are a table
//! (`jobs::Jobs`) the tunnel PART fills: the seam M5 plugs its jobs into (`docs/tunnel.md`, beside this crate).

pub mod agent;
pub mod channel;
pub mod files;
pub mod filetime;
pub mod flag;
pub mod jobs;
pub mod ledger;
pub mod link;
pub mod path_map;
pub mod queue;
pub mod segments;
pub mod shed;
pub mod wire;

pub use agent::{Options, TunnelAgent};
pub use jobs::{Emit, JobContext, Jobs};
