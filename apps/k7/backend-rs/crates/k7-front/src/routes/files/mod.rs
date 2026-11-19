//! files — the file hub answered by the front under `K7_FILES=front` (phase 5a, `docs/rust-phase5-plan.md`): the
//! accounts' file services and their proxy, the libraries, devices and pairing,
//! the device door's files, `/api/ready` and the seam's file half. Choices: `apps/k7/backend-rs/docs/files.md`.
//! What a reader gets wrong: the switch takes the family only once the backend says it runs no hub of its own.

pub mod hub;
pub mod ready;
pub mod service;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Router;

use crate::AppState;
use crate::settings::Settings;

/// A router over the front's state.
pub type Routes = Router<Arc<AppState>>;

/// The hub's own prefixes: every path under them is the family's (`docs/files.md`, "The switch").
const PREFIXES: [&str; 3] = ["/api/files", "/api/files-hub", "/api/ready"];

/// The seam's file half that stays the backend's: its bearers, the services' bearer, and what it runs.
const SEAM_KEPT: [&str; 3] = [
    "/api/internal/files/verify-token",
    "/api/internal/files/service-token",
    "/api/internal/files/hub",
];

fn under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Whether `path` is the family's.
pub fn is_files_path(path: &str) -> bool {
    if PREFIXES.iter().any(|p| under(path, p)) {
        return true;
    }
    if under(path, "/api/internal/files") {
        return !SEAM_KEPT.contains(&path);
    }
    false
}

/// Where the front's own callers ask the seam's file half: this front once it holds the hub, the backend before.
pub fn seam_addr(config: &crate::Config, settings: &Settings, port: u16) -> String {
    if settings.files_front {
        format!("127.0.0.1:{port}")
    } else {
        config.upstream.clone()
    }
}

/// The file hub in the front, and what its routes need beside it.
pub struct Files {
    /// Set once the backend answered that it runs no hub.
    owned: AtomicBool,
    upstream: String,
    secret: String,
    /// The accounts' file services, the libraries' service and the proxy to them (`hub.rs`).
    pub hub: hub::State,
    /// The accounts' native services (`service/`).
    pub service: service::Services,
}

impl Files {
    /// The family's state when the switch is on and the front can check sessions as the backend does.
    pub fn start(config: &crate::Config, settings: &Settings, port: u16) -> Option<Arc<Files>> {
        if !settings.files_front {
            return None;
        }
        if !settings.claims_sessions() {
            eprintln!(
                "k7-front: K7_FILES=front needs the auth folder (STUDIO_AUTH_DIR or STUDIO_MEDIATEK); the file hub stays the backend's"
            );
            return None;
        }
        if !config.env.contains_key("STUDIO_FILES_ROOT")
            && std::env::var_os("STUDIO_FILES_ROOT").is_none()
        {
            eprintln!(
                "k7-front: K7_FILES=front needs STUDIO_FILES_ROOT, where the accounts' workspaces live; the file hub stays the backend's"
            );
            return None;
        }
        if !settings.engine_core {
            eprintln!(
                "k7-front: K7_FILES=front needs K7_ENGINE_CORE=front, whose registry asks the hub about device tokens; the file hub stays the backend's"
            );
            return None;
        }
        Some(Arc::new(Files {
            owned: AtomicBool::new(false),
            upstream: config.upstream.clone(),
            secret: config.secret.clone(),
            hub: hub::State::new(config, settings, port),
            service: service::Services::new(
                &config.env,
                std::sync::Arc::new(|m: &str| println!("files | {m}")),
            ),
        }))
    }

    /// Whether the front answers the file hub now.
    pub fn owned(&self) -> bool {
        self.owned.load(Ordering::Relaxed)
    }

    /// Ask the backend, until it answers, whether it runs a hub; the front takes the family only if not.
    pub async fn launch(self: Arc<Self>, state: Arc<AppState>) {
        // Under `K7_LAST_ROUTES=front` no backend is asked: the front answers for it.
        while state.last.is_none() {
            match backend_runs_hub(&self.upstream, &self.secret).await {
                Ok(true) => {
                    eprintln!(
                        "k7-front: the backend runs the file hub, so K7_FILES=front is not followed here; give it the switch too"
                    );
                    return;
                }
                Ok(false) => break,
                Err(()) => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        }
        self.owned.store(true, Ordering::Relaxed);
        eprintln!("k7-front: the file hub is answered here (K7_FILES=front)");
        hub::launch(&state).await;
    }

    /// Stop every service; called once the front stops listening.
    pub async fn stop(&self) {
        if self.owned() {
            self.service.stop().await;
            self.hub.stop().await;
        }
    }
}

/// `GET /api/internal/files/hub` at the backend: whether it runs a hub; Err while it does not answer.
async fn backend_runs_hub(upstream: &str, secret: &str) -> Result<bool, ()> {
    use http_body_util::BodyExt;
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(hyper_util::client::legacy::connect::HttpConnector::new());
    let req = hyper::Request::get(format!("http://{upstream}/api/internal/files/hub"))
        .header("x-k7-internal", secret)
        .body(axum::body::Body::empty())
        .map_err(|_| ())?;
    let ask = async {
        let res = client.request(req).await.ok()?;
        let status = res.status();
        let bytes = res.into_body().collect().await.ok()?.to_bytes();
        // An older backend does not know the question: it runs a hub.
        if status == hyper::StatusCode::NOT_FOUND {
            return Some(true);
        }
        let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        v.get("runs").and_then(serde_json::Value::as_bool)
    };
    tokio::time::timeout(Duration::from_secs(3), ask)
        .await
        .ok()
        .flatten()
        .ok_or(())
}

/// The front's file hub, which `claim` let through only when it is owned.
pub fn files_of(state: &AppState) -> &Files {
    state
        .files
        .as_deref()
        .expect("claimed only with the file hub owned")
}

/// A JSON answer spelt as `JSON.stringify` spells it, as Express's `res.json` sends it.
pub fn json_answer(status: u16, body: &serde_json::Value) -> axum::response::Response {
    use hyper::header::{CONTENT_TYPE, HeaderValue};
    let mut r =
        axum::response::Response::new(axum::body::Body::from(k7_sync::json::stringify(body)));
    *r.status_mut() =
        hyper::StatusCode::from_u16(status).unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

/// The family's routes behind the backend's chain (security headers, the loopback lock, CORS).
pub fn routes() -> Routes {
    Router::new().merge(hub::routes()).merge(ready::routes())
}

/// The engine core's routes outside the chain with the family's added: upgrades and the like, then the seam's file half, which the seam's gate guards.
pub fn with_bare((upgrades, seam): (Routes, Routes)) -> (Routes, Routes) {
    (upgrades.merge(hub::bare()), seam.merge(hub::seam()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_family_s_paths() {
        for p in [
            "/api/files",
            "/api/files/account/libraries",
            "/api/files-hub",
            "/api/files-hub/internal/libraries/sync",
            "/api/ready",
            "/api/internal/files/cold-run",
        ] {
            assert!(is_files_path(p), "{p}");
        }
        for p in [
            "/api/filesx",
            "/api/internal/files/verify-token",
            "/api/internal/files/hub",
            "/api/sync",
            "/api/readyz",
        ] {
            assert!(!is_files_path(p), "{p}");
        }
    }
}
