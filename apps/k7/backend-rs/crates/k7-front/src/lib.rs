//! k7-front — the strangler in front of k7's TypeScript backend: the routes ported to Rust answer here,
//! every other request goes through to the TypeScript backend unchanged. Choices: `apps/k7/backend-rs/docs/front.md`.
//! The thing a reader would get wrong: behind the front every TS socket is loopback, so the real peer
//! travels in `x-k7-peer`, believed only with the shared secret (`apps/k7/backend/src/front.ts`).

mod atomic;
pub mod auth_guard;
mod config;
pub mod embedded;
pub mod exit;
mod https;
mod js;
mod listen;
mod peer;
mod prefs_schema;
mod proxy;
mod routes;
mod session;
mod settings;
mod streams;
pub mod trust;
mod web;

#[cfg(test)]
mod engine_jobs_vectors;
#[cfg(test)]
mod local_vectors;
#[cfg(test)]
mod stop_tests;
#[cfg(test)]
mod turn_vectors;
#[cfg(test)]
mod vectors;

pub use config::{Config, ENV_KEYS};
pub use peer::client_address;
pub use routes::cameras::Limits as CameraLimits;
pub use settings::Settings;

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::Response;
use axum::routing::{MethodRouter, delete, get, post};
use hyper::Method;

/// What every handler reads: the backend's settings, the revocations, and the proxy a declined request goes to.
pub struct AppState {
    pub settings: Settings,
    pub revocations: k7_auth::Revocations,
    pub proxy: Arc<proxy::Proxy>,
    pub app_releases: routes::app_releases::Stores,
    pub cameras: routes::cameras::Cameras,
    /// `library-tokens.json`, shared with the backend.
    pub tokens: k7_auth::api_token::TokenFile,
    /// The machine's engine, when a device's host gave it to the front (`docs/local.md`).
    pub local: Option<Arc<routes::local::Local>>,
    /// Set once the front stops, which ends the answers that stream for ever.
    pub closing: tokio::sync::watch::Sender<bool>,
    /// The engine core, when the front answers it (`K7_ENGINE_CORE=front`, `docs/engine.md`).
    pub engine: Option<routes::engine::Engine>,
    /// `K7_FRONT_SECRET`, which also gates the internal seam.
    pub front_secret: String,
    /// The seams the engine slots ask, once the front can check sessions as the backend does (`routes/engine_reach`).
    pub seams: Option<Arc<routes::engine_reach::Seams>>,
    /// The file hub, when the front is told to answer it (`K7_FILES=front`, `routes/files`).
    pub files: Option<Arc<routes::files::Files>>,
    /// The backend's last routes, when the front is told to answer them (`K7_LAST_ROUTES=front`, `routes/last`).
    pub last: Option<Arc<routes::last::Last>>,
}

/// Whether the front answers `method` on a sign-in or token path; anything else there is the backend's.
fn owns_auth(method: &Method, path: &str, local_user: bool) -> Option<bool> {
    let read = matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    let one_token = path
        .strip_prefix("/api/token/")
        .is_some_and(|id| !id.is_empty() && !id.contains('/'));
    Some(match path {
        "/auth/me" => read,
        "/auth/local" => local_user && read,
        "/auth/logout" => method == Method::POST || method == Method::OPTIONS,
        "/api/token" => read || method == Method::POST,
        _ if one_token => method == Method::DELETE || method == Method::OPTIONS,
        _ => return None,
    })
}

/// A route behind the session: owned when the front can check sessions as the backend does, else proxied.
fn sessioned(route: MethodRouter<Arc<AppState>>) -> MethodRouter<Arc<AppState>> {
    route.fallback(proxied)
}

pub(crate) async fn proxied(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    // Under `K7_LAST_ROUTES=front` nothing goes to the backend: what it would answer is answered here.
    if state.last.is_some() {
        return routes::last::answer(state, peer, req).await;
    }
    proxy::forward(State(state.proxy.clone()), peer, req).await
}

/// Ahead of the guard: a method, a host or a session the front cannot answer as the backend would goes to it untouched.
async fn claim(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    // The local engine's family: answered here only when a device's host gave them to the front.
    let path = req.uri().path();
    let local = routes::local::owns(req.method(), path).map(|owned| {
        let runs = state.local.as_ref().is_some_and(|l| l.owned());
        let device = state.settings.local_user && routes::local::is_device_path(path);
        (owned && (runs || device), true)
    });
    if let Some((owned, family)) = local {
        let session = !family || routes::local::needs_session(path);
        if !owned || session && !state.settings.claims_sessions() {
            return proxied(State(state), peer, req).await;
        }
        return next.run(req).await;
    }
    let host = web::request_host(req.headers(), &state.settings);
    let hostname = host
        .rsplit_once(':')
        .map_or(host.as_str(), |(h, _)| h)
        .to_ascii_lowercase();
    let site = state.settings.site_hosts.contains(&hostname);
    let path = req.uri().path();
    let method = req.method();
    let slots = state.seams.is_some() && !site;
    let files = state.files.as_ref().is_some_and(|f| f.owned());
    // The file hub's paths move together, on every host: under the switch the backend runs no hub.
    if routes::files::is_files_path(path) {
        if files {
            return next.run(req).await;
        }
        return proxied(State(state), peer, req).await;
    }
    // The engine slots' paths move together: all here, or all the backend's.
    if routes::engine_reach::is_slots_path(path) {
        if slots {
            return next.run(req).await;
        }
        return proxied(State(state), peer, req).await;
    }
    // The engine core's paths move together: all here, or all the backend's.
    if routes::engine::is_engine_path(path) {
        if state.engine.is_none() || site || !routes::engine::owns(method, path) {
            return proxied(State(state), peer, req).await;
        }
        return next.run(req).await;
    }
    let releases = routes::app_releases::is_family_path(path);
    let uploads = path.starts_with("/api/app-releases/");
    let needs_session = path != "/api/health" && !releases;
    // Behind the library's `/api` guard a bearer is checked against tokens only the backend holds.
    let guarded = path == "/api/preferences"
        || path.starts_with("/api/ai/")
        || path == "/api/token"
        || path.starts_with("/api/token/");
    let bearer = req
        .headers()
        .get(hyper::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.len() >= 6 && v[..6].eq_ignore_ascii_case("bearer"));
    // An upload path owns its two writes only: the backend's bearer check answers every other method there.
    let owned_method = if let Some(owned) = owns_auth(method, path, state.settings.local_user) {
        owned
    } else if uploads {
        method == Method::PUT || method == Method::POST && path.ends_with("/publish")
    } else {
        matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
            || (path == "/api/preferences" && method == Method::PUT)
            || (path == "/api/cameras/ticket" && method == Method::POST)
            || (path == "/api/ai/settings" && method == Method::PUT)
            || (path == "/api/ai/organize/turn" && method == Method::POST)
    };
    if !owned_method
        || site && needs_session
        || guarded && bearer
        || needs_session && !state.settings.claims_sessions()
        || releases && state.settings.local_user
    {
        return proxied(State(state), peer, req).await;
    }
    next.run(req).await
}

/// What `build` gives `serve_with` besides the router: the state, whose hub, engine and streams it stops.
type Built = (Router, Arc<AppState>);

/// The routes Rust owns, the proxy their fallback, with the hub's limits chosen and the state kept for `serve_with` to stop.
fn build(config: &Config, limits: routes::cameras::Limits, port: u16) -> Built {
    let settings = Settings::new(&config.env, config.trust_hops);
    // A proxy set fixes the hop count the proxy stamps with too.
    let config = &Config {
        trust_hops: settings.trust_hops,
        ..config.clone()
    };
    let revocations = session::revocations(
        settings
            .auth_dir
            .as_deref()
            .unwrap_or(std::path::Path::new("")),
    );
    let app_releases = routes::app_releases::Stores::new(&settings.app_releases_dir);
    let cameras = routes::cameras::Cameras::new(settings.camera_ice_servers.as_deref(), limits);
    let local = routes::local::Local::from_settings(&settings, port);
    let auth_dir = settings.auth_dir.clone().unwrap_or_default();
    let tokens = routes::tokens::token_file(&auth_dir);
    let local_user = settings.local_user;
    // The seam's file half is asked of this front once it holds the hub, of the backend before.
    let seam = routes::files::seam_addr(config, &settings, port);
    let engine = routes::engine::Engine::start(&settings);
    routes::files::service::native::media::direct::keep_module_in(settings.front_cache.clone());
    let seams = routes::engine_reach::Seams::start(config, &settings, &seam);
    let files = routes::files::Files::start(config, &settings, port);
    let last = routes::last::Last::start(config, &settings, files.is_some());
    let state = Arc::new(AppState {
        settings,
        revocations,
        proxy: proxy::Proxy::new(config),
        app_releases,
        cameras,
        tokens,
        local,
        closing: tokio::sync::watch::channel(false).0,
        engine,
        front_secret: config.secret.clone(),
        seams,
        files,
        last,
    });
    let (engine_guarded, engine_bare) = routes::engine::routes();
    let owned = Router::new()
        .route("/api/health", get(routes::health::health).fallback(proxied))
        .route(
            "/api/preferences",
            sessioned(get(routes::prefs::get).put(routes::prefs::put)),
        )
        .merge(routes::app_releases::router())
        .route("/api/cameras", sessioned(get(routes::cameras::list)))
        .route(
            "/api/cameras/ticket",
            sessioned(post(routes::cameras::ticket)),
        )
        .route("/api/cameras/ws", sessioned(get(routes::cameras::socket)))
        .route(
            "/api/ai/settings",
            sessioned(get(routes::ai::get_settings).put(routes::ai::put_settings)),
        )
        .route("/api/ai/organize", sessioned(get(routes::ai::get_organize)))
        .route(
            "/api/ai/organize/turn",
            sessioned(post(routes::ai::turn::post_turn)),
        )
        .route(
            "/auth/logout",
            sessioned(post(routes::auth::account::logout)),
        )
        .route("/auth/me", sessioned(get(routes::auth::account::me)))
        .route(
            "/api/token",
            sessioned(get(routes::tokens::list).post(routes::tokens::create)),
        )
        .route("/api/token/{id}", sessioned(delete(routes::tokens::revoke)))
        .route("/api/local", get(routes::local::routes::info))
        .route("/api/local/tray", get(routes::local::routes::tray))
        .route("/api/local/engine", get(routes::local::routes::get))
        .route(
            "/api/local/engine/events",
            get(routes::local::routes::events),
        )
        .route("/api/local/engine/cmd", post(routes::local::routes::cmd))
        .route(
            "/api/local/engine/slots",
            post(routes::local::routes::slots),
        );
    // Mounted only on a device: a hosted server has no local user to sign in.
    let owned = if local_user {
        owned.route("/auth/local", sessioned(get(routes::auth::local::local)))
    } else {
        owned
    };
    let owned = owned
        .merge(engine_guarded)
        .merge(routes::engine_reach::routes())
        .merge(routes::files::routes())
        .route_layer(from_fn_with_state(state.clone(), web::guard))
        .route_layer(from_fn_with_state(state.clone(), claim));
    // Outside the web chain, as the TypeScript serves them: the upgrades never cross Express, the seam has its own gate.
    let (upgrades, seam) = routes::files::with_bare(engine_bare);
    let bare = routes::engine::gate_internal(state.clone(), (upgrades, seam))
        .route_layer(from_fn_with_state(state.clone(), claim));
    let router = Router::new()
        .route(
            "/api/front/health",
            get(|| async {
                axum::Json(serde_json::json!({ "ok": true, "front": env!("CARGO_PKG_VERSION") }))
            }),
        )
        // The internal seam's local half, with its own gate in place of the chain (`docs/local.md`).
        .route(
            "/api/internal/engine/local",
            get(routes::local::internal::local).fallback(proxied),
        )
        .merge(owned)
        .merge(bare)
        .fallback(proxied)
        .with_state(state.clone());
    let router = routes::engine::with_hooks(router, &state)
        .layer(from_fn_with_state(state.clone(), peer::screen_forwarded));
    (router, state)
}

/// Serves `config` until `shutdown` resolves; `ready` is told the address once it listens.
pub async fn serve(
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
    ready: impl FnOnce(SocketAddr),
) -> std::io::Result<()> {
    serve_with(config, routes::cameras::Limits::default(), shutdown, ready).await
}

/// `serve`, with the hub's limits chosen; the hub closes its sockets once `shutdown` resolves.
pub async fn serve_with(
    config: Config,
    limits: routes::cameras::Limits,
    shutdown: impl Future<Output = ()> + Send + 'static,
    ready: impl FnOnce(SocketAddr),
) -> std::io::Result<()> {
    serve_seen(config, limits, shutdown, ready, |_| {}).await
}

/// The host's own certificate authorities added to every outbound call's roots, before any client is built (`docs/front.md`).
fn trust_extra_cas(config: &Config) {
    // k7's git transports and libgit2's config search paths, set once before any repository is opened.
    k7_git::init();
    let setting = |k: &str| config.env.get(k).cloned().or_else(|| std::env::var(k).ok());
    match trust::configure(
        setting("K7_EXTRA_CA_CERTS").as_deref(),
        setting("NODE_EXTRA_CA_CERTS").as_deref(),
    ) {
        Ok(0) => {}
        Ok(n) => eprintln!("k7-front: trusting {n} more certificate authorities beside Mozilla's"),
        Err(e) => eprintln!(
            "k7-front: the extra certificate authorities were not read, Mozilla's alone are trusted: {e}"
        ),
    }
}

/// `serve_with`, handing `seen` the state weakly, so a test can tell the state is let go once the front stops.
async fn serve_seen(
    config: Config,
    limits: routes::cameras::Limits,
    shutdown: impl Future<Output = ()> + Send + 'static,
    ready: impl FnOnce(SocketAddr),
    seen: impl FnOnce(std::sync::Weak<AppState>),
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let addr = listener.local_addr()?;
    trust_extra_cas(&config);
    ready(addr);
    let (app, state) = build(&config, limits, addr.port());
    seen(Arc::downgrade(&state));
    // The FILE family first: the page that opens on a phone asks for its workspace list before anything else (docs/files.md).
    if let Some(f) = state.files.clone() {
        tokio::spawn(f.launch(state.clone()));
    }
    // The supervisor once this front listens and the registry it heartbeats to answers.
    if let Some(local) = state.local.clone() {
        tokio::spawn(local.launch(addr.port()));
    }
    let shutdown = async move {
        shutdown.await;
        // The supervisor first: it deregisters through this front, which must still answer.
        if let Some(local) = &state.local {
            local.stop().await;
        }
        state.closing.send_replace(true);
        state.cameras.hub.stop();
        if let Some(e) = &state.engine {
            e.core.hub.stop();
        }
        if let Some(f) = &state.files {
            f.stop().await;
        }
    };
    listen::serve(listener, app, listen::Limits::default(), shutdown).await
}
