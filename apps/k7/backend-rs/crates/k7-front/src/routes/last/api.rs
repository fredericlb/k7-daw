//! api.rs — the library's `/api` guard, a library bearer or the session, and the
//! routes still behind it: a bearer that does not verify is refused even beside a valid cookie, no bearer needs the
//! session. What a reader gets wrong: a bearer that verifies signs nobody in, since the preferences' own `requireAuth`
//! reads the session again; the AI routes have no copy behind the guard, so a bearer there meets the 404.

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
    if let Some(token) = crate::routes::engine_reach::caller::bearer(&req) {
        if super::internal::verify_bearer(state, &token).is_none() {
            return error(StatusCode::UNAUTHORIZED, "invalid token");
        }
    } else if signed(req.headers(), &state.settings, &state.revocations).is_none() {
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
