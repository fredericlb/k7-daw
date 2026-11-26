//! net.rs — what an address or an origin says about a peer: the machine itself, its own network, a page
//! the engine may serve. Pure functions, ported from Pairing.cpp's free functions with its tests.
//! What a reader gets wrong: relayed traffic reaches the engine from loopback, so "local" is the peer
//! address alone (`docs/pairing.md`).

/// A peer address the machine itself owns: 127.0.0.0/8, ::1, and the v4-mapped form of either.
pub fn is_loopback(ip: &str) -> bool {
    let v = ip.strip_prefix("::ffff:").unwrap_or(ip);
    v == "::1" || v == "localhost" || v.starts_with("127.")
}

/// A peer on the machine's own network: 10/8, 172.16/12, 192.168/16, 169.254/16, fc00::/7, fe80::/10.
pub fn is_private(ip: &str) -> bool {
    let v = ip.strip_prefix("::ffff:").unwrap_or(ip);
    if let Some([a, b]) = dotted_quad(v) {
        return a == 10
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
            || (a == 169 && b == 254);
    }
    if !v.contains(':') || v.len() < 4 {
        return false;
    }
    let h: Vec<u8> = v.bytes().take(4).map(|c| c.to_ascii_lowercase()).collect();
    // fc00::/7 is the unique-local range, fe80::/10 the link-local one.
    h[0] == b'f'
        && (h[1] == b'c' || h[1] == b'd' || (h[1] == b'e' && (b'8'..=b'b').contains(&h[2])))
}

/// `a.b.c.d` with every part 0..=255, as the first two parts; None for any other text.
fn dotted_quad(v: &str) -> Option<[u32; 2]> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut n = [0u32; 4];
    for (slot, p) in n.iter_mut().zip(&parts) {
        if p.is_empty() || p.len() > 3 || !p.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *slot = p.parse().ok()?;
        if *slot > 255 {
            return None;
        }
    }
    Some([n[0], n[1]])
}

/// Two secrets compared in a time that does not depend on where they first differ.
pub fn same_key(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// `origin` lower-cased and without a trailing slash, the one spelling both sides are compared in.
fn normal_origin(origin: &str) -> String {
    origin
        .to_ascii_lowercase()
        .trim_end_matches('/')
        .to_string()
}

/// Whether a socket whose handshake carries `Origin: origin` may be served: loopback, k7.rocks, or one of `extra`.
pub fn origin_allowed(origin: &str, extra: &[String]) -> bool {
    let o = normal_origin(origin);
    let Some((scheme, rest)) = o.split_once("://") else {
        return false;
    };
    // A browser never sends a ws: origin, so this is a native client.
    if scheme == "ws" || scheme == "wss" {
        return true;
    }
    if extra.iter().any(|e| normal_origin(e) == o) {
        return true;
    }
    if scheme != "http" && scheme != "https" {
        return false;
    }
    let bracketed = rest.starts_with('[');
    let port_at = rest
        .rfind(':')
        .filter(|&i| !bracketed || rest.find(']').is_some_and(|c| i > c));
    let (host, port) = match port_at {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    if port.is_some_and(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit())) {
        return false;
    }
    if host == "127.0.0.1" || host == "localhost" || host == "[::1]" {
        return true;
    }
    if scheme != "https" || port.is_some() {
        return false;
    }
    if host == "k7.rocks" {
        return true;
    }
    let Some(sub) = host.strip_suffix(".k7.rocks") else {
        return false;
    };
    // Only letters, digits, dots and dashes before the apex, so nothing like `user@` passes for a subdomain.
    !sub.is_empty()
        && host
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
}

/// `STUDIO_ENGINE_ALLOWED_ORIGINS`: comma-separated origins, trimmed, lower-cased, a trailing slash dropped.
pub fn parse_origins(list: &str) -> Vec<String> {
    list.split(',')
        .map(|one| normal_origin(one.trim_matches(|c| c == ' ' || c == '\t')))
        .filter(|o| !o.is_empty())
        .collect()
}
