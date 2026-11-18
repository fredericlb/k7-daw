//! warm.rs — a signed-in `/auth/me` starts the account's file service, which the TypeScript backend runs: the
//! front asks the backend the same question in the background and drops its answer, so the backend's own
//! `/auth/me` hook warms the files as it did before the front answered (`apps/k7/backend-rs/docs/routes.md`).

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use hyper::Method;

use crate::AppState;

/// Sends `GET /auth/me` with the request's headers to the backend, without waiting for it; under `K7_FILES=front`, warms this hub.
pub fn files(state: &AppState, peer: ConnectInfo<SocketAddr>, req: &Request, email: &str) {
    if let Some(f) = state.files.clone().filter(|f| f.owned()) {
        let email = email.to_string();
        tokio::spawn(async move { f.hub.warm(&email).await });
        return;
    }
    // Under `K7_LAST_ROUTES=front` the backend runs no file service to warm.
    if state.last.is_some() {
        return;
    }
    let mut copy = Request::new(Body::empty());
    *copy.method_mut() = Method::GET;
    *copy.uri_mut() = req.uri().clone();
    *copy.headers_mut() = req.headers().clone();
    copy.headers_mut().remove(hyper::header::CONTENT_LENGTH);
    let proxy = state.proxy.clone();
    tokio::spawn(async move {
        let response = crate::proxy::forward(State(proxy), peer, copy).await;
        let _ = axum::body::to_bytes(response.into_body(), 64 * 1024).await;
    });
}
