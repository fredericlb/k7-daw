//! sockets — whether a port can be bound EXCLUSIVELY. Windows lets a bind succeed beside a listener that
//! asked to share its port, so a plain bind would call a port free that the engine's own bind would be
//! refused: `SO_EXCLUSIVEADDRUSE` makes the question the engine's.

use std::net::Ipv4Addr;

use windows::Win32::Networking::WinSock::{
    AF_INET, INVALID_SOCKET, IPPROTO_TCP, SO_EXCLUSIVEADDRUSE, SOCK_STREAM, SOCKADDR, SOCKADDR_IN,
    SOL_SOCKET, WSADATA, WSAStartup, bind, closesocket, setsockopt, socket,
};

/// WSAEMFILE and WSAENOBUFS: the socket layer out of descriptors, which says nothing of a port.
pub const OUT_OF_SOCKETS: [i32; 2] = [10024, 10055];

/// Whether a fresh socket binds `port` on `host` exclusively now; a socket we cannot open says yes.
pub fn binds_exclusive(host: Ipv4Addr, port: u16) -> bool {
    let mut data = WSADATA::default();
    // SAFETY: Winsock's startup is counted and may be repeated; `data` is its out parameter.
    unsafe {
        if WSAStartup(0x0202, &mut data) != 0 {
            return true;
        }
        let Ok(s) = socket(i32::from(AF_INET.0), SOCK_STREAM, IPPROTO_TCP.0) else {
            return true;
        };
        if s == INVALID_SOCKET {
            return true;
        }
        let one = 1i32.to_ne_bytes();
        setsockopt(s, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, Some(&one));
        let mut addr = SOCKADDR_IN {
            sin_family: AF_INET,
            sin_port: port.to_be(),
            ..SOCKADDR_IN::default()
        };
        addr.sin_addr.S_un.S_addr = u32::from(host).to_be();
        let ok = bind(
            s,
            (&raw const addr).cast::<SOCKADDR>(),
            std::mem::size_of::<SOCKADDR_IN>() as i32,
        ) == 0;
        closesocket(s);
        ok
    }
}
