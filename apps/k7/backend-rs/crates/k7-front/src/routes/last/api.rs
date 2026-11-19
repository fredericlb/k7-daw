//! api.rs — the `/api` guard, the session, and the routes still behind it.
//! What a reader gets wrong: the AI routes have no copy behind the guard, so a session there meets the 404.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::Response;
use hyper::{Method, StatusCode};

use crate::AppState;
use crate::routes::prefs;
use crate::session::signed;
use crate::web::error;

/// A request under `/api` no family took.
pub async fn answer(state: &Arc<AppState>, req: Request) -> Response {
    if signed(req.headers(), &state.settings, &state.revocations).is_none() {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let read = matches!(*req.method(), Method::GET | Method::HEAD);
    let put = req.method() == Method::PUT;
    let s = State(state.clone());
    match req.uri().path() {
        "/api/preferences" if read => prefs::get(s, req).await,
        "/api/preferences" if put => prefs::put(s, req).await,
        _ => super::tail(state, req).await,
    }
}
