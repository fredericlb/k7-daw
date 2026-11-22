//! files — the file hub answered by the front under `K7_FILES=front` (`docs/files.md`): the accounts' file
//! services and their proxy, the libraries, devices and pairing, the device door's files, `/api/ready` and the
//! seam's file half. Choices: `apps/k7-daw-backend/docs/files.md`.
//! What a reader gets wrong: without the switch the family's paths are declined, since nothing else runs a hub.

pub mod hub;
pub mod ready;
pub mod service;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::Router;

use crate::AppState;
use crate::settings::Settings;

/// A router over the front's state.
pub type Routes = Router<Arc<AppState>>;

/// The hub's own prefixes: every path under them is the family's (`docs/files.md`, "The switch").
const PREFIXES: [&str; 3] = ["/api/files", "/api/files-hub", "/api/ready"];

fn under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Whether `path` is the family's.
pub fn is_files_path(path: &str) -> bool {
    if PREFIXES.iter().any(|p| under(path, p)) {
        return true;
    }
    under(path, "/api/internal/files")
}

/// The file hub in the front, and what its routes need beside it.
pub struct Files {
    /// Set once the hub is launched.
    owned: AtomicBool,
    /// The accounts' file services, the libraries' service and the proxy to them (`hub.rs`).
    pub hub: hub::State,
    /// The accounts' native services (`service/`).
    pub service: service::Services,
}

impl Files {
    /// The family's state when the switch is on and the front can check sessions.
    pub fn start(config: &crate::Config, settings: &Settings, port: u16) -> Option<Arc<Files>> {
        if !settings.files_front {
            return None;
        }
        if !config.env.contains_key("STUDIO_FILES_ROOT")
            && std::env::var_os("STUDIO_FILES_ROOT").is_none()
        {
            eprintln!(
                "k7-front: K7_FILES=front needs STUDIO_FILES_ROOT, where the accounts' workspaces live; the file hub is not answered"
            );
            return None;
        }
        if !settings.engine_core {
            eprintln!(
                "k7-front: K7_FILES=front needs K7_ENGINE_CORE=front, whose registry asks the hub about device tokens; the file hub is not answered"
            );
            return None;
        }
        service::native::git::listening_on(port);
        Some(Arc::new(Files {
            owned: AtomicBool::new(false),
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

    /// Launch the hub, and take the family from then on.
    pub async fn launch(self: Arc<Self>, state: Arc<AppState>) {
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

/// The family's routes behind the web chain (security headers, the loopback lock, CORS).
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
        for p in ["/api/filesx", "/api/sync", "/api/readyz"] {
            assert!(!is_files_path(p), "{p}");
        }
    }
}
