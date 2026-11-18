//! account.rs — `/auth/logout` and `/auth/me`: the session the cookie holds, and the end of it.
//! What a reader gets wrong: a logout everywhere is written where the backend reads it, so both processes drop the session.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CACHE_CONTROL, HeaderValue};
use serde_json::{Value, json};

use super::ctx::{Ctx, now_ms};
use super::express::Query;
use super::target::is_same_origin_request;
use crate::AppState;
use crate::web::{error, read_json};

pub async fn logout(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let q = Query::parse(req.uri().query());
    let (headers, _) = match read_json(req).await {
        Ok(read) => read,
        Err(refused) => return refused,
    };
    let mut ctx = Ctx::open(&headers, &state);
    if !is_same_origin_request(&headers, &state.settings) {
        return ctx.finish(
            error(StatusCode::FORBIDDEN, "cross-site request refused"),
            &state,
        );
    }
    if q.string("everywhere").as_deref() == Some("1")
        && let Some(Value::String(sub)) = ctx.user(&state).and_then(|u| u.get("sub").cloned())
        && let Err(e) = state.revocations.revoke(&sub, now_ms())
    {
        eprintln!("session revocations: {e}");
    }
    ctx.bag.clear();
    ctx.finish(Json(json!({ "ok": true })).into_response(), &state)
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    let ctx = Ctx::open(req.headers(), &state);
    let user = ctx.user(&state);
    let mut response = match &user {
        Some(user) => Json(json!({ "user": user })).into_response(),
        None => error(StatusCode::UNAUTHORIZED, "unauthenticated"),
    };
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let response = ctx.finish(response, &state);
    if let Some(email) = user
        .as_ref()
        .and_then(|u| u.get("email"))
        .filter(|e| crate::js::truthy(e))
    {
        super::warm::files(&state, peer, &req, email.as_str().unwrap_or(""));
    }
    response
}
