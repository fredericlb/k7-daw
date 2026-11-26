//! The tunnel's hub, judged end to end: the testbed (the front with its engine core) in a process of its own, the
//! machine's real agent (`k7-supervisor`) dialling it, a fake engine behind the agent and clients on bound paths.
//! The cases are the ones `engine-tunnel`'s TypeScript suites held, over the Rust agent alone (`docs/engine.md`).

mod bed;
mod hub_confinement;
mod hub_drain;
mod hub_jobs;
mod hub_lifecycle;
mod hub_protocol;
mod hub_tunnel;
mod hub_uploads;
