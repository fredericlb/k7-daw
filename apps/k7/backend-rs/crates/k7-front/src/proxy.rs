//! proxy.rs — a request the front does not own goes to the TypeScript backend as it came: streamed both
//! ways, never buffered, and a WebSocket upgrade spliced byte for byte once the backend says 101.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::header::{CONNECTION, HOST, HeaderValue, UPGRADE};
use hyper::{HeaderMap, StatusCode, Uri, Version};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::Config;
use crate::peer;

pub struct Proxy {
    client: Client<HttpConnector, Body>,
    upstream: String,
    secret: String,
    hops: usize,
}

impl Proxy {
    pub fn new(config: &Config) -> Arc<Self> {
        let mut connector = HttpConnector::new();
        connector.set_nodelay(true);
        // Below the backend's 95 s keep-alive, so the pool never reuses a socket the backend is closing.
        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(60))
            .build(connector);
        Arc::new(Proxy {
            client,
            upstream: config.upstream.clone(),
            secret: config.secret.clone(),
            hops: config.trust_hops,
        })
    }
}

impl Proxy {
    /// The secret the backend checks, which is also the internal seam's.
    pub fn secret(&self) -> &str {
        &self.secret
    }
}

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

pub async fn forward(
    State(proxy): State<Arc<Proxy>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut req: Request,
) -> Response {
    // A front alone has nobody to hand a request to: a route it does not answer is nobody's.
    if proxy.upstream.is_empty() {
        return (StatusCode::BAD_GATEWAY, "no backend behind this front").into_response();
    }
    let upgrade = is_upgrade(req.headers());
    let client_side = upgrade.then(|| hyper::upgrade::on(&mut req));
    let (mut parts, body) = req.into_parts();
    let path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
    let Ok(uri) = format!("http://{}{path}", proxy.upstream).parse::<Uri>() else {
        return (StatusCode::BAD_REQUEST, "bad request target").into_response();
    };
    // An HTTP/2 request names its host in :authority; the backend reads Host.
    if !parts.headers.contains_key(HOST)
        && let Some(v) = parts
            .uri
            .authority()
            .and_then(|a| HeaderValue::from_str(a.as_str()).ok())
    {
        parts.headers.insert(HOST, v);
    }
    strip_hop_by_hop(&mut parts.headers, upgrade);
    peer::stamp(&mut parts.headers, peer.ip(), &proxy.secret, proxy.hops);
    parts.uri = uri;
    parts.version = Version::HTTP_11;
    let mut response = match proxy.client.request(Request::from_parts(parts, body)).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("k7's backend did not answer: {e}"),
            )
                .into_response();
        }
    };
    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        if let Some(client_side) = client_side {
            tokio::spawn(splice(client_side, hyper::upgrade::on(&mut response)));
        }
    } else {
        strip_hop_by_hop(response.headers_mut(), false);
    }
    response.map(Body::new)
}

/// Both halves of an upgraded connection, copied into each other until either closes.
async fn splice(client: hyper::upgrade::OnUpgrade, upstream: hyper::upgrade::OnUpgrade) {
    let (Ok(client), Ok(upstream)) = tokio::join!(client, upstream) else {
        return;
    };
    let _ =
        tokio::io::copy_bidirectional(&mut TokioIo::new(client), &mut TokioIo::new(upstream)).await;
}
