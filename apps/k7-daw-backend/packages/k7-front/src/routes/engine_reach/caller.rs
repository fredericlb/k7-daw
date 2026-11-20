//! caller.rs — who a request is, as the backend's guards decide it: `requireAuth`, the session alone.

use axum::extract::Request;
use axum::response::Response;
use hyper::StatusCode;

use crate::AppState;
use crate::session::holds;
use crate::web::error;

/// `requireAuth`: the session alone.
pub fn session_only(state: &AppState, req: &Request) -> Result<(), Box<Response>> {
    if holds(req.headers(), &state.settings) {
        Ok(())
    } else {
        Err(Box::new(error(StatusCode::UNAUTHORIZED, "unauthenticated")))
    }
}
