//! last — what no family answers: the seam's TypeScript-era half (the bearer's `verify-token`), the site page, the
//! library's `/api` guard, the built page with its fallback, and the 404 page.
//! What a reader gets wrong: every request no family takes ends here, so nothing is ever handed to another
//! process (`apps/k7/backend-rs/docs/last.md`).

mod api;
mod dev_frontend;
pub(crate) mod express;
mod frontend;
mod internal;
mod send;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CONNECTION, HeaderValue, SET_COOKIE};
use tower::ServiceExt;

use crate::AppState;
use crate::settings::Settings;

/// What the last routes keep between requests: where the built page is, and the dev tray's Vite.
pub struct Last {
    frontend: frontend::Frontend,
    dev: Option<dev_frontend::DevFrontend>,
}

impl Last {
    /// The routes' state: where the built page is, and the dev tray's Vite when one is named.
    pub fn start(settings: &Settings) -> Arc<Last> {
        Arc::new(Last {
            frontend: frontend::Frontend::find(settings),
            dev: settings
                .dev_frontend
                .as_deref()
                .and_then(dev_frontend::DevFrontend::new),
        })
    }
}

/// A request no family took, answered here.
pub async fn answer(state: Arc<AppState>, peer: ConnectInfo<SocketAddr>, req: Request) -> Response {
    // Nothing here takes a socket, so every upgrade is refused at once.
    if crate::hop::is_upgrade(req.headers()) {
        let mut r = StatusCode::NOT_FOUND.into_response();
        r.headers_mut()
            .insert(CONNECTION, HeaderValue::from_static("close"));
        return r;
    }
    let path = req.uri().path();
    // The seam sits ahead of the loopback lock and CORS, behind a gate of its own.
    if path == internal::INTERNAL || path.starts_with("/api/internal/") {
        return internal::answer(&state, peer.0, req).await;
    }
    // A router built per request, so nothing the state keeps holds the state.
    let chain = Router::new()
        .fallback(after_chain)
        .layer(from_fn_with_state(state.clone(), crate::web::guard))
        .with_state(state);
    match chain.oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    }
}

/// Everything after CORS: on a device, any other account's session is rewritten as the local user's.
async fn after_chain(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let rewrite = state
        .settings
        .local_user
        .then(|| crate::session::signed(req.headers(), &state.settings, &state.revocations))
        .flatten()
        .map(|s| s.set_cookies)
        .unwrap_or_default();
    let mut response = route(&state, req).await;
    if !response.headers().contains_key(SET_COOKIE) {
        for c in rewrite {
            response.headers_mut().append(SET_COOKIE, c);
        }
    }
    response
}

/// The order after the chain: the site page, the `/api` guard, then the page.
async fn route(state: &Arc<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    if under(&path, "/api") {
        return api::answer(state, req).await;
    }
    tail(state, req).await
}

/// What every route passed on meets last: the page, then Express's 404.
async fn tail(state: &Arc<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let last = &state.last;
    if let Some(dev) = &last.dev {
        if under(&path, "/api") || under(&path, "/auth") {
            return express::not_found(req.method(), req.uri().path(), false);
        }
        return dev.forward(req).await;
    }
    last.frontend.answer(req).await
}

/// `prefix` itself or a path below it, as Express mounts a router.
fn under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_is_a_path_boundary() {
        assert!(under("/api", "/api") && under("/api/x", "/api") && !under("/apix", "/api"));
    }
}
