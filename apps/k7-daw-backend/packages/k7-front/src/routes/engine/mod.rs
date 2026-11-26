//! engine — the registry's routes, the relay, the tunnel hub's upgrades and the internal seam, answered by
//! `k7-engine` when the engine core is ON (`K7_ENGINE_CORE=front`), declined to the backend, which answers 404, when
//! OFF. Which paths move together and why: `apps/k7-daw-backend/docs/engine.md`.

mod chains;
#[cfg(test)]
mod chains_tests;
#[cfg(feature = "test-hooks")]
pub mod hooks;
mod internal;
mod registry;
pub(crate) mod upgrade;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::response::Response;
use axum::routing::{MethodRouter, get, post};
use hyper::Method;
use hyper::StatusCode;
use k7_engine::Core;
use k7_engine::log::{Sink, Throttled};

use crate::AppState;
use crate::session::{LOCAL_EMAIL, holds};
use crate::settings::Settings;
use crate::web::error;

/// A router over the front's state.
pub type Routes = Router<Arc<AppState>>;

/// The engine core and what its routes need beside it.
pub struct Engine {
    pub core: Arc<Core>,
    /// Once a minute per (engine, complaint).
    pub complain: Throttled,
    pub log: Sink,
}

/// Whether the core answers `method` on `path` (the engine core ON); anything else there is the backend's.
pub fn owns(method: &Method, path: &str) -> bool {
    let get = matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    let post = *method == Method::POST || *method == Method::OPTIONS;
    let id_then = |prefix: &str, rest: &str| {
        path.strip_prefix(prefix)
            .and_then(|p| p.strip_suffix(rest))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
    };
    match path {
        "/api/registry/hello" | "/api/engines" => get,
        "/api/registry/heartbeat"
        | "/api/registry/deregister"
        | "/api/engine/select"
        | "/api/engine/release" => post,
        "/api/engine-tunnel" => get,
        _ if path.starts_with("/api/engine-tunnel/local/") => get,
        _ if id_then("/api/engine/", "/ws") => get,
        _ if id_then("/api/engines/", "/chains") => get || *method == Method::PUT,
        _ => internal::owns(method, path),
    }
}

/// Whether `path` is the engine core's: the registry, the relay, the hub, the seam's engine half.
pub fn is_engine_path(path: &str) -> bool {
    path.starts_with("/api/registry/")
        || path == "/api/engines"
        || path.starts_with("/api/engines/")
        || path.starts_with("/api/engine/")
        || path == "/api/engine-tunnel"
        || path.starts_with("/api/engine-tunnel/")
        || path.starts_with("/api/internal/engine/")
}

impl Engine {
    /// The core, when the front is told to answer it and can check sessions as the backend does.
    pub fn start(settings: &Settings) -> Option<Engine> {
        if !settings.engine_core {
            return None;
        }
        let clock = k7_engine::Clock::default();
        let log = k7_engine::log::registry_sink();
        #[cfg_attr(not(feature = "test-hooks"), allow(unused_mut))]
        let mut opts = k7_engine::CoreOptions {
            registry: k7_engine::registry::Options {
                state_dir: Some(settings.engines_dir.clone()),
                ..Default::default()
            },
            uploads: settings.upload_limits,
            ..Default::default()
        };
        #[cfg(feature = "test-hooks")]
        hooks::configure(&mut opts);
        let core = k7_engine::Core::new(opts, clock.clone());
        Some(Engine {
            core,
            complain: Throttled::new(log.clone(), clock),
            log,
        })
    }
}

/// The seam's engine routes behind their own gate.
pub fn gate_internal(state: Arc<AppState>, bare: (Routes, Routes)) -> Routes {
    let (upgrades, seam) = bare;
    upgrades.merge(internal::gated(state, seam))
}

/// The testbed's hooks, in a build with them; the router unchanged otherwise.
pub fn with_hooks(router: Router, state: &Arc<AppState>) -> Router {
    #[cfg(feature = "test-hooks")]
    return hooks::mount(router, state.clone());
    #[cfg(not(feature = "test-hooks"))]
    {
        let _ = state;
        router
    }
}

/// The core's routes: the guarded ones, and those outside the backend's chain (upgrades, the seam).
pub fn routes() -> (Routes, (Routes, Routes)) {
    let p = |m: MethodRouter<Arc<AppState>>| m.fallback(crate::declined);
    let guarded = Router::new()
        .route("/api/registry/hello", p(get(registry::hello)))
        .route("/api/registry/heartbeat", p(post(registry::heartbeat)))
        .route("/api/registry/deregister", p(post(registry::deregister)))
        .route("/api/engines", p(get(registry::engines)))
        .route("/api/engine/select", p(post(registry::select)))
        .route("/api/engine/release", p(post(registry::release)))
        .route(
            "/api/engines/{id}/chains",
            p(get(chains::chains).put(chains::set)),
        );
    let upgrades = Router::new()
        .route("/api/engine/{id}/ws", p(get(upgrade::relay)))
        .route("/api/engine-tunnel", p(get(upgrade::tunnel)))
        .route("/api/engine-tunnel/local/{key}", p(get(upgrade::local)));
    (guarded, (upgrades, internal::routes()))
}

/// The core of a request's front, which `claim` let through only when it is on.
pub fn engine_of(state: &AppState) -> &Engine {
    state
        .engine
        .as_ref()
        .expect("claimed only with the engine core on")
}

/// `claimHolder`: the device's one account.
pub fn holder() -> String {
    LOCAL_EMAIL.to_string()
}

/// `requireAuth`, then `answer` with the session; the cookies a device's rewrite started go with it.
pub fn with_session(
    state: &AppState,
    req: &Request,
    answer: impl FnOnce() -> Response,
) -> Response {
    if holds(req.headers(), &state.settings) {
        answer()
    } else {
        error(StatusCode::UNAUTHORIZED, "unauthenticated")
    }
}

/// `req.ip` as the backend reads it behind the front.
pub fn client_ip(req: &Request, peer: SocketAddr, settings: &Settings) -> String {
    let forwarded: Vec<&str> = req
        .headers()
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    let joined = forwarded.join(",");
    crate::peer::client_address(
        peer.ip(),
        (!forwarded.is_empty()).then_some(joined.as_str()),
        settings.trust_hops,
    )
}
