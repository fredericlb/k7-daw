//! ports.rs — what a port says about the engine. Whether it is SERVED is asked by connecting, with three
//! answers: up, down, or could-not-ask (no socket to be had, which is what running out of descriptors looks
//! like, and must never read as "down"). Whether it is FREE is asked by binding, on the wildcard AND on
//! 127.0.0.1, since on macOS a wildcard bind succeeds beside another program's loopback listener.

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// How many ports from the first one a start tries, unless the port is fixed.
pub const PORT_SPAN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Probe {
    Up,
    Down,
    Unknown,
}

/// Whether `e` is the process or the system out of descriptors: no answer about the port at all.
fn out_of_sockets(e: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        matches!(e.raw_os_error(), Some(libc::EMFILE | libc::ENFILE))
    }
    #[cfg(windows)]
    {
        e.raw_os_error()
            .is_some_and(|c| crate::win::sockets::OUT_OF_SOCKETS.contains(&c))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = e;
        false
    }
}

/// A connect to `127.0.0.1:port`, bounded at ~300 ms.
pub fn probe(port: u16) -> Probe {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
        Ok(_) => Probe::Up,
        Err(e) if out_of_sockets(&e) => Probe::Unknown,
        Err(_) => Probe::Down,
    }
}

/// Something serves this loopback port: never "it is OURS".
pub fn is_serving(port: u16) -> bool {
    probe(port) == Probe::Up
}

fn binds_on(host: Ipv4Addr, port: u16) -> bool {
    // Exclusively on Windows, where a plain bind succeeds beside a listener that let its port be shared.
    #[cfg(windows)]
    return crate::win::sockets::binds_exclusive(host, port);
    #[cfg(not(windows))]
    // A socket we could not even open says nothing of the port, so it reads as free, as the C++ one did.
    std::net::TcpListener::bind((host, port)).map_or_else(|e| out_of_sockets(&e), |_| true)
}

/// Could the engine bind `port` now, on the wildcard and on loopback alike.
pub fn bindable(port: u16) -> bool {
    binds_on(Ipv4Addr::UNSPECIFIED, port) && binds_on(Ipv4Addr::LOCALHOST, port)
}

/// The ports a start tries, in order: `first` alone when `fixed`, else the next `PORT_SPAN` that exist.
pub fn candidates(first: u16, fixed: bool) -> Vec<u16> {
    let span = if fixed { 1 } else { PORT_SPAN };
    if first == 0 {
        return Vec::new();
    }
    (first..=u16::MAX).take(span).collect()
}

/// The pid listening on `port`, asked of `lsof`; None when none, or when there is no lsof to ask.
#[cfg(unix)]
pub fn listener_pid(port: u16) -> Option<u32> {
    // macOS keeps it in /usr/sbin, which an app's PATH does not list.
    let lsof = ["/usr/sbin/lsof", "/usr/bin/lsof", "lsof"]
        .into_iter()
        .find(|p| !p.starts_with('/') || std::path::Path::new(p).is_file())?;
    let out = std::process::Command::new(lsof)
        .args(["-t", "-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// The pid listening on `port`, asked of the TCP table.
#[cfg(windows)]
pub fn listener_pid(port: u16) -> Option<u32> {
    crate::win::tcp::listener_pid(port)
}
