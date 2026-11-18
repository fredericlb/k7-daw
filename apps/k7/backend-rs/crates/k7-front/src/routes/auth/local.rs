//! local.rs — `/auth/local`, a device's signed-out state: an existing session is kept, an empty one becomes the
//! local user. What a reader gets wrong: it answers this machine only, by the peer's address and by name.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::response::Response;
use hyper::StatusCode;

use super::ctx::Ctx;
use super::express::{Query, html, redirect};
use super::target::{public_origin, safe_return_target};
use crate::AppState;
use crate::web::is_loopback_request;

pub async fn local(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    let headers = req.headers().clone();
    let q = Query::parse(req.uri().query());
    let mut ctx = Ctx::open_rewritten(&headers, &state);
    if !is_loopback_request(peer, &headers) {
        return ctx.finish(
            html(
                StatusCode::FORBIDDEN,
                "The local user signs in on this machine only.",
            ),
            &state,
        );
    }
    if ctx.user(&state).is_none() {
        ctx.start(crate::session::local_user());
    }
    let target = q
        .string("return_to")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/".into());
    let to = safe_return_target(&target, &public_origin(&headers, &state.settings));
    ctx.finish(redirect(StatusCode::FOUND, &to, &headers), &state)
}
