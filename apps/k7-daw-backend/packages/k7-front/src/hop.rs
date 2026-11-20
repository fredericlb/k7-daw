//! hop.rs — what a front that relays a request to another process (the dev page's Vite) keeps of it: the
//! hop-by-hop headers dropped, and whether a request asks to upgrade.

use hyper::HeaderMap;
use hyper::header::{CONNECTION, HeaderValue, UPGRADE};

/// Hop-by-hop headers, which describe one connection and never cross a proxy.
const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
];

pub fn is_upgrade(headers: &HeaderMap) -> bool {
    headers.contains_key(UPGRADE)
        && headers
            .get_all(CONNECTION)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .any(|v| {
                v.split(',')
                    .any(|t| t.trim().eq_ignore_ascii_case("upgrade"))
            })
}

/// Drops the hop-by-hop headers, and those `Connection` names; an upgrade keeps `Connection: upgrade` and `Upgrade`.
pub(crate) fn strip_hop_by_hop(headers: &mut HeaderMap, upgrade: bool) {
    let named: Vec<String> = headers
        .get_all(CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(',').map(|t| t.trim().to_ascii_lowercase()))
        .collect();
    for name in named.iter().map(String::as_str).chain(HOP_BY_HOP) {
        if !(upgrade && name == "upgrade") {
            headers.remove(name);
        }
    }
    if upgrade {
        headers.insert(CONNECTION, HeaderValue::from_static("upgrade"));
    } else {
        headers.remove(UPGRADE);
    }
}
