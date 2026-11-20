//! refuse.rs — what a request gets where a native piece declines: a refusal with a sentence, never another answer (4-7).
//! What a reader gets wrong: the reason is noted by whoever declines and read back by the one place that refuses; a
//! route no piece owns is not refused but answered 404.

use std::future::Future;
use std::sync::{Arc, Mutex};

use axum::response::Response;
use hyper::header::{HeaderValue, RETRY_AFTER};
use serde_json::json;

use super::{Piece, bench, chains, media, prefs_routes, projects, shares, workspaces};
use crate::routes::files::json_answer;

/// Every reason a native piece gives for declining, each with the status and the sentence it is refused with.
const REASONS: &[(&str, u16, &str)] = &[
    (
        "busy",
        503,
        "the file service is busy; ask again in a moment",
    ),
    (
        "library-active",
        409,
        "the open project is inside a library, which this request does not read",
    ),
    (
        "long-path",
        422,
        "the path is longer than the service reads",
    ),
    (
        "lossy-text",
        422,
        "a file of the answer is not valid UTF-8 and cannot be sent as text",
    ),
    (
        "no-libraries",
        409,
        "the account has no libraries folder yet",
    ),
    (
        "not-regular",
        422,
        "the path is not a regular file or folder",
    ),
    (
        "surfaces-outside",
        422,
        "a surfaces folder points outside the project",
    ),
    (
        "too-large",
        413,
        "the file or the listing is larger than the service answers",
    ),
    (
        "too-deep",
        422,
        "the folders are nested deeper than the service reads",
    ),
    (
        "too-many",
        422,
        "the folder holds more entries than the service reads",
    ),
    (
        "would-throw",
        422,
        "the request asks for what this server does not do: a name it cannot read, or a write it cannot make",
    ),
    ("service", 503, "file service: the runtime did not answer"),
    ("lost", 503, "file service: the answer was lost"),
    (
        "no-capture",
        409,
        "this account's workspaces are not captured here",
    ),
    (
        "retired",
        503,
        "file service: the compile session was retired while it ran",
    ),
    (
        "pins",
        409,
        "the bench's library pins are not written yet; open the chains page once",
    ),
    (
        "no-session",
        503,
        "file service: no compile session is open",
    ),
    (
        "no-docs",
        503,
        "file service: the compiler is not available",
    ),
    ("panic", 500, "internal error"),
    ("ask-shape", 500, "internal error"),
    ("doc-shape", 500, "internal error"),
    ("engine-shape", 500, "internal error"),
    ("catalogue", 500, "internal error"),
    ("no-view", 500, "internal error"),
    ("compile", 500, "internal error"),
    ("graph", 500, "internal error"),
    ("workspace", 500, "internal error"),
    ("mounts", 500, "internal error"),
    ("vocabulary", 500, "internal error"),
    (
        "clip-file",
        422,
        "a clip names a file the compile cannot read",
    ),
    (
        "media",
        503,
        "file service: the media piece is not available",
    ),
    ("body", 400, "the request body is not one this route reads"),
    ("refused", 409, "the request was refused"),
];

/// The reasons the table knows, for a test that holds every decline to a sentence.
pub fn known() -> impl Iterator<Item = &'static str> {
    REASONS.iter().map(|(why, _, _)| *why)
}

/// What a request was refused with: its status and sentence.
pub fn sentence(why: &str) -> (u16, String) {
    match REASONS.iter().find(|(w, _, _)| *w == why) {
        Some((_, status, text)) => (*status, (*text).to_string()),
        None => (409, format!("the request was declined ({why})")),
    }
}

/// The refusal for `why`: its status, `{error, declined}`, and `Retry-After` where it is a wait.
pub fn response(why: &str) -> Response {
    let (status, text) = sentence(why);
    let mut r = json_answer(status, &json!({ "error": text, "declined": why }));
    if why == "busy" {
        r.headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from_static("1"));
    }
    r
}

/// The reason a request was declined for, shared by every task the request spawns.
#[derive(Clone, Default)]
struct Why(Arc<Mutex<Option<&'static str>>>);

tokio::task_local! {
    static CURRENT: Why;
}

/// `work` with a place for the reason it is declined for.
pub async fn scoped<F: Future>(work: F) -> F::Output {
    CURRENT.scope(Why::default(), work).await
}

/// `work` as a task of its own that still notes its declines on the request that spawned it.
pub fn carry<F: Future>(work: F) -> impl Future<Output = F::Output> {
    let why = CURRENT.try_with(Clone::clone).ok();
    async move {
        match why {
            Some(w) => CURRENT.scope(w, work).await,
            None => work.await,
        }
    }
}

/// `tokio::spawn` of a task that notes its declines on the request that spawned it.
pub fn spawn<F>(work: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(carry(work))
}

/// A decline noted for the refusal that may follow.
pub fn note(why: &str) {
    let why = known().find(|w| *w == why).unwrap_or("refused");
    let _ = CURRENT.try_with(|w| *w.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(why));
}

/// The reason last noted on this request, or a generic one.
pub fn last() -> &'static str {
    CURRENT
        .try_with(|w| *w.0.lock().unwrap_or_else(|p| p.into_inner()))
        .ok()
        .flatten()
        .unwrap_or("refused")
}

/// The refusal for the reason last noted.
pub fn refused() -> Response {
    response(last())
}

/// The piece a hooked route belongs to, which refuses it when it declines; `None` for a route no piece owns.
pub fn hook_piece(method: &hyper::Method, path: &str) -> Option<Piece> {
    use hyper::Method;
    let get = method == Method::GET;
    let post = method == Method::POST;
    if workspaces::Kind::of(method, path).is_some() {
        return Some(Piece::Workspaces);
    }
    if projects::Kind::of(method, path).is_some() {
        return Some(Piece::Projects);
    }
    if media::routes::Kind::of(method, path).is_some() {
        return Some(Piece::Media);
    }
    if chains::tag_of(method, path).is_some() || bench::owns(method, path) {
        return Some(Piece::Chains);
    }
    if shares::routes::tag_of(method, path).is_some() {
        return Some(Piece::Shares);
    }
    if prefs_routes::owns(method, path) {
        return Some(Piece::Entries);
    }
    match path {
        "/workspace" | "/engine/served" if get => Some(Piece::Engine),
        "/engine/project" | "/engine/unload" | "/engine/sync/retry" if post => Some(Piece::Engine),
        "/plugin-params" | "/plugin-native-ui" if get => Some(Piece::EngineJobs),
        "/module-base/snapshot" | "/preview" | "/auto-freeze/thaw" if post => {
            Some(Piece::EngineJobs)
        }
        "/module-libraries" if get => Some(Piece::Listings),
        "/logs" if get => Some(Piece::Runtime),
        _ => None,
    }
}

/// Whether a piece owns the hooked route `method path`: a request none of the hooks answered is then refused, not a 404.
pub fn owned(method: &hyper::Method, path: &str) -> bool {
    hook_piece(method, path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_sentence_and_busy_is_a_wait() {
        for why in known() {
            let (status, text) = sentence(why);
            assert!(!text.is_empty() && status >= 400, "{why}");
            let r = response(why);
            assert_eq!(r.status().as_u16(), status, "{why}");
            assert_eq!(
                r.headers().contains_key(RETRY_AFTER),
                why == "busy",
                "{why}"
            );
        }
        assert_eq!(sentence("busy").0, 503);
    }

    #[tokio::test]
    async fn the_reason_noted_in_a_task_the_request_spawned_is_the_requests() {
        scoped(async {
            note("too-many");
            carry(async { note("busy") }).await;
            tokio::spawn(carry(async { note("lost") })).await.unwrap();
            assert_eq!(last(), "lost");
        })
        .await;
        assert_eq!(last(), "refused");
    }
}
