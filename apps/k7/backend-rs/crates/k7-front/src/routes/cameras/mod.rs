//! cameras — `/api/cameras`: the account's cameras that are on, the ticket that opens the signalling socket,
//! and that socket, as `apps/k7/backend/src/cameras/` answers them. The three move together: the hub's state
//! lives in this process, and a ticket only this front minted opens it (`apps/k7/backend-rs/docs/routes.md`).

mod hub;
mod names;
pub(crate) mod ticket;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::header::{CONNECTION, HeaderValue};
use hyper::{HeaderMap, StatusCode};
use serde_json::{Map, Value, json};

pub use hub::{Hub, Limits};

use crate::AppState;
use crate::session::{https, signed_in, with_cookies};
use crate::web::{error, read_json};
use hub::{MAX_FRAME_BYTES, Role};
use ticket::Tickets;

/// The hub, the tickets that open it, and the ICE servers handed out with each ticket.
pub struct Cameras {
    pub hub: Arc<Hub>,
    tickets: Tickets,
    ice_servers: Value,
}

impl Cameras {
    pub fn new(ice: Option<&str>, limits: Limits) -> Self {
        Cameras {
            hub: Arc::new(Hub::new(limits)),
            tickets: Tickets::new(),
            ice_servers: Value::Array(names::ice_servers(ice)),
        }
    }
}

/// `subjectOf`: the `sub`, else the email; empty is no account.
fn account_of(user: &Map<String, Value>) -> Option<String> {
    let pick = |k: &str| user.get(k).filter(|v| !v.is_null());
    pick("sub")
        .or_else(|| pick("email"))
        .and_then(Value::as_str)
        .filter(|a| !a.is_empty())
        .map(str::to_string)
}

/// `answer` for the session's account, with the cookies the front started; else the 401 `requireAuth` and the route send.
fn as_account(
    headers: &HeaderMap,
    state: &AppState,
    answer: impl FnOnce(&str) -> Value,
) -> Response {
    let https = https(headers, &state.settings);
    let Some(signed) = signed_in(headers, &state.settings, &state.revocations, https) else {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    };
    let response = match account_of(&signed.user) {
        Some(a) => axum::Json(answer(&a)).into_response(),
        None => error(StatusCode::UNAUTHORIZED, "no account"),
    };
    with_cookies(response, &signed)
}

/// GET: the account's cameras that are on, `[{ name, since }]`.
pub async fn list(State(state): State<Arc<AppState>>, req: Request) -> Response {
    as_account(req.headers(), &state, |a| state.cameras.hub.list(a))
}

/// POST `/ticket`: `{ ticket, iceServers }`, what opens the socket for a minute.
pub async fn ticket(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let headers = match read_json(req).await {
        Ok((h, _)) => h,
        Err(r) => return r,
    };
    let c = &state.cameras;
    as_account(
        &headers,
        &state,
        |a| json!({ "ticket": c.tickets.issue(a), "iceServers": c.ice_servers }),
    )
}

/// A refused upgrade: a bare status that closes the connection, as the backend writes it on the socket.
fn deny(status: StatusCode) -> Response {
    let mut r = status.into_response();
    r.headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    r
}

/// The socket: `?ticket=…&role=camera&name=…` or `?ticket=…&role=viewer`; a request that is no upgrade goes to the backend.
pub async fn socket(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    if !crate::proxy::is_upgrade(req.headers()) {
        return crate::proxied(State(state), peer, req).await;
    }
    let query = req.uri().query().unwrap_or("");
    let param = |k: &str| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.into_owned())
    };
    let cameras = &state.cameras;
    let Some(account) = cameras.tickets.verify(&param("ticket").unwrap_or_default()) else {
        return deny(StatusCode::UNAUTHORIZED);
    };
    let role = match param("role").as_deref() {
        Some("camera") => Role::Camera(param("name").unwrap_or_default()),
        Some("viewer") => Role::Viewer,
        _ => return deny(StatusCode::BAD_REQUEST),
    };
    if cameras.hub.full(&account, &role) {
        return deny(StatusCode::TOO_MANY_REQUESTS);
    }
    let (mut parts, _) = req.into_parts();
    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
        Ok(u) => u,
        Err(rejection) => return rejection.into_response(),
    };
    let Some(admitted) = cameras.hub.admit(&account, role) else {
        return deny(StatusCode::TOO_MANY_REQUESTS);
    };
    let (hub, failed, id) = (
        cameras.hub.clone(),
        cameras.hub.clone(),
        admitted.id.clone(),
    );
    upgrade
        .max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_failed_upgrade(move |_| failed.leave(&id))
        .on_upgrade(move |socket| hub.serve(admitted, socket))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_is_the_sub_else_the_email() {
        let user = |v: Value| v.as_object().unwrap().clone();
        assert_eq!(
            account_of(&user(json!({ "sub": "s", "email": "e" }))).as_deref(),
            Some("s")
        );
        assert_eq!(
            account_of(&user(json!({ "email": "e" }))).as_deref(),
            Some("e")
        );
        assert_eq!(
            account_of(&user(json!({ "sub": null, "email": "e" }))).as_deref(),
            Some("e")
        );
        assert_eq!(account_of(&user(json!({ "sub": "", "email": "e" }))), None);
        assert_eq!(account_of(&user(json!({}))), None);
    }
}
