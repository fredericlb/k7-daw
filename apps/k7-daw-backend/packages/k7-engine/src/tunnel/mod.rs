//! tunnel — the hub half of `apps/k7-daw-frontend/packages/engine-tunnel`: the tunnels supervisors dial in, the channels they carry,
//! the files those channels name, and the loopback paths bound to an engine. The agent half is the C++
//! supervisor's (`apps/k7-daw-engine/k7-supervisor/src/TunnelAgent.cpp`).

pub mod backpressure;
pub mod channel;
pub mod files;
pub mod frames;
pub mod hub;
pub mod hub_tunnel;
pub mod local;
mod put;
pub mod spelling;
pub mod uploads;
