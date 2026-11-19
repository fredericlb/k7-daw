//! hub.rs — the file hub of the front: the hello's ticket, `/api/files` answered by a device's account runtime behind it (410 on a
//! server), the libraries, the cut's context (`k7-hub`). The runtimes call back on `/api/files-hub/internal` with their secret. Choices: `apps/k7/backend-rs/docs/files.md`.
//! What a reader gets wrong: `/api/files` has no session guard; the ticket is its gate, the session its fallback.

mod answers;
mod proxy;
mod spawn;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use axum::Router;
use axum::routing::{any, get, post};
use k7_hub::Log;
use k7_hub::account::Names;
use k7_hub::seam::Seam;
use serde_json::{Value, json};

use super::Routes;
use crate::AppState;
use crate::settings::Settings;

/// The account the libraries' service is held under; every real account's slug ends in `-<hash>`, this one cannot.
const LIBRARIES: &str = "_libraries";
/// The home a server's cut resolves its chain in, under the data root; no account's slug can be it either.
const CUT: &str = "_cut";
/// How long a standalone's files wait for its libraries before they answer anyway (`docs/files.md`, "The libraries' service").
const LIBRARY_WAIT: Duration = Duration::from_secs(45);

/// What another slice reads of an account whose runtime calls back.
#[derive(Clone, Debug)]
pub struct Instance {
    pub slug: String,
    pub user: String,
    pub home: PathBuf,
    pub secret: String,
}

/// What this part of the family holds.
pub struct State {
    data_root: PathBuf,
    mediatek_root: PathBuf,
    /// Signs the tickets: `${SESSION_SECRET}:files`, or drawn per process.
    ticket_key: Vec<u8>,
    /// What the relay recognises as the `_libraries` account; drawn per process, never written anywhere.
    library_secret: String,
    /// Whether this front is a device's (`STUDIO_LOCAL_USER`), not a server.
    device: bool,
    /// How accounts are named and what the host adds to each one's environment.
    pub(crate) names: Arc<Names>,
    /// Each account's engine, held here whether its runtime exists yet or not.
    pub(crate) bindings: Arc<k7_hub::binding::Bindings>,
    /// How the libraries' runtime is named and given its environment.
    pub(crate) libraries: Arc<Names>,
    /// How a server's cut context is named: the libraries' folders, no account's.
    pub(crate) cut: Arc<Names>,
    /// The origin's libraries, made at launch where a code root is (`service/libraries.rs`).
    pub(crate) library_native: OnceLock<Arc<super::service::libraries::Native>>,
    pub(crate) seam: Arc<Seam>,
    log: Log,
    /// The front, for the callbacks that reach its state; set at launch.
    app: Arc<OnceLock<Weak<AppState>>>,
    /// The media bearers minted here, by id, for as long as this process lives.
    service_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// Set once a standalone's libraries settled, or waited long enough.
    settled: Arc<AtomicBool>,
    started: OnceLock<Instant>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl State {
    pub fn new(config: &crate::Config, settings: &Settings, port: u16) -> Self {
        spawn::state(config, settings, port)
    }

    /// The account whose runtime holds `secret`: one of this hub's own calling back.
    pub async fn account_of_secret(&self, secret: &str) -> Option<Instance> {
        let a = self.files()?.service.by_secret(secret)?;
        Some(Instance {
            slug: a.slug,
            user: a.user,
            home: a.home,
            secret: a.secret,
        })
    }

    /// The family, once launched: what the hub's own callbacks reach the accounts through.
    fn files(&self) -> Option<Arc<super::Files>> {
        self.app.get().and_then(Weak::upgrade)?.files.clone()
    }

    /// What the relay recognises as the `_libraries` account.
    pub fn library_secret(&self) -> &str {
        &self.library_secret
    }

    /// Whether this front is a device's: only a device makes an account runtime.
    pub fn is_device(&self) -> bool {
        self.device
    }

    /// `<dataRoot>/<slug>`.
    pub fn account_dir(&self, slug: &str) -> PathBuf {
        self.data_root.join(slug)
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    /// What `/api/ready` reports of the hub: the accounts' runtimes held now; nothing starts, so nothing waits or is capped.
    pub fn status(&self) -> Value {
        let running = self.files().map_or(0, |f| f.service.running());
        json!({
            "running": running,
            "starting": 0,
            "cap": k7_hub::account::REPORTED_CAP,
            "oldestStartMs": 0,
            "lastStartError": Value::Null,
        })
    }

    /// The account's runtime, made now rather than by its first request; a failure is that request's.
    pub async fn warm(&self, user: &str) {
        if !self.is_device() {
            return;
        }
        let Some(f) = self.files() else {
            return;
        };
        if let Some(why) = f.service.refusal(user) {
            return (self.log)(&format!("not warming {user}: {why}"));
        }
        if let Err(e) = f.service.held(user, &self.names).await {
            (self.log)(&format!("warming {user}'s runtime: {e}"));
        }
    }

    /// The subject of a media bearer this hub minted for a service, or None.
    pub fn verify_service_token(&self, secret: &str, token: &str) -> Option<String> {
        let c = k7_auth::api_token::claims(secret, token)?;
        let held = self
            .service_tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        (c.exp == 0.0 && held.get(&c.jti) == Some(&c.sub)).then_some(c.sub)
    }

    /// Whether a standalone's files answer yet: once its libraries settled, or after LIBRARY_WAIT.
    fn ready(&self) -> bool {
        self.settled.load(Ordering::Relaxed)
            || self
                .started
                .get()
                .is_some_and(|t| t.elapsed() >= LIBRARY_WAIT)
    }

    /// Saves and stops what it holds.
    pub async fn stop(&self) {
        for t in self
            .tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
        {
            t.abort();
        }
    }
}

/// Started once the front holds the family.
pub async fn launch(state: &Arc<AppState>) {
    spawn::launch(state).await;
}

/// Its routes behind the chain.
pub fn routes() -> Routes {
    let p = |m: axum::routing::MethodRouter<Arc<AppState>>| m.fallback(crate::declined);
    Router::new()
        .route("/api/files-hub", p(get(answers::hello)))
        .route("/api/files-hub/internal", any(answers::internal))
        .route("/api/files-hub/internal/{*rest}", any(answers::internal))
        .route("/api/files", any(proxy::files))
        .route("/api/files/{*rest}", any(proxy::files))
}

/// Its routes outside the chain.
pub fn bare() -> Routes {
    Router::new()
}

/// Its share of the seam's file half, under `/api/internal/files`, behind the seam's gate.
pub fn seam() -> Routes {
    Router::new()
        .route("/api/internal/files/cold-run", post(answers::cold_run))
        .route("/api/internal/files/slot", post(answers::slot))
}
