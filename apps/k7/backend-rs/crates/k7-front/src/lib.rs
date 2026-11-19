//! k7-front — k7's backend: the session, the preferences, the file hub, the engine core, the built page.
//! Choices: `apps/k7/backend-rs/docs/front.md`. The thing a reader would get wrong: a request no family
//! owns is answered by `routes::last` (the page, else a 404), never handed to another process.

mod atomic;
pub mod auth_guard;
mod config;
pub mod embedded;
pub mod exit;
mod hop;
mod https;
mod js;
mod listen;
mod peer;
mod prefs_schema;
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
use axum::routing::{MethodRouter, get, post};
use hyper::Method;

/// What every handler reads: the settings, the revocations and the last routes a declined request ends in.
pub struct AppState {
    pub settings: Settings,
    pub revocations: k7_auth::Revocations,
    pub cameras: routes::cameras::Cameras,
    /// The machine's engine, when a device's host gave it to the front (`docs/local.md`).
    pub local: Option<Arc<routes::local::Local>>,
    /// Set once the front stops, which ends the answers that stream for ever.
    pub closing: tokio::sync::watch::Sender<bool>,
    /// The engine core, when the front answers it (`K7_ENGINE_CORE=front`, `docs/engine.md`).
    pub engine: Option<routes::engine::Engine>,
    /// `K7_FRONT_SECRET`, which also gates the internal seam.
    pub front_secret: String,
    /// The seams the engine slots ask, once the front can check sessions (`routes/engine_reach`).
    pub seams: Option<Arc<routes::engine_reach::Seams>>,
    /// The file hub, when the front is told to answer it (`K7_FILES=front`, `routes/files`).
    pub files: Option<Arc<routes::files::Files>>,
    /// What no family answers: the built page and the 404 (`routes/last`).
    pub last: Arc<routes::last::Last>,
}

/// Whether the front answers `method` on a sign-in path; anything else there is declined.
fn owns_auth(method: &Method, path: &str, local_user: bool) -> Option<bool> {
    let read = matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    Some(match path {
        "/auth/me" => read,
        "/auth/local" => local_user && read,
        "/auth/logout" => method == Method::POST || method == Method::OPTIONS,
        _ => return None,
    })
}

/// A route behind the session: owned when the front can check sessions, else declined.
fn sessioned(route: MethodRouter<Arc<AppState>>) -> MethodRouter<Arc<AppState>> {
    route.fallback(declined)
}

pub(crate) async fn declined(
    State(state): State<Arc<AppState>>,
    _peer: ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    routes::last::answer(state, req).await
}

/// Ahead of the guard: a method, a host or a session the front cannot answer here ends in the last routes.
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
            return declined(State(state), peer, req).await;
        }
        return next.run(req).await;
    }
    let path = req.uri().path();
    let method = req.method();
    let slots = state.seams.is_some();
    let files = state.files.as_ref().is_some_and(|f| f.owned());
    // The file hub's paths move together, on every host.
    if routes::files::is_files_path(path) {
        if files {
            return next.run(req).await;
        }
        return declined(State(state), peer, req).await;
    }
    // The engine slots' paths move together: all here, or all declined.
    if routes::engine_reach::is_slots_path(path) {
        if slots {
            return next.run(req).await;
        }
        return declined(State(state), peer, req).await;
    }
    // The engine core's paths move together: all here, or all declined.
    if routes::engine::is_engine_path(path) {
        if state.engine.is_none() || !routes::engine::owns(method, path) {
            return declined(State(state), peer, req).await;
        }
        return next.run(req).await;
    }
    let needs_session = path != "/api/health";
    let owned_method = if let Some(owned) = owns_auth(method, path, state.settings.local_user) {
        owned
    } else {
        matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
            || (path == "/api/preferences" && method == Method::PUT)
            || (path == "/api/cameras/ticket" && method == Method::POST)
            || (path == "/api/ai/settings" && method == Method::PUT)
            || (path == "/api/ai/organize/turn" && method == Method::POST)
    };
    if !owned_method || needs_session && !state.settings.claims_sessions() {
        return declined(State(state), peer, req).await;
    }
    next.run(req).await
}

/// What `build` gives `serve_with` besides the router: the state, whose hub, engine and streams it stops.
type Built = (Router, Arc<AppState>);

/// The routes, the last routes their fallback, with the hub's limits chosen and the state kept for `serve_with` to stop.
fn build(config: &Config, limits: routes::cameras::Limits, port: u16) -> Built {
    let settings = Settings::new(&config.env, config.trust_hops);
    // A trusted-proxies list fixes the hop count.
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
    let cameras = routes::cameras::Cameras::new(settings.camera_ice_servers.as_deref(), limits);
    let local = routes::local::Local::from_settings(&settings, port);
    let local_user = settings.local_user;
    // The seam's file half is this front's own.
    let seam = format!("127.0.0.1:{port}");
    let engine = routes::engine::Engine::start(&settings);
    routes::files::service::native::media::direct::keep_module_in(settings.front_cache.clone());
    let seams = routes::engine_reach::Seams::start(config, &settings, &seam);
    let files = routes::files::Files::start(config, &settings, port);
    let last = routes::last::Last::start(&settings);
    let state = Arc::new(AppState {
        settings,
        revocations,
        cameras,
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
        .route(
            "/api/health",
            get(routes::health::health).fallback(declined),
        )
        .route(
            "/api/preferences",
            sessioned(get(routes::prefs::get).put(routes::prefs::put)),
        )
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
    // Outside the web chain: the upgrades never cross the guards, the seam has its own gate.
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
            get(routes::local::internal::local).fallback(declined),
        )
        .merge(owned)
        .merge(bare)
        .fallback(declined)
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
