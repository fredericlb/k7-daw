//! urls — the URL rules the supervisor shares with the web apps: which registry a typed address means, which
//! studio family it belongs to, what a server's cache folder is called, and which plain-http address may carry
//! the engine's token. The twin of `apps/k7-daw-frontend/packages/web-protocol/services.ts`, whose cases the
//! tests replay: two spellings of one rule send a heartbeat to the wrong place.

use url::Url;

const DEFAULT_K7_PORT: &str = "8789";

/// k7's loopback port: `k7=` in `$STUDIO_PORTS` when a host moved it, else 8789.
fn k7_port() -> String {
    k7_port_in(&std::env::var("STUDIO_PORTS").unwrap_or_default())
}

/// `k7_port` over a `$STUDIO_PORTS` text: `k7=18789,pl8=18795`.
pub fn k7_port_in(spec: &str) -> String {
    for pair in spec.split(',') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let digits =
            !value.is_empty() && value.len() <= 5 && value.bytes().all(|b| b.is_ascii_digit());
        if name == "k7" && digits && value.parse::<u32>().is_ok_and(|n| (1..65536).contains(&n)) {
            return value
                .parse::<u32>()
                .map(|n| n.to_string())
                .unwrap_or_default();
        }
    }
    DEFAULT_K7_PORT.into()
}

/// Parse like Node's `new URL(s)`, or nothing where that would throw.
pub fn parse(s: &str) -> Option<Url> {
    Url::parse(s).ok()
}

/// `host[:port]`, the port absent when it is the scheme's default.
pub fn host_of(u: &Url) -> String {
    let host = u.host_str().unwrap_or_default();
    match u.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    }
}

/// `scheme://host[:port]`, or "null" for a scheme with no origin.
pub fn origin_of(u: &Url) -> String {
    match u.scheme() {
        "http" | "https" | "ws" | "wss" | "ftp" => format!("{}://{}", u.scheme(), host_of(u)),
        _ => "null".into(),
    }
}

/// Whether `hostname` is one where services are ports rather than subdomains.
pub fn is_loopback_host(h: &str) -> bool {
    if h == "localhost" || h == "[::1]" {
        return true;
    }
    let Some(rest) = h.strip_prefix("127.") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Where a service answers, given any URL of its family: that URL's origin, as k7 serves every service; "" for a non-URL.
pub fn service_url(origin: &str) -> String {
    parse(origin).map(|u| origin_of(&u)).unwrap_or_default()
}

/// The family key: k7's origin, or the bare scheme and host on loopback; "" for a non-URL.
pub fn studio_family(raw: &str) -> String {
    let Some(u) = parse(raw) else {
        return String::new();
    };
    let host = u.host_str().unwrap_or_default();
    if is_loopback_host(host) {
        return format!("{}://{host}", u.scheme());
    }
    service_url(&origin_of(&u))
}

/// The ONE engine registry, the family root, for any URL of a family; "" for a non-URL or a non-web scheme.
pub fn engine_registry_url(raw: &str) -> String {
    let Some(u) = parse(raw) else {
        return String::new();
    };
    if !matches!(u.scheme(), "http" | "https") {
        return String::new();
    }
    let host = u.host_str().unwrap_or_default();
    if is_loopback_host(host) {
        return format!("{}://{host}:{}", u.scheme(), k7_port());
    }
    service_url(&origin_of(&u))
}

/// Why `raw` may not carry this engine's token, or "": http/ws only to loopback or to a host `allowlist` names.
pub fn plaintext_refusal_with(raw: &str, allowlist: &str) -> String {
    let Some(u) = parse(raw) else {
        return format!("{raw} is not a web address");
    };
    match u.scheme() {
        "https" | "wss" => return String::new(),
        "http" | "ws" => {}
        _ => return format!("{raw} is not an http(s) address"),
    }
    let hostname = u.host_str().unwrap_or_default();
    if is_loopback_host(hostname) {
        return String::new();
    }
    let host = host_of(&u);
    for entry in allowlist.split(',') {
        let entry = entry.trim().to_lowercase();
        if !entry.is_empty() && (entry == hostname || entry == host) {
            return String::new();
        }
    }
    format!(
        "{} is plain http and not loopback: refused, since the engine's token would cross in clear (use https, or name the host in STUDIO_INSECURE_REGISTRIES)",
        origin_of(&u)
    )
}

/// `plaintext_refusal_with` against `$STUDIO_INSECURE_REGISTRIES`: comma-separated hostnames or `host:port`s.
pub fn plaintext_refusal(raw: &str) -> String {
    plaintext_refusal_with(
        raw,
        &std::env::var("STUDIO_INSECURE_REGISTRIES").unwrap_or_default(),
    )
}

/// What the user typed, reduced to a bare origin, strictly; a bare host gets `https://`.
pub fn normalise_user_origin(input: &str) -> Option<String> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }
    let u = parse(&if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    })?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    // Rejected, not dropped: a URL carrying credentials is not an address somebody meant.
    if !u.username().is_empty() || u.password().is_some() {
        return None;
    }
    let host = u.host_str().unwrap_or_default();
    if host.is_empty() || host.contains('*') {
        return None;
    }
    Some(origin_of(&u))
}

/// A folder name for a server, from its host: `pl8.example:443` becomes `pl8.example_443`.
pub fn cache_key(registry_url: &str) -> String {
    let key = parse(registry_url)
        .map(|u| host_of(&u))
        .unwrap_or_else(|| registry_url.to_string());
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `base` with any trailing slash removed, then `path` appended.
pub fn join(base: &str, path: &str) -> String {
    format!("{}{path}", base.strip_suffix('/').unwrap_or(base))
}
