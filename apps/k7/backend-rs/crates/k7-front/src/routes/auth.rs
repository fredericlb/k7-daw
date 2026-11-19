//! auth.rs — `GET /auth/me`, the one place a page is handed its token: a request that holds it is told who it is,
//! one that does not is given it. What a reader gets wrong: that is no sign-in, `web::guard` already refused
//! every caller that is not this machine, and a page on another site gets no cookie (`docs/sessions.md`).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CACHE_CONTROL, HeaderValue, SET_COOKIE};
use serde_json::json;

use crate::AppState;
use crate::session::{LOCAL_EMAIL, grant, holds, local_user};
use crate::web::error;

pub async fn me(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let fetch_site = req
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok());
    if fetch_site == Some("cross-site") {
        return error(StatusCode::FORBIDDEN, "cross-site request refused");
    }
    let held = holds(req.headers(), &state.settings);
    // The account's files start now, so they are warm by the time the page asks.
    if let Some(f) = state.files.clone().filter(|f| f.owned()) {
        tokio::spawn(async move { f.hub.warm(LOCAL_EMAIL).await });
    }
    let mut response = Json(json!({ "user": local_user() })).into_response();
    let h = response.headers_mut();
    h.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if !held {
        h.append(SET_COOKIE, grant(&state.settings));
    }
    response
}
