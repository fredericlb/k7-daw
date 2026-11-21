//! relay.rs — this device's page reaching another device's door: the HTTP routes streamed to the peer's door with
//! its engine token added, and the engine socket carried byte for byte to the peer's engine. The page speaks to
//! loopback only (Android refuses cleartext to anything else). What a reader gets wrong: the engine sees THIS
//! device's LAN address, so its own trust rule decides, not the page's.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ws::{CloseFrame, Message as Down, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRequestParts, Path, Request, State};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use hyper::header::{HOST, HeaderValue};
use hyper::{HeaderMap, StatusCode, Uri};
use tokio_tungstenite::tungstenite::Message as Up;
use tokio_tungstenite::tungstenite::protocol::CloseFrame as UpClose;

use super::peers::{PeerView, authority};
use crate::AppState;
use crate::session::holds;
use crate::web::error;

/// A peer's door is given this long to answer one request.
const ANSWER_WITHIN: Duration = Duration::from_secs(120);
/// What an engine frame may weigh: a client's document is far bigger than any other message.
const MAX_FRAME: usize = 64 * 1024 * 1024;

/// What the page's own headers must not carry to another device.
const NOT_FORWARDED: [&str; 6] = [
    "cookie",
    "authorization",
    "host",
    "x-studio-engine-token",
    "x-studio-engine-id",
    "x-k7-engine",
];

pub async fn hello(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    through(state, id, "hello".into(), req).await
}

pub async fn screens(
    State(state): State<Arc<AppState>>,
    Path((id, rest)): Path<(String, String)>,
    req: Request,
) -> Response {
    through(state, id, format!("screens/{rest}"), req).await
}

pub async fn files(
    State(state): State<Arc<AppState>>,
    Path((id, rest)): Path<(String, String)>,
    req: Request,
) -> Response {
    through(state, id, format!("files/{rest}"), req).await
}

/// A request for `/lan/<sub>` at the peer's door, its token added and the page's credentials left behind.
async fn through(state: Arc<AppState>, id: String, sub: String, req: Request) -> Response {
    let signed = req.headers().clone();
    let Some(peer) = known(&state, &id, &signed).await else {
        return answer_for(&state, &id, &signed).await;
    };
    let (mut parts, body) = req.into_parts();
    let query = parts
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let authority = authority(&peer.host, peer.door);
    let Ok(uri) = format!("http://{authority}/lan/{sub}{query}").parse::<Uri>() else {
        return error(StatusCode::BAD_REQUEST, "bad request target");
    };
    crate::hop::strip_hop_by_hop(&mut parts.headers, false);
    for name in NOT_FORWARDED {
        parts.headers.remove(name);
    }
    let (Ok(host), Ok(token)) = (
        HeaderValue::from_str(&authority),
        HeaderValue::from_str(&peer.token),
    ) else {
        return error(
            StatusCode::BAD_GATEWAY,
            "this device's token is not a header",
        );
    };
    parts.headers.insert(HOST, host);
    parts.headers.insert("x-k7-engine", token);
    parts.uri = uri;
    parts.version = hyper::Version::HTTP_11;
    let lan = state
        .lan
        .as_ref()
        .expect("claimed only where the family is on");
    let asked = lan.http.request(Request::from_parts(parts, body));
    match tokio::time::timeout(ANSWER_WITHIN, asked).await {
        Ok(Ok(mut r)) => {
            crate::hop::strip_hop_by_hop(r.headers_mut(), false);
            r.map(Body::new)
        }
        Ok(Err(e)) => error(
            StatusCode::BAD_GATEWAY,
            &format!("that device did not answer: {e}"),
        ),
        Err(_) => error(
            StatusCode::GATEWAY_TIMEOUT,
            "that device did not answer in time",
        ),
    }
}

/// The peer when the page is signed in, the device is known and trusted; None leaves `answer_for` to say which.
async fn known(state: &AppState, id: &str, headers: &HeaderMap) -> Option<PeerView> {
    if !holds(headers, &state.settings) {
        return None;
    }
    let lan = state.lan.as_ref()?;
    lan.peer(state, id).await.filter(|p| p.trusted)
}

/// Why `known` said no.
async fn answer_for(state: &AppState, id: &str, headers: &HeaderMap) -> Response {
    if !holds(headers, &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    match state.lan.as_ref() {
        Some(lan) => match lan.peer(state, id).await {
            Some(_) => error(StatusCode::FORBIDDEN, "this device does not trust this one"),
            None => error(StatusCode::NOT_FOUND, "no such device on the network"),
        },
        None => error(StatusCode::NOT_FOUND, "no such device on the network"),
    }
}

/// `GET /api/lan/<id>/engine`: the peer's engine socket, carried frame for frame.
pub async fn engine(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let signed = req.headers().clone();
    let Some(peer) = known(&state, &id, &signed).await else {
        return answer_for(&state, &id, &signed).await;
    };
    let url = format!("ws://{}/", authority(&peer.host, peer.engine));
    let (mut parts, _) = req.into_parts();
    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
        Ok(u) => u,
        Err(rejection) => return rejection.into_response(),
    };
    // Dialled before the 101, so a device that is gone is a 502 and not a socket that opens and closes.
    let dialled = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(url),
    )
    .await;
    let Ok(Ok((up, _))) = dialled else {
        return error(
            StatusCode::BAD_GATEWAY,
            "that device's engine did not answer",
        );
    };
    upgrade
        .max_message_size(MAX_FRAME)
        .max_frame_size(MAX_FRAME)
        .on_upgrade(move |socket| pump(socket, up))
}

type Peer =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn to_up(m: Down) -> Up {
    match m {
        Down::Text(t) => Up::text(t.as_str()),
        Down::Binary(b) => Up::Binary(b),
        Down::Ping(b) => Up::Ping(b),
        Down::Pong(b) => Up::Pong(b),
        Down::Close(c) => Up::Close(c.map(|c| UpClose {
            code: c.code.into(),
            reason: c.reason.as_str().into(),
        })),
    }
}

fn to_down(m: Up) -> Option<Down> {
    Some(match m {
        Up::Text(t) => Down::Text(t.as_str().into()),
        Up::Binary(b) => Down::Binary(b),
        Up::Ping(b) => Down::Ping(b),
        Up::Pong(b) => Down::Pong(b),
        Up::Close(c) => Down::Close(c.map(|c| CloseFrame {
            code: c.code.into(),
            reason: c.reason.as_str().into(),
        })),
        Up::Frame(_) => return None,
    })
}

/// Both directions until either side closes.
async fn pump(page: WebSocket, peer: Peer) {
    let (mut page_tx, mut page_rx) = page.split();
    let (mut peer_tx, mut peer_rx) = peer.split();
    let up = async {
        while let Some(Ok(m)) = page_rx.next().await {
            if peer_tx.send(to_up(m)).await.is_err() {
                break;
            }
        }
        let _ = peer_tx.close().await;
    };
    let down = async {
        while let Some(Ok(m)) = peer_rx.next().await {
            let Some(m) = to_down(m) else { continue };
            if page_tx.send(m).await.is_err() {
                break;
            }
        }
        let _ = page_tx.close().await;
    };
    tokio::select! { _ = up => {}, _ = down => {} }
}
