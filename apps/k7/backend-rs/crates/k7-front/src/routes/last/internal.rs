//! internal.rs — the seam's TypeScript half (`apps/k7/backend/src/media/internalFiles.ts` behind `internalGate`):
//! a library bearer's subject, and the backend's word that it runs no médiathèque and no recorder.
//! What a reader gets wrong: `verify-token` checks the front's own bearers too, the ones its hub mints for the file
//! services, since no other process holds them (`packages/engine-registry/docs/internalSeam.md`).

use std::net::SocketAddr;

use axum::Json;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue};
use hyper::{Method, StatusCode};
use k7_engine::registry::is_loopback_address;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use crate::AppState;
use crate::web::{error, read_json_within};

pub const INTERNAL: &str = "/api/internal";

/// The router's own body limit, `express.json({ limit: "8kb" })`.
const BODY_LIMIT: usize = 8 * 1024;

/// How long a use of a library token waits before it is written, as `fileTokenStore` flushes.
const FLUSH_MS: f64 = 60_000.0;

/// The subject a library bearer verifies as, the `/api` guard's own check: the hub's ephemeral bearers, then the listed ones.
pub fn verify_bearer(state: &AppState, token: &str) -> Option<String> {
    let secret = &state.settings.api_token_secret;
    if let Some(sub) = state
        .files
        .as_ref()
        .and_then(|f| f.hub.verify_service_token(secret, token))
    {
        return Some(sub);
    }
    let now = crate::session::now_ms();
    let sub = state.tokens.verify_and_touch(secret, token, now);
    if let Err(e) = state.tokens.flush_due(now, FLUSH_MS) {
        eprintln!("k7-front: the library tokens' last uses were not written: {e}");
    }
    sub
}

/// Every request under `/api/internal` the front's own families left: the gate, then the answer.
pub async fn answer(state: &AppState, peer: SocketAddr, req: Request) -> Response {
    let mut response = gated(state, peer, req).await;
    let h = response.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    if h.get(CONTENT_TYPE).is_some_and(|v| v == "application/json") {
        h.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
    }
    response
}

/// A loopback peer (403), then the secret in constant time (401), as `internalGate` refuses them.
async fn gated(state: &AppState, peer: SocketAddr, req: Request) -> Response {
    let peer = crate::peer::canonical(peer.ip());
    if !is_loopback_address(Some(&peer)) {
        return error(
            StatusCode::FORBIDDEN,
            "the internal API answers this machine only",
        );
    }
    let given = req
        .headers()
        .get("x-k7-internal")
        .map(|v| v.as_bytes())
        .unwrap_or_default();
    let secret = state.front_secret.as_bytes();
    if secret.is_empty() || !bool::from(given.ct_eq(secret)) {
        return error(
            StatusCode::UNAUTHORIZED,
            "the internal API needs this process's secret",
        );
    }
    let mut response = files_half(state, req).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// What `internalFilesRouter` answers once the front holds the hub, and the seam's 404 for the rest.
async fn files_half(state: &AppState, req: Request) -> Response {
    let rest = req
        .uri()
        .path()
        .strip_prefix("/api/internal/files")
        .unwrap_or("");
    match rest {
        "/verify-token" if req.method() == Method::POST => {
            let body = match read_json_within(req, BODY_LIMIT).await {
                Ok((_, v)) => v,
                Err(r) => return r,
            };
            let token = body.get("token").and_then(Value::as_str).filter(|t| {
                let units = t.encode_utf16().count();
                (1..=4096).contains(&units)
            });
            let Some(token) = token else {
                return error(StatusCode::BAD_REQUEST, "a token is required");
            };
            Json(json!({ "sub": verify_bearer(state, token) })).into_response()
        }
        _ => error(StatusCode::NOT_FOUND, "no such internal route"),
    }
}
