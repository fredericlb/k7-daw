//! engine_jobs.rs — the session routes that hand an engine's machine a job: its slots, set by its owner
//! (`apps/k7/backend-rs/docs/engine/slots.md`).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, Request, State};
use axum::response::Response;
use axum::routing::{delete, get, post};
use hyper::StatusCode;
use k7_engine::js::{invalid_type, number_issues, string_issues};
use k7_engine::tunnel::frames::{Kind, Root};
use serde_json::{Map, Value, json};

use super::caller::{Caller, session_only};
use super::engines::reach;
use super::job::{JOB_IDLE, JOB_TOTAL, of_type, request_job};
use super::{Routes, answer, fail, seams_of};
use crate::AppState;
use crate::web::read_json;

/// How long a save may take, its files crossing first.
const SAVE: Duration = Duration::from_secs(10 * 60);
const NOT_CONNECTED: &str =
    "that engine is not connected to this server — its supervisor holds no tunnel here";
/// How long an engine's machine gets to take a jobs channel before the job is refused.
pub const OPEN_JOBS: Duration = Duration::from_secs(10);

pub fn routes() -> Routes {
    Router::new()
        .route(
            "/api/engine-slots/{id}/slots",
            get(slots).post(save).fallback(crate::declined),
        )
        .route(
            "/api/engine-slots/{id}/slots/{slot}/enable",
            post(enable).fallback(crate::declined),
        )
        .route(
            "/api/engine-slots/{id}/slots/{slot}/footswitch",
            post(footswitch).fallback(crate::declined),
        )
        .route(
            "/api/engine-slots/{id}/slot/disable",
            post(disable).fallback(crate::declined),
        )
        .route(
            "/api/engine-slots/{id}/slots/{slot}",
            delete(remove).fallback(crate::declined),
        )
}

fn text<'a>(user: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    user.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// `claimHolder`: the session's email, else its subject.
pub fn holder(user: &Map<String, Value>) -> String {
    text(user, "email")
        .or_else(|| text(user, "sub"))
        .unwrap_or("")
        .to_string()
}

/// `requireAuth`: the session alone.
#[allow(clippy::result_large_err, reason = "the refusal is the answer itself")]
fn session(state: &AppState, req: &Request) -> Result<Caller, Response> {
    session_only(state, req).map_err(|r| *r)
}

/// zod's issues for a body that must be an object, `check` listing the rest; None when it passes.
fn checked(b: &Value, check: impl FnOnce(&Map<String, Value>, &mut Vec<String>)) -> Option<String> {
    let Some(o) = b.as_object() else {
        return Some(invalid_type("object", Some(b)));
    };
    let mut issues = Vec::new();
    check(o, &mut issues);
    (!issues.is_empty()).then(|| issues.join("; "))
}

fn string_min1(o: &Map<String, Value>, k: &str, issues: &mut Vec<String>) {
    if let Err(e) = string_issues(o.get(k), 1, usize::MAX, None) {
        issues.extend(e);
    }
}

/// `slotSaveBody`'s verdict.
pub fn save_issues(b: &Value) -> Option<String> {
    checked(b, |o, i| string_min1(o, "ref", i))
}

/// `footswitchBody`'s verdict: 0 to 127, or null.
pub fn footswitch_issues(b: &Value) -> Option<String> {
    checked(b, |o, i| {
        if !matches!(o.get("cc"), Some(Value::Null))
            && let Err(e) = number_issues(o.get("cc"), true, 0.0, 127.0)
        {
            i.extend(e);
        }
    })
}

/// A verdict as `validate` answers it.
fn refused(issues: Option<String>) -> Option<Response> {
    issues.map(|e| fail(StatusCode::BAD_REQUEST, &e))
}

/// `owned`: engine `id` must be one this device's account may use.
#[allow(clippy::result_large_err, reason = "the refusal is the answer itself")]
async fn owned(state: &AppState, caller: &Caller, id: &str) -> Result<(), Response> {
    let user = caller.user().unwrap_or_default();
    match reach(state).view(id, &holder(&user)).await {
        Err(e) => {
            eprintln!("engine slots: {e}");
            Err(fail(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
        }
        Ok(None) => Err(fail(
            StatusCode::NOT_FOUND,
            &format!("no engine '{id}' is registered"),
        )),
        Ok(Some(v)) if !v.usable => Err(fail(
            StatusCode::FORBIDDEN,
            "only the engine's owner sets what it plays",
        )),
        Ok(Some(_)) => Ok(()),
    }
}

/// One slot job on `id`'s machine, answered with the store as it then stands.
async fn job(
    state: &AppState,
    id: &str,
    body: Value,
    roots: Vec<Root>,
    total: Option<Duration>,
) -> Response {
    let link = match reach(state).open(id, roots, Kind::Jobs, OPEN_JOBS).await {
        Ok(Some(l)) => l,
        Ok(None) => return fail(StatusCode::SERVICE_UNAVAILABLE, NOT_CONNECTED),
        Err(e) => {
            eprintln!("engine slots: {e}");
            return fail(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };
    let mut link = link;
    // A save waits for its files to cross before the machine says anything: the long wait is both limits.
    let (t, i) = total.map_or((JOB_TOTAL, JOB_IDLE), |t| (t, t));
    let body = body.as_object().cloned().unwrap_or_default();
    let reply = request_job(&mut link, body, of_type("slots"), |_| {}, t, i).await;
    link.tx.close();
    match reply {
        Ok(r) => answer(
            StatusCode::OK,
            &json!({
                "slots": r.get("slots").filter(|s| s.is_array()).cloned().unwrap_or(json!([])),
                "enabledSlot": r.get("enabledSlot").filter(|s| s.is_string()).cloned().unwrap_or(Value::Null),
            }),
        ),
        Err(e) => fail(StatusCode::BAD_REQUEST, &e.0),
    }
}

/// The caller, then `owned`, then the job `body`; the answer carries the session's cookies.
async fn owner_job(state: Arc<AppState>, req: Request, id: String, body: Value) -> Response {
    let caller = match session(&state, &req) {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(r) = owned(&state, &caller, &id).await {
        return caller.finish(r);
    }
    caller.finish(job(&state, &id, body, Vec::new(), None).await)
}

async fn slots(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    owner_job(state, req, id, json!({ "type": "slots" })).await
}

async fn enable(
    State(state): State<Arc<AppState>>,
    Path((id, slot)): Path<(String, String)>,
    req: Request,
) -> Response {
    owner_job(
        state,
        req,
        id,
        json!({ "type": "slot-enable", "slotId": slot }),
    )
    .await
}

async fn disable(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    owner_job(state, req, id, json!({ "type": "slot-disable" })).await
}

async fn remove(
    State(state): State<Arc<AppState>>,
    Path((id, slot)): Path<(String, String)>,
    req: Request,
) -> Response {
    owner_job(
        state,
        req,
        id,
        json!({ "type": "slot-remove", "slotId": slot }),
    )
    .await
}

/// `POST /:id/slots/:slot/footswitch`: `{ cc }`, 0 to 127 or null, checked before the owner.
async fn footswitch(
    State(state): State<Arc<AppState>>,
    Path((id, slot)): Path<(String, String)>,
    req: Request,
) -> Response {
    let caller = match session(&state, &req) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return caller.finish(r),
    };
    if let Some(r) = refused(footswitch_issues(&body)) {
        return caller.finish(r);
    }
    if let Err(r) = owned(&state, &caller, &id).await {
        return caller.finish(r);
    }
    let body = json!({ "type": "slot-footswitch", "slotId": slot, "cc": body["cc"] });
    caller.finish(job(&state, &id, body, Vec::new(), None).await)
}

/// `POST /:id/slots`: `{ ref }`, the account's chain compiled by its file service, then saved on the machine.
async fn save(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let caller = match session(&state, &req) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return caller.finish(r),
    };
    if let Some(r) = refused(save_issues(&body)) {
        return caller.finish(r);
    }
    if let Err(r) = owned(&state, &caller, &id).await {
        return caller.finish(r);
    }
    let user = caller.user().unwrap_or_default();
    let email = user.get("email").and_then(Value::as_str).unwrap_or("");
    let compiled = seams_of(&state)
        .seam
        .file_hub_job("/slot", json!({ "user": email, "ref": body["ref"] }))
        .await;
    let compiled = match compiled {
        Ok(c) => c,
        Err(e) => return caller.finish(fail(StatusCode::BAD_REQUEST, &e)),
    };
    let roots: Vec<Root> =
        serde_json::from_value(compiled.get("roots").cloned().unwrap_or(json!([])))
            .unwrap_or_default();
    let body = json!({ "type": "slot-save", "slot": compiled.get("slot").cloned().unwrap_or(Value::Null) });
    caller.finish(job(&state, &id, body, roots, Some(SAVE)).await)
}
