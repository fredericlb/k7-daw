//! last — the TypeScript backend's last routes, answered by the front under `K7_LAST_ROUTES=front`: the seam's
//! TypeScript half, the site page, the library's `/api` guard, the built page
//! with its fallback, and Express's 404, each as `apps/k7/backend/src/server.ts` answers.
//! What a reader gets wrong: under the switch this stands where the proxy stood, so every request no family takes
//! ends here and node is asked nothing (`apps/k7/backend-rs/docs/last.md`).

mod api;
mod dev_frontend;
pub(crate) mod express;
mod frontend;
mod internal;
mod send;
mod site;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use hyper::header::{CONNECTION, HeaderValue, SET_COOKIE};
use hyper::{HeaderMap, StatusCode};
use tower::ServiceExt;

use crate::settings::Settings;
use crate::{AppState, Config};

/// What the last routes keep between requests: where the built page is, and the dev tray's Vite.
pub struct Last {
    frontend: frontend::Frontend,
    dev: Option<dev_frontend::DevFrontend>,
}

impl Last {
    /// The routes' state when the switch is on and the file hub is here; None leaves declined requests to the proxy.
    pub fn start(config: &Config, settings: &Settings, files: bool) -> Option<Arc<Last>> {
        let asked = config.env.get("K7_LAST_ROUTES").map(|v| v.trim()) == Some("front");
        if !settings.last_routes || !files {
            if asked {
                eprintln!(
                    "k7-front: K7_LAST_ROUTES=front needs K7_FILES=front: the backend keeps its last routes"
                );
            }
            return None;
        }
        eprintln!("k7-front: the backend's last routes are answered here (K7_LAST_ROUTES=front)");
        Some(Arc::new(Last {
            frontend: frontend::Frontend::find(settings),
            dev: settings
                .dev_frontend
                .as_deref()
                .and_then(dev_frontend::DevFrontend::new),
        }))
    }
}

/// A request the front declined, answered as the backend would have answered it.
pub async fn answer(state: Arc<AppState>, peer: ConnectInfo<SocketAddr>, req: Request) -> Response {
    // The backend's upgrade guard: nothing it still serves takes a socket, so every upgrade is refused at once.
    if crate::proxy::is_upgrade(req.headers()) {
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

/// The backend's order after the chain: the site page, the `/api` guard, then the page.
async fn route(state: &Arc<AppState>, req: Request) -> Response {
    let hostname = hostname(req.headers(), &state.settings);
    let path = req.uri().path().to_string();
    if state
        .settings
        .site_hosts
        .contains(&hostname.to_ascii_lowercase())
        && !site::passes(&path)
    {
        return site::answer(&req);
    }
    if under(&path, "/api") {
        return api::answer(state, req).await;
    }
    tail(state, req).await
}

/// What every route passed on meets last: the page, then Express's 404.
async fn tail(state: &Arc<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let Some(last) = state.last.as_ref() else {
        return express::not_found(req.method(), req.uri().path(), false);
    };
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

/// `req.hostname`: the host the client addressed, its port dropped, a bracketed IPv6 literal kept whole.
fn hostname(headers: &HeaderMap, settings: &Settings) -> String {
    let host = crate::web::request_host(headers, settings);
    let offset = if host.starts_with('[') {
        host.find(']').map_or(0, |i| i + 1)
    } else {
        0
    };
    match host[offset..].find(':') {
        Some(i) => host[..offset + i].to_string(),
        None => host,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hostname_drops_its_port_only() {
        let settings = Settings::new(&Default::default(), 0);
        let mut h = HeaderMap::new();
        for (host, want) in [
            ("app.k7.rocks:8789", "app.k7.rocks"),
            ("[::1]:18789", "[::1]"),
            ("[::1]", "[::1]"),
            ("k7.rocks", "k7.rocks"),
        ] {
            h.insert(hyper::header::HOST, HeaderValue::from_static(host));
            assert_eq!(hostname(&h, &settings), want);
        }
        assert!(under("/api", "/api") && under("/api/x", "/api") && !under("/apix", "/api"));
    }
}
