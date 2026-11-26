//! win — what only Windows does for the supervisor: the kill-on-close job the host holds, the TCP table's
//! listener, the crash minidump, the adapters and the wide hostname, the memory, the trust store's `Disallowed` and server-auth
//! rules, DirectML's discrete adapter. The DECISIONS are plain code that runs everywhere (`tcp`, `certs`,
//! `dml_choice`); only the calls that ask Windows are `cfg(windows)`, and these files name nothing of the crate.

pub mod certs;
pub mod crash;
pub mod dml_choice;
pub mod tcp;

#[cfg(windows)]
pub mod dxgi;
#[cfg(windows)]
pub mod job;
#[cfg(windows)]
pub mod net;
#[cfg(windows)]
pub mod sockets;
#[cfg(windows)]
pub mod sys;

/// One network adapter as Windows lists it, before `addresses` turns it into an interface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Adapter {
    pub name: String,
    pub ipv4: Vec<std::net::Ipv4Addr>,
    pub mac: Vec<u8>,
    pub loopback: bool,
    pub addressed: bool,
}
