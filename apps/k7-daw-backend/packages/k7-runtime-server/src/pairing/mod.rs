//! pairing — who may drive the engine over the LAN: the address and origin rules (`net`), the paired-devices
//! store (`store`) and the gate every message passes (`gate`).
//! Nothing here owns a socket: the socket core hands the gate an id and acts on what it returns.

pub mod gate;
pub mod net;
pub mod store;

/// A connected socket as the socket core numbers it; the gate keys trust on it and never dereferences anything.
pub type SocketId = u64;
