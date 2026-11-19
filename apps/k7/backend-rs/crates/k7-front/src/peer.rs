//! peer.rs — who the client is, decided once here: `trust proxy` arithmetic over `X-Forwarded-For`, and the
//! forwarded headers a client cannot forge. A forwarded address never makes a client loopback: only the socket can
//! (`docs/front.md`).

use std::net::IpAddr;

use hyper::HeaderMap;
use hyper::header::{HeaderName, HeaderValue};

const FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
const FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");
const FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");
const FORWARDED_PREFIX: HeaderName = HeaderName::from_static("x-forwarded-prefix");

/// One entry of `K7_FRONT_TRUSTED_PROXIES`: an address and a prefix length, IPv4-mapped addresses read as IPv4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cidr {
    net: IpAddr,
    bits: u8,
}

impl Cidr {
    /// `a.b.c.d/n`, `x::/n` or a bare address.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (addr, bits) = match text.split_once('/') {
            Some((a, b)) => (a, Some(b)),
            None => (text, None),
        };
        let net = unmapped(
            addr.parse::<IpAddr>()
                .map_err(|_| format!("{text:?} is not an address"))?,
        );
        let max = if net.is_ipv4() { 32 } else { 128 };
        let bits = match bits {
            Some(b) => b
                .parse::<u8>()
                .ok()
                .filter(|b| *b <= max)
                .ok_or_else(|| format!("{text:?} has no valid prefix length"))?,
            None => max,
        };
        Ok(Cidr { net, bits })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let (a, b) = match (self.net, unmapped(ip)) {
            (IpAddr::V4(n), IpAddr::V4(i)) => (u32::from(n) as u128, u32::from(i) as u128),
            (IpAddr::V6(n), IpAddr::V6(i)) => (u128::from(n), u128::from(i)),
            _ => return false,
        };
        let width = if self.net.is_ipv4() { 32 } else { 128 };
        let shift = width - u32::from(self.bits);
        shift >= width || a >> shift == b >> shift
    }
}

/// A comma- or space-separated list of `Cidr`s, refused whole when one entry is not one.
pub fn parse_proxies(text: &str) -> Result<Vec<Cidr>, String> {
    text.split([',', ' '])
        .filter(|t| !t.trim().is_empty())
        .map(Cidr::parse)
        .collect()
}

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

/// What a configured proxy set leaves of a request's forwarded headers: none from a peer outside it, else one `X-Forwarded-For` entry.
pub fn screen(headers: &mut HeaderMap, peer: IpAddr, proxies: &[Cidr]) {
    let trusted = |ip: IpAddr| proxies.iter().any(|c| c.contains(ip));
    let forwarded: Vec<String> = headers
        .get_all(&FORWARDED_FOR)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(String::from)
        .collect();
    headers.remove(&FORWARDED_FOR);
    if !trusted(peer) {
        headers.remove(&FORWARDED_HOST);
        headers.remove(&FORWARDED_PROTO);
        headers.remove(&FORWARDED_PREFIX);
        return;
    }
    // Express's trust function: from the right, every hop a trusted proxy wrote, up to the first one none did.
    let mut client = None;
    for hop in forwarded.iter().rev() {
        client = Some(hop.clone());
        if !hop.parse::<IpAddr>().is_ok_and(trusted) {
            break;
        }
    }
    if let Some(v) = client.and_then(|c| HeaderValue::from_str(&c).ok()) {
        headers.insert(FORWARDED_FOR, v);
    }
}

/// Under `K7_FRONT_TRUSTED_PROXIES`, every route and the proxy read only what a listed proxy forwarded (`peer::screen`).
pub(crate) async fn screen_forwarded(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::AppState>>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if let Some(proxies) = &state.settings.trusted_proxies {
        // No socket to read is no proxy to believe.
        let peer = req
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map_or(std::net::Ipv6Addr::UNSPECIFIED.into(), |c| c.0.ip());
        screen(req.headers_mut(), peer, proxies);
    }
    next.run(req).await
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

    #[test]
    fn parses_cidrs() {
        let set = parse_proxies("172.16.0.0/12, 10.0.0.1 fd00::/8").unwrap();
        assert!(set[0].contains("172.18.0.5".parse().unwrap()));
        assert!(set[0].contains("::ffff:172.31.255.1".parse().unwrap()));
        assert!(!set[0].contains("172.32.0.1".parse().unwrap()));
        assert!(set[1].contains("10.0.0.1".parse().unwrap()));
        assert!(!set[1].contains("10.0.0.2".parse().unwrap()));
        assert!(set[2].contains("fd12::1".parse().unwrap()));
        assert!(
            Cidr::parse("0.0.0.0/0")
                .unwrap()
                .contains("8.8.8.8".parse().unwrap())
        );
        assert!(parse_proxies("10.0.0.0/33").is_err());
        assert!(parse_proxies("traefik").is_err());
    }

    #[test]
    fn a_proxy_set_believes_only_its_own_members() {
        let set = parse_proxies("172.16.0.0/12").unwrap();
        let mut h = HeaderMap::new();
        h.insert(
            FORWARDED_FOR,
            HeaderValue::from_static("6.6.6.6, 198.51.100.7"),
        );
        h.insert(FORWARDED_HOST, HeaderValue::from_static("evil"));
        h.insert(FORWARDED_PROTO, HeaderValue::from_static("https"));
        screen(&mut h, "203.0.113.9".parse().unwrap(), &set);
        assert!(h.get(&FORWARDED_FOR).is_none());
        assert!(h.get(&FORWARDED_HOST).is_none());
        assert!(h.get(&FORWARDED_PROTO).is_none());
        let mut h = HeaderMap::new();
        h.insert(
            FORWARDED_FOR,
            HeaderValue::from_static("6.6.6.6, 198.51.100.7, 172.18.0.3"),
        );
        h.insert(FORWARDED_PROTO, HeaderValue::from_static("https"));
        screen(&mut h, "172.18.0.5".parse().unwrap(), &set);
        assert_eq!(h.get(&FORWARDED_FOR).unwrap(), "198.51.100.7");
        assert_eq!(h.get(&FORWARDED_PROTO).unwrap(), "https");
    }
}
