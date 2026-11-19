//! upgrade.rs — the three WebSockets the core accepts, in the order they are checked before the 101:
//! a browser's relay socket (its ticket names the engine and the account), a supervisor's tunnel (its token,
//! its secret, its protocol, its render key), and a loopback client on a bound path. A request that is no
//! upgrade goes to the backend; a refusal is a bare status, as `deny` writes it.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::header::{
    CONNECTION, CONTENT_TYPE, HeaderValue, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY,
    SEC_WEBSOCKET_VERSION, UPGRADE,
};
use hyper::{Method, StatusCode};
use hyper_util::rt::TokioIo;
use k7_engine::registry::is_loopback_address;
use k7_engine::tunnel::frames::{
    TUNNEL_PROTOCOL_HEADER, TUNNEL_RENDER_KEY_HEADER, TUNNEL_SECRET_HEADER, TUNNEL_TOKEN_HEADER,
};
use k7_engine::tunnel::hub::{Deny, TunnelAsk, local_key};
use k7_engine::{relay, tunnel};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;

use super::engine_of;
use crate::AppState;

pub(crate) type Upgraded = TokioIo<hyper::upgrade::Upgraded>;

/// A refusal before the 101: the status alone, and the connection closed.
pub(crate) fn deny(d: Deny) -> Response {
    let (status, body) = match d {
        Deny::Status(s) => (s, None),
        Deny::Reason(s, r) => (s, Some(r)),
    };
    let mut r = match body {
        Some(b) => {
            let mut r = (
                StatusCode::from_u16(status).unwrap_or(StatusCode::FORBIDDEN),
                b,
            )
                .into_response();
            r.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            r
        }
        None => StatusCode::from_u16(status)
            .unwrap_or(StatusCode::FORBIDDEN)
            .into_response(),
    };
    r.headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    r
}

/// The 101 for a valid handshake and the socket it becomes, or `ws`'s 400 for an invalid one.
pub(crate) fn handshake(
    mut req: Request,
    serve: impl FnOnce(WebSocketStream<Upgraded>) -> futures_util::future::BoxFuture<'static, ()>
    + Send
    + 'static,
) -> Response {
    let h = req.headers();
    let header_is = |name, want: &str| {
        h.get_all(name)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .any(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case(want)))
    };
    let key = h.get(SEC_WEBSOCKET_KEY).cloned();
    let valid = req.method() == Method::GET
        && header_is(UPGRADE, "websocket")
        && header_is(CONNECTION, "upgrade")
        && h.get(SEC_WEBSOCKET_VERSION)
            .is_some_and(|v| v == "13" || v == "8");
    let Some(key) = key.filter(|_| valid) else {
        return deny(Deny::Status(400));
    };
    let on = hyper::upgrade::on(&mut req);
    tokio::spawn(async move {
        if let Ok(upgraded) = on.await {
            let ws = WebSocketStream::from_raw_socket(
                TokioIo::new(upgraded),
                Role::Server,
                Some(k7_engine::ws::config()),
            )
            .await;
            serve(ws).await;
        }
    });
    let mut r = Response::new(Body::empty());
    *r.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    let h = r.headers_mut();
    h.insert(UPGRADE, HeaderValue::from_static("websocket"));
    h.insert(CONNECTION, HeaderValue::from_static("Upgrade"));
    if let Ok(v) = HeaderValue::from_str(&derive_accept_key(key.as_bytes())) {
        h.insert(SEC_WEBSOCKET_ACCEPT, v);
    }
    r
}

/// `decodeURIComponent`, None for a malformed escape.
fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn query(req: &Request, name: &str) -> Option<String> {
    let q = req.uri().query().unwrap_or("");
    url::form_urlencoded::parse(q.as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// `/api/engine/<id>/ws?ticket=…`: the ticket is the upgrade's only credential.
pub async fn relay(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    if !crate::hop::is_upgrade(req.headers()) {
        return crate::declined(State(state), peer, req).await;
    }
    let raw = req
        .uri()
        .path()
        .trim_start_matches("/api/engine/")
        .trim_end_matches("/ws")
        .to_string();
    let Some(id) = decode(&raw) else {
        return deny(Deny::Status(400));
    };
    let core = engine_of(&state).core.clone();
    let ticket = core.tickets.verify(
        &query(&req, "ticket").unwrap_or_default(),
        core.clock.now_ms(),
    );
    let Some(ticket) = ticket.filter(|t| t.id == id) else {
        return deny(Deny::Status(401));
    };
    let Some(entry) = core.registry.get(&id) else {
        return deny(Deny::Status(404));
    };
    handshake(req, move |ws| {
        Box::pin(relay::serve(core, id, ticket.holder, entry, ws))
    })
}

/// `/api/engine-tunnel?id=…`: a supervisor dialling in.
pub async fn tunnel(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    if !crate::hop::is_upgrade(req.headers()) {
        return crate::declined(State(state), peer, req).await;
    }
    let hub = engine_of(&state).core.hub.clone();
    let header = |n: &str| {
        req.headers()
            .get(n)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let id = query(&req, "id").unwrap_or_default();
    let (token, secret) = (
        header(TUNNEL_TOKEN_HEADER).unwrap_or_default(),
        header(TUNNEL_SECRET_HEADER),
    );
    let (protocol, key) = (
        header(TUNNEL_PROTOCOL_HEADER),
        header(TUNNEL_RENDER_KEY_HEADER),
    );
    let ask = TunnelAsk {
        id: &id,
        token: &token,
        secret: secret.as_deref().filter(|s| !s.is_empty()),
        protocol: protocol.as_deref(),
        render_key: key.as_deref(),
    };
    match hub.check_tunnel(&ask) {
        Err(d) => deny(d),
        Ok(renders) => handshake(req, move |ws| Box::pin(hub.serve_tunnel(id, renders, ws))),
    }
}

/// `/api/engine-tunnel/local/<key>`: a process on this machine, its key the only credential.
pub async fn local(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    if !crate::hop::is_upgrade(req.headers()) {
        return crate::declined(State(state), ConnectInfo(peer), req).await;
    }
    // Not one of the served upgrade paths: refused as `upgradeGuard.ts` refuses it.
    let Some(key) = local_key(req.uri().path()).map(str::to_string) else {
        return deny(Deny::Status(404));
    };
    let hub = engine_of(&state).core.hub.clone();
    let loopback = is_loopback_address(Some(&crate::peer::canonical(peer.ip())));
    match hub.check_local(&key, loopback) {
        Err(d) => deny(d),
        Ok(target) => handshake(req, move |ws| Box::pin(tunnel::local::serve(target, ws))),
    }
}
