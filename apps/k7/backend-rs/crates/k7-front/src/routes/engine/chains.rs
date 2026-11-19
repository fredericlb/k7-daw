//! chains.rs — naming the chain an ENGINE separates with: its own settings, read for its card and written by its
//! owner over that engine's jobs channel (`engine/k7-supervisor/docs/stemSplit.md`). What a reader gets wrong:
//! the write lands in the machine's `engine-host.json` and is read back from there, never kept on this server.

use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use k7_engine::tunnel::frames::{Kind, Root};
use serde_json::{Value, json};

use crate::AppState;
use crate::routes::engine::holder;
use crate::routes::engine_reach::caller::session_only;
use crate::routes::engine_reach::engine_jobs::OPEN_JOBS;
use crate::routes::engine_reach::engines::reach;
use crate::routes::engine_reach::job::{JOB_IDLE, JOB_TOTAL, of_type, request_job};
use crate::web::read_json;

const NOT_CONNECTED: &str =
    "that engine is not connected to this server — its supervisor holds no tunnel here";

/// `chainsBody`: `{ stems?, instrumental? }`, each a chain reference, `""` to unsay one.
pub fn chains_issues(b: &Value) -> Option<String> {
    let Some(o) = b.as_object() else {
        return Some(k7_engine::js::invalid_type("object", Some(b)));
    };
    o.iter()
        .find(|(k, v)| !matches!(k.as_str(), "stems" | "instrumental") || !v.is_string())
        .map(|(k, v)| format!("{k}: {} is no chain name", k7_engine::js::stringify(v)))
}

/// `GET /api/engines/:id/chains`: what that machine separates with, as its own settings answer.
pub async fn chains(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    if let Err(refused) = session_only(&state, &req) {
        return *refused;
    }
    match owned(&state, &id).await {
        Ok(()) => ask(&state, &id, json!({ "type": "chains" })).await,
        Err(r) => r,
    }
}

/// `PUT /api/engines/:id/chains`: its owner names them; a key left out is untouched, and `""` clears one.
pub async fn set(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    if let Err(refused) = session_only(&state, &req) {
        return *refused;
    }
    // The body is read before the gate, so a malformed one is the 400 it is rather than a 403 about ownership.
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    if let Some(e) = chains_issues(&body) {
        return fail(StatusCode::BAD_REQUEST, &e);
    }
    match owned(&state, &id).await {
        Ok(()) => ask(&state, &id, json!({ "type": "chains", "set": body })).await,
        Err(r) => r,
    }
}

/// Engine `id` as the caller sees it must be theirs: an engine's settings are its owner's to write.
#[allow(clippy::result_large_err, reason = "the refusal is the answer itself")]
async fn owned(state: &AppState, id: &str) -> Result<(), Response> {
    match reach(state).view(id, &holder()).await {
        Err(e) => {
            eprintln!("engine chains: {e}");
            Err(fail(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
        }
        Ok(None) => Err(fail(
            StatusCode::NOT_FOUND,
            &format!("no engine '{id}' is registered"),
        )),
        Ok(Some(v)) if !v.usable => Err(fail(
            StatusCode::FORBIDDEN,
            "only the engine's owner names what it separates with",
        )),
        Ok(Some(_)) => Ok(()),
    }
}

/// One `chains` request on `id`'s machine, answered with the chains as they then stand there.
async fn ask(state: &AppState, id: &str, body: Value) -> Response {
    let link = match reach(state)
        .open(id, Vec::<Root>::new(), Kind::Jobs, OPEN_JOBS)
        .await
    {
        Ok(Some(l)) => l,
        Ok(None) => return fail(StatusCode::SERVICE_UNAVAILABLE, NOT_CONNECTED),
        Err(e) => {
            eprintln!("engine chains: {e}");
            return fail(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };
    let mut link = link;
    let asked = body.as_object().cloned().unwrap_or_default();
    let reply = request_job(
        &mut link,
        asked,
        of_type("chains"),
        |_| {},
        JOB_TOTAL,
        JOB_IDLE,
    )
    .await;
    link.tx.close();
    match reply {
        Ok(r) => say(
            StatusCode::OK,
            &json!({ "chains": r.get("chains").cloned().unwrap_or_else(|| json!({})) }),
        ),
        Err(e) => fail(StatusCode::BAD_REQUEST, &e.0),
    }
}

/// A JSON answer spelt as `JSON.stringify` spells it, with Express's content type, as the media writes one.
fn say(status: StatusCode, body: &Value) -> Response {
    let mut r = (status, k7_mediatek::json::stringify(body)).into_response();
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

/// `{ error }` with its status.
fn fail(status: StatusCode, message: &str) -> Response {
    say(status, &json!({ "error": message }))
}
