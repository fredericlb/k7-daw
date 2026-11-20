//! internal.rs — the local engine's half of the internal seam (`apps/k7/backend-rs/docs/local.md`):
//! `GET /api/internal/engine/local`, which tells whether the supervisor the front runs has answered. Gated as the engine half is: a loopback peer AND the front's secret.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CACHE_CONTROL, HeaderValue};
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::AppState;
use crate::web::error;

/// The gate's refusal, or None to go on: the SOCKET's peer, since nothing in front of the front may vouch for one.
fn refused(state: &AppState, peer: SocketAddr, req: &Request) -> Option<Response> {
    let ip = match peer.ip() {
        std::net::IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map_or(std::net::IpAddr::V6(v6), Into::into),
        v4 => v4,
    };
    if !ip.is_loopback() {
        return Some(error(
            StatusCode::FORBIDDEN,
            "the internal API answers this machine only",
        ));
    }
    let given = req
        .headers()
        .get("x-k7-internal")
        .map(HeaderValue::as_bytes);
    let secret = state.front_secret.as_bytes();
    let same = given.is_some_and(|g| !secret.is_empty() && bool::from(g.ct_eq(secret)));
    (!same).then(|| {
        error(
            StatusCode::UNAUTHORIZED,
            "the internal API needs this process's secret",
        )
    })
}

fn no_store(mut r: Response) -> Response {
    r.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// The front's engine when it runs one; otherwise the request is the backend's, which answers 404.
fn owned(state: &AppState) -> Option<&Arc<super::Local>> {
    state.local.as_ref().filter(|l| l.owned())
}

/// `GET /api/internal/engine/local`: whether the supervisor has sent its first snapshot since it started.
pub async fn local(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    let Some(local) = owned(&state).cloned() else {
        return crate::declined(State(state), peer, req).await;
    };
    if let Some(r) = refused(&state, peer.0, &req) {
        return r;
    }
    let answered = local.engine.state().snapshot.is_some();
    no_store(axum::Json(json!({ "answered": answered })).into_response())
}
