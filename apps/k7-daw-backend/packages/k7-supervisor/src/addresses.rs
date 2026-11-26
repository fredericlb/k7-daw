//! addresses — what the heartbeat advertises: the hostname, a coarse machine fingerprint, and the IPv4s a
//! browser on the LAN could dial; the shape is web-protocol's `EngineAddresses`. The fingerprint is a hint the
//! registry never deduplicates on. It is sha256(`host|macs`), the MACs lowercase with colons, sorted and
//! deduplicated, so an engine keeps its fingerprint across a change of supervisor.
//! What a reader gets wrong: Windows lists no interface yet (M8 adds the adapters), so it advertises none.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// One network interface as the heartbeat reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Iface {
    pub name: String,
    pub ipv4: Vec<String>,
    pub mac: String,
    pub internal: bool,
    /// Carries an IPv4 or IPv6 address: only those are fingerprinted.
    pub addressed: bool,
}

/// Interfaces that reach no browser (VPNs, container bridges, WSL, Hyper-V), matched by NAME.
pub fn phantom_interface(name: &str) -> bool {
    let n = name.to_lowercase();
    let prefixes = [
        "vethernet",
        "wsl",
        "hyper-v",
        "virtualbox",
        "vmware",
        "vmnet",
        "docker",
        "br-",
        "veth",
        "tun",
        "tap",
        "utun",
        "zerotier",
        "tailscale",
    ];
    prefixes.iter().any(|p| n.starts_with(p))
        || n.strip_prefix("wg")
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
}

fn mac_string(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(unix)]
fn interfaces() -> Vec<Iface> {
    let mut out: Vec<Iface> = Vec::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: `getifaddrs` fills `list` with a chain `freeifaddrs` releases; every pointer read below is the chain's.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return out;
    }
    let mut at = list;
    while !at.is_null() {
        // SAFETY: `at` is a live node of the chain.
        let a = unsafe { &*at };
        at = a.ifa_next;
        // Up AND running, as libuv lists them: an interface that is merely up reaches nobody.
        let flags = a.ifa_flags as libc::c_int;
        if a.ifa_addr.is_null() || flags & libc::IFF_UP == 0 || flags & libc::IFF_RUNNING == 0 {
            continue;
        }
        // SAFETY: `ifa_name` is a NUL-terminated string of the chain.
        let name = unsafe { std::ffi::CStr::from_ptr(a.ifa_name) }
            .to_string_lossy()
            .into_owned();
        let at_name = match out.iter().position(|f| f.name == name) {
            Some(i) => i,
            None => {
                out.push(Iface {
                    name,
                    ..Iface::default()
                });
                out.len() - 1
            }
        };
        let f = &mut out[at_name];
        if flags & libc::IFF_LOOPBACK != 0 {
            f.internal = true;
        }
        // SAFETY: `ifa_addr` is non-null and starts with a `sockaddr` header.
        let family = i32::from(unsafe { (*a.ifa_addr).sa_family });
        if family == libc::AF_INET6 {
            f.addressed = true;
        } else if family == libc::AF_INET {
            f.addressed = true;
            // SAFETY: an AF_INET address is a `sockaddr_in`.
            let sin = unsafe { &*a.ifa_addr.cast::<libc::sockaddr_in>() };
            f.ipv4
                .push(std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)).to_string());
        } else if let Some(mac) = link_mac(a.ifa_addr, family) {
            f.mac = mac;
        }
    }
    // SAFETY: `list` came from `getifaddrs` and is released once.
    unsafe { libc::freeifaddrs(list) };
    out
}

/// The hardware address a link-layer node carries, when it is a 6-byte one.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn link_mac(addr: *const libc::sockaddr, family: i32) -> Option<String> {
    if family != libc::AF_LINK {
        return None;
    }
    // SAFETY: an AF_LINK address is a `sockaddr_dl`, whose name (`sdl_nlen`) precedes the address in `sdl_data`.
    let sdl = unsafe { &*addr.cast::<libc::sockaddr_dl>() };
    if sdl.sdl_alen != 6 {
        return None;
    }
    // `sdl_data` is declared 12 bytes long but holds the name and the address past it, as `LLADDR` reads it.
    let base = sdl.sdl_data.as_ptr();
    let bytes: Vec<u8> = (0..6)
        // SAFETY: the kernel sized this node for `sdl_nlen + sdl_alen` bytes of data.
        .map(|i| unsafe { *base.add(usize::from(sdl.sdl_nlen) + i) } as u8)
        .collect();
    Some(mac_string(&bytes))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn link_mac(addr: *const libc::sockaddr, family: i32) -> Option<String> {
    if family != libc::AF_PACKET {
        return None;
    }
    // SAFETY: an AF_PACKET address is a `sockaddr_ll`.
    let ll = unsafe { &*addr.cast::<libc::sockaddr_ll>() };
    (ll.sll_halen == 6).then(|| mac_string(&ll.sll_addr[..6]))
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
)))]
fn link_mac(_addr: *const libc::sockaddr, _family: i32) -> Option<String> {
    None
}

#[cfg(not(unix))]
fn interfaces() -> Vec<Iface> {
    Vec::new()
}

/// This machine's name; `$STUDIO_MACHINE_NAME` first, since Android answers "localhost" for every phone.
pub fn host_name() -> String {
    if let Some(named) = std::env::var("STUDIO_MACHINE_NAME")
        .ok()
        .filter(|n| !n.is_empty())
    {
        return named;
    }
    #[cfg(unix)]
    {
        let mut buf = [0 as libc::c_char; 256];
        // SAFETY: the buffer is 256 bytes and one is kept back for the NUL.
        if unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len() - 1) } == 0 {
            // SAFETY: `gethostname` NUL-terminated within the buffer, which started zeroed.
            return unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
                .to_string_lossy()
                .into_owned();
        }
    }
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "localhost".into())
}

fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `{ hostname, machineFingerprint, port, addresses: { loopback, lanIps, port } }` over the given interfaces.
pub fn presence_of(host: &str, ifaces: &[Iface], port: u16) -> Value {
    let mut lan: Vec<String> = Vec::new();
    let mut macs: BTreeSet<&str> = BTreeSet::new();
    for f in ifaces {
        if f.addressed && !f.internal && !f.mac.is_empty() && f.mac != "00:00:00:00:00:00" {
            macs.insert(&f.mac);
        }
        if f.internal || phantom_interface(&f.name) {
            continue;
        }
        for ip in &f.ipv4 {
            // Link-local is unroutable, and loopback is advertised on its own.
            if ip.starts_with("169.254.") || ip.starts_with("127.") || ip == "0.0.0.0" {
                continue;
            }
            lan.push(ip.clone());
        }
    }
    let seed = format!("{host}|{}", macs.into_iter().collect::<Vec<_>>().join(","));
    json!({
        "hostname": host,
        "machineFingerprint": sha256_hex(&seed),
        "port": port,
        "addresses": { "loopback": "127.0.0.1", "lanIps": lan, "port": port },
    })
}

/// This machine's presence on `port`. Blocking.
pub fn presence(port: u16) -> Value {
    presence_of(&host_name(), &interfaces(), port)
}
