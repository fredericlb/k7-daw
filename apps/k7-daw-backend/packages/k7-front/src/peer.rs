//! peer.rs — who the client is, decided once here: `trust proxy` arithmetic over `X-Forwarded-For`. A forwarded
//! address never makes a client loopback: only the socket can (`docs/front.md`).

use std::net::IpAddr;

fn unmapped(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

fn is_loopback(text: &str) -> bool {
    text.parse::<IpAddr>()
        .is_ok_and(|ip| unmapped(ip).is_loopback())
}

/// The client as Express's `req.ip` reads it with `trust proxy` = `hops`, a forwarded loopback address aside.
pub fn client_address(peer: IpAddr, forwarded_for: Option<&str>, hops: usize) -> String {
    let mut chain = vec![canonical(peer)];
    if let Some(list) = forwarded_for {
        chain.extend(
            list.split(',')
                .rev()
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .map(String::from),
        );
    }
    let client = chain[hops.min(chain.len() - 1)].clone();
    // Only the socket proves this machine: a header naming loopback from elsewhere is a client's lie.
    if is_loopback(&client) && !unmapped(peer).is_loopback() {
        return chain[0].clone();
    }
    client
}

/// An IPv4 client on a dual-stack socket keeps Node's `::ffff:` spelling, which the backend's checks already read.
pub(crate) fn canonical(peer: IpAddr) -> String {
    match peer {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => format!("::ffff:{v4}"),
            None => v6.to_string(),
        },
        IpAddr::V4(v4) => v4.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_hops_as_express_does() {
        let peer: IpAddr = "10.0.0.2".parse().unwrap();
        assert_eq!(client_address(peer, None, 0), "10.0.0.2");
        assert_eq!(
            client_address(peer, Some("1.1.1.1, 2.2.2.2"), 0),
            "10.0.0.2"
        );
        assert_eq!(client_address(peer, Some("1.1.1.1, 2.2.2.2"), 1), "2.2.2.2");
        assert_eq!(client_address(peer, Some("1.1.1.1, 2.2.2.2"), 2), "1.1.1.1");
        assert_eq!(client_address(peer, Some("1.1.1.1"), 5), "1.1.1.1");
    }

    #[test]
    fn a_forwarded_loopback_is_never_believed() {
        let peer: IpAddr = "172.18.0.5".parse().unwrap();
        assert_eq!(client_address(peer, Some("127.0.0.1"), 1), "172.18.0.5");
        assert_eq!(client_address(peer, Some("::1"), 1), "172.18.0.5");
        assert_eq!(
            client_address(peer, Some("::ffff:127.0.0.1"), 1),
            "172.18.0.5"
        );
        let local: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(client_address(local, Some("127.0.0.1"), 1), "127.0.0.1");
        assert_eq!(
            client_address(peer, Some("198.51.100.7"), 1),
            "198.51.100.7"
        );
        // Behind a loopback proxy (the dev Vite) a LAN client stays its LAN address, never this machine.
        assert_eq!(
            client_address(local, Some("192.168.87.10"), 1),
            "192.168.87.10"
        );
    }
}
