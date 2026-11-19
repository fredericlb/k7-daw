//! local — the machine's own engine, run by the front when a device's host turns it on (`K7_LOCAL_ENGINE=front`):
//! `/api/local/engine*` answered here, the supervisor started once the front listens and its registry answers, and
//! stopped before the front stops. Choices: `apps/k7/backend-rs/docs/local.md`.
//! What a reader gets wrong: `/api/local` is a device's whether or not it runs the engine.

pub mod internal;
pub mod routes;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use http_body_util::Empty;
use hyper::Method;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use k7_local::LocalEngine;

use crate::settings::Settings;

/// Whether the supervisor is: not started yet, started by the front, or stopped for good.
const PENDING: u8 = 0;
const FRONT: u8 = 1;
const STOPPED: u8 = 2;

pub struct Local {
    pub engine: Arc<LocalEngine>,
    owner: AtomicU8,
    /// What `/api/local` names: the tray's release, else the backend's package version.
    pub version: String,
}

impl Local {
    /// The local engine a device's settings ask the front, listening on `port`, to run; None runs no engine here.
    pub fn from_settings(s: &Settings, port: u16) -> Option<Arc<Self>> {
        let wanted = s.local_user && s.local_engine_front;
        let bin = s.supervisor_bin.clone().filter(|_| wanted)?;
        if !s.claims_sessions() {
            eprintln!(
                "k7-front: K7_LOCAL_ENGINE=front, but no auth folder is known: this machine runs no engine"
            );
            return None;
        }
        let engine = LocalEngine::new(k7_local::engine::Config {
            bin,
            home: s.supervisor_home.clone(),
            // The front's own address: the registry the pages see, whichever language answers it.
            registry: format!("http://127.0.0.1:{port}"),
            roots: s.engine_roots.clone(),
            env: s.supervisor_env.clone(),
            backoff: Default::default(),
            stop_grace: k7_local::engine::STOP_GRACE,
        });
        Some(Arc::new(Local {
            engine,
            owner: AtomicU8::new(PENDING),
            version: s.release.clone(),
        }))
    }

    /// Whether the front answers the engine's routes: until it stops.
    pub fn owned(&self) -> bool {
        self.owner.load(Ordering::SeqCst) != STOPPED
    }

    /// Once the front listens on `port`: the registry awaited, then the supervisor started.
    pub async fn launch(self: Arc<Self>, port: u16) {
        let http: Client<HttpConnector, Empty<Bytes>> =
            Client::builder(TokioExecutor::new()).build_http();
        let registry = format!("http://127.0.0.1:{port}");
        // The supervisor's first heartbeat goes to this registry.
        wait_ok(&http, &format!("{registry}/api/registry/hello")).await;
        if self
            .owner
            .compare_exchange(PENDING, FRONT, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        self.engine.start();
    }

    /// Before the front stops: the supervisor deregisters while the registry still answers, then stops its engine.
    pub async fn stop(&self) {
        // A launch still waiting must not start a supervisor nobody will stop.
        let was = self.owner.swap(STOPPED, Ordering::SeqCst);
        if was == FRONT {
            self.engine.stop().await;
        }
    }
}

/// Waits until `url` answers a 2xx.
async fn wait_ok(http: &Client<HttpConnector, Empty<Bytes>>, url: &str) {
    loop {
        if let Ok(uri) = url.parse::<hyper::Uri>()
            && let Ok(res) = http.get(uri).await
            && res.status().is_success()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Whether the front answers `method` on `path` itself; None when the path is not the family's.
pub fn owns(method: &Method, path: &str) -> Option<bool> {
    let read = matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    Some(match path {
        "/api/local" | "/api/local/engine" | "/api/local/engine/events" => read,
        "/api/local/engine/cmd" | "/api/local/engine/slots" => {
            *method == Method::POST || *method == Method::OPTIONS
        }
        _ => return None,
    })
}

/// Whether `path` is a device's own whatever runs its engine: the probe and the tray's status line.
pub fn is_device_path(path: &str) -> bool {
    path == "/api/local"
}

/// Whether the family's `path` sits behind the session.
pub fn needs_session(path: &str) -> bool {
    path.starts_with("/api/local/engine")
}
