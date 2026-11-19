//! engine_reach — how the engine slots reach an engine: its view in the registry, a channel through the hub
//! (`engines.rs`), the slot jobs (`engine_jobs.rs`), the request/reply over a jobs channel (`job.rs`), and who asks (`caller.rs`).
//! What a reader gets wrong: the file service's own jobs (`routes/files/service/native`) use `job.rs` too.

pub mod caller;
pub(crate) mod engine_jobs;
pub(crate) mod engines;
#[cfg(test)]
mod engines_tests;
pub(crate) mod job;
pub(crate) mod seam;

use std::sync::Arc;

use axum::Router;
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use serde_json::{Value, json};

use crate::AppState;
use crate::settings::Settings;

/// A router over the front's state.
pub type Routes = Router<Arc<AppState>>;

/// Whether `path` is the slot routes' (`docs/routes.md`).
pub fn is_slots_path(path: &str) -> bool {
    path.strip_prefix("/api/engine-slots")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The seam the slot routes ask, held once the front can check sessions and holds an engine core.
pub struct Seams {
    /// The seam's file half, asked for a bearer's subject and the account's file service's jobs.
    pub(crate) seam: seam::Seam,
}

impl Seams {
    /// The seam when the front can check sessions and holds an engine core.
    pub fn start(config: &crate::Config, settings: &Settings, seam: &str) -> Option<Arc<Seams>> {
        if !settings.claims_sessions() {
            eprintln!(
                "k7-front: the engine slots need the auth folder (STUDIO_AUTH_DIR or STUDIO_MEDIATEK); they are not answered"
            );
            return None;
        }
        if !settings.engine_core {
            eprintln!(
                "k7-front: the engine slots need K7_ENGINE_CORE=front; they are not answered"
            );
            return None;
        }
        Some(Arc::new(Seams {
            seam: seam::Seam::new(seam, &config.secret),
        }))
    }

    /// Any bearer revoked here may be one the cache still holds.
    pub fn forget_bearers(&self) {
        self.seam.forget_bearers();
    }
}

/// The front's seams, which `claim` let through only when they exist.
pub fn seams_of(state: &AppState) -> &Seams {
    state
        .seams
        .as_deref()
        .expect("claimed only with the seams answered here")
}

/// The slot routes, each declining a method it does not own.
pub fn routes() -> Routes {
    engine_jobs::routes()
}

/// A JSON answer spelt as `JSON.stringify` spells it, numbers included, with Express's content type.
pub(crate) fn answer(status: StatusCode, body: &Value) -> Response {
    let mut r = (status, k7_mediatek::json::stringify(body)).into_response();
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

/// `{ error }` with its status.
pub(crate) fn fail(status: StatusCode, message: &str) -> Response {
    answer(status, &json!({ "error": message }))
}

#[cfg(test)]
mod tests {
    use super::is_slots_path;

    #[test]
    fn knows_its_paths_and_no_other() {
        assert!(is_slots_path("/api/engine-slots"));
        assert!(is_slots_path("/api/engine-slots/e1/slots"));
        for p in ["/api/engine-slotsx", "/api/engines", "/api/shares"] {
            assert!(!is_slots_path(p), "{p}");
        }
    }
}
