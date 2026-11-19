//! caller.rs — who a request is, as the backend's guards decide it: `requireAuth`, the session alone.

use axum::extract::Request;
use axum::response::Response;
use hyper::StatusCode;
use serde_json::{Map, Value};

use crate::AppState;
use crate::session::{Signed, signed};
use crate::web::error;

/// Who asked: a page's session.
#[derive(Clone)]
pub struct Caller(Signed);

impl Caller {
    /// `req.user` as the backend's routes read it.
    pub fn user(&self) -> Option<Map<String, Value>> {
        Some(self.0.user.clone())
    }

    /// The answer, with the cookies a rewrite to the local user started.
    pub fn finish(&self, response: Response) -> Response {
        crate::session::with_cookies(response, &self.0)
    }
}

/// `requireAuth`: the session alone.
pub fn session_only(state: &AppState, req: &Request) -> Result<Caller, Box<Response>> {
    signed(req.headers(), &state.settings, &state.revocations)
        .map(Caller)
        .ok_or_else(|| Box::new(error(StatusCode::UNAUTHORIZED, "unauthenticated")))
}
