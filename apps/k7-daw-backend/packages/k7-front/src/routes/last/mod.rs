//! last — what no family answers: the site page, the `/api` guard, the built page with its fallback, and the
//! 404 page.
//! What a reader gets wrong: every request no family takes ends here, so nothing is ever handed to another
//! process (`apps/k7-daw-backend/docs/last.md`).

mod api;
mod dev_frontend;
mod frontend;

pub(crate) use frontend::not_found;

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CONNECTION, HeaderValue};
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
pub async fn answer(state: Arc<AppState>, req: Request) -> Response {
    // Nothing here takes a socket, so every upgrade is refused at once.
    if crate::hop::is_upgrade(req.headers()) {
        let mut r = StatusCode::NOT_FOUND.into_response();
        r.headers_mut()
            .insert(CONNECTION, HeaderValue::from_static("close"));
        return r;
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

/// Everything after CORS.
async fn after_chain(State(state): State<Arc<AppState>>, req: Request) -> Response {
    route(&state, req).await
}

/// The order after the chain: the site page, the `/api` guard, then the page.
async fn route(state: &Arc<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    if under(&path, "/api") {
        return api::answer(state, req).await;
    }
    if let Some(to) = forge_return(req.method(), &path, req.uri().query()) {
        return redirect(&to);
    }
    tail(state, req).await
}

/// A Forgejo sign-in sent back to the root, the only loopback path its built-in client takes: on to the callback.
fn forge_return(method: &hyper::Method, path: &str, query: Option<&str>) -> Option<String> {
    let query = query.filter(|_| method == hyper::Method::GET && path == "/")?;
    let has = |k: &str| {
        url::form_urlencoded::parse(query.as_bytes()).any(|(n, v)| n == k && !v.is_empty())
    };
    let back = has("state") && (has("code") || has("error"));
    back.then(|| {
        format!(
            "{}?{query}",
            crate::routes::files::service::native::FORGE_CALLBACK
        )
    })
}

fn redirect(to: &str) -> Response {
    let mut r = StatusCode::FOUND.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        r.headers_mut().insert(hyper::header::LOCATION, v);
    }
    r.headers_mut().insert(
        hyper::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    r
}

/// What every route passed on meets last: the page, then Express's 404.
async fn tail(state: &Arc<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let last = &state.last;
    if let Some(dev) = &last.dev {
        if under(&path, "/api") || under(&path, "/auth") {
            return not_found();
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
    fn a_sign_in_back_at_the_root_goes_on_to_the_callback() {
        let get = hyper::Method::GET;
        assert_eq!(
            forge_return(&get, "/", Some("code=c&state=s")).as_deref(),
            Some("/api/files/git/forge/callback?code=c&state=s")
        );
        assert!(forge_return(&get, "/", Some("error=access_denied&state=s")).is_some());
        // The page itself, a state alone, another path or another method is the page.
        assert!(forge_return(&get, "/", None).is_none());
        assert!(forge_return(&get, "/", Some("state=s")).is_none());
        assert!(forge_return(&get, "/settings", Some("code=c&state=s")).is_none());
        assert!(forge_return(&hyper::Method::POST, "/", Some("code=c&state=s")).is_none());
    }

    #[test]
    fn under_is_a_path_boundary() {
        assert!(under("/api", "/api") && under("/api/x", "/api") && !under("/apix", "/api"));
    }
}
