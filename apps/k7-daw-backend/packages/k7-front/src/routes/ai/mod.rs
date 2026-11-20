//! ai/ — `/api/ai/settings`, `GET /api/ai/organize` and `POST /api/ai/organize/turn`: the account's Claude key and model
//! in one small file under `<prefs>/ai`, re-read on every request.
//! The thing a reader would get wrong: the turn (`turn.rs`) makes ONE model turn; the conversation and every check are
//! the page's (`apps/k7-daw-frontend/docs/aiOrganize.md`).

pub mod claude;
pub mod sse;
pub mod turn;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use serde_json::{Map, Value, json};

use crate::AppState;
use crate::atomic::write_atomic;
use crate::js::{invalid_type, trim, truthy, utf16_len};
use crate::routes::prefs::{file_for, pretty};
use crate::session::{LOCAL_SUB, holds};
use crate::web::{error, read_json_within};

/// The model asked for when the account names none (`DEFAULT_AI_MODEL`).
pub const DEFAULT_AI_MODEL: &str = "claude-opus-5";

/// The limit every `/api/ai` body is parsed under.
pub const AI_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// The longest model name a patch may set, in UTF-16 units.
const MODEL_MAX: usize = 200;

/// `sk-ant-…wxyz`: seven units, an ellipsis and the last four; a short key is only `…`.
pub fn hint_of(key: &str) -> String {
    let units: Vec<u16> = key.encode_utf16().collect();
    if units.len() <= 16 {
        return "…".into();
    }
    let head = String::from_utf16_lossy(&units[..7]);
    let tail = String::from_utf16_lossy(&units[units.len() - 4..]);
    format!("{head}…{tail}")
}

/// `viewOf`: the model, the default, and the key only as a hint.
fn view_of(s: Option<&Map<String, Value>>) -> Value {
    let model = s
        .and_then(|s| s.get("model"))
        .filter(|m| truthy(m))
        .cloned();
    let key = s.and_then(|s| s.get("apiKey")).filter(|k| truthy(k));
    json!({
        "model": model.unwrap_or(Value::Null),
        "defaultModel": DEFAULT_AI_MODEL,
        "keyHint": key.map(|k| hint_of(k.as_str().unwrap_or(""))),
    })
}

/// What a patch sets: absent keeps, a string replaces, null or `""` removes.
#[derive(Debug, PartialEq)]
pub enum Change {
    Keep,
    Set(String),
    Clear,
}

/// `patchSchema.safeParse`: the two changes, or zod's messages joined with `; `.
pub fn parse_patch(body: &Value) -> Result<(Change, Change), String> {
    let Some(o) = body.as_object() else {
        return Err(invalid_type("object", Some(body)));
    };
    let mut issues = Vec::new();
    let mut field = |name: &str, max: Option<usize>| match o.get(name) {
        None => Change::Keep,
        Some(Value::Null) => Change::Clear,
        Some(Value::String(s)) => {
            let t = trim(s);
            if max.is_some_and(|m| utf16_len(t) > m) {
                issues.push(format!(
                    "Too big: expected string to have <={} characters",
                    max.unwrap_or(0)
                ));
                Change::Keep
            } else if t.is_empty() {
                Change::Clear
            } else {
                Change::Set(t.to_string())
            }
        }
        other => {
            issues.push(invalid_type("string", other));
            Change::Keep
        }
    };
    let key = field("apiKey", None);
    let model = field("model", Some(MODEL_MAX));
    let strangers: Vec<String> = o
        .keys()
        .filter(|k| *k != "apiKey" && *k != "model")
        .map(|k| format!("\"{k}\""))
        .collect();
    match strangers.len() {
        0 => {}
        1 => issues.push(format!("Unrecognized key: {}", strangers[0])),
        _ => issues.push(format!("Unrecognized keys: {}", strangers.join(", "))),
    }
    if issues.is_empty() {
        Ok((key, model))
    } else {
        Err(issues.join("; "))
    }
}

/// The account's stored settings: None for none, Err for a file that is there and cannot be read.
pub(crate) fn read_for_update(state: &AppState) -> Result<Option<Map<String, Value>>, ()> {
    let path = state
        .settings
        .prefs_dir
        .join("ai")
        .join(file_for(LOCAL_SUB));
    match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(o)) => Ok(Some(o)),
            Ok(_) => Ok(None),
            Err(_) => Err(()),
        },
    }
}

/// The signed-in account, or the 401 `requireAuth` answers.
fn account(state: &AppState, req: &Request) -> Result<(), Box<Response>> {
    if holds(req.headers(), &state.settings) {
        Ok(())
    } else {
        Err(Box::new(error(StatusCode::UNAUTHORIZED, "unauthenticated")))
    }
}

/// GET `/settings`: the view of what is stored.
pub async fn get_settings(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if let Err(r) = account(&state, &req) {
        return *r;
    }
    let stored = read_for_update(&state).ok().flatten();
    axum::Json(view_of(stored.as_ref())).into_response()
}

/// PUT `/settings`: the patch merged onto what is stored, never onto a file that could not be read.
pub async fn put_settings(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let session = holds(req.headers(), &state.settings);
    // Parsed before the session is checked, ahead of every guard.
    let body = match read_json_within(req, AI_BODY_LIMIT).await {
        Ok((_, v)) => v,
        Err(r) => return r,
    };
    if !session {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let (key, model) = match parse_patch(&body) {
        Ok(p) => p,
        Err(issues) => return error(StatusCode::BAD_REQUEST, &issues),
    };
    let Ok(current) = read_for_update(&state) else {
        let message = "the AI settings could not be read, so nothing was changed";
        return error(StatusCode::INTERNAL_SERVER_ERROR, message);
    };
    let mut next = current.unwrap_or_default();
    for (name, change) in [("apiKey", key), ("model", model)] {
        match change {
            Change::Keep => {}
            Change::Clear => {
                next.shift_remove(name);
            }
            Change::Set(v) => {
                next.insert(name.to_string(), Value::String(v));
            }
        }
    }
    let dir = state.settings.prefs_dir.join("ai");
    if write_atomic(
        &dir,
        &file_for(LOCAL_SUB),
        &pretty(&Value::Object(next.clone())),
    )
    .is_err()
    {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the AI settings could not be stored",
        );
    }
    axum::Json(view_of(Some(&next))).into_response()
}

/// GET `/organize`: whether the button should exist, and the model a click would use.
pub async fn get_organize(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if let Err(r) = account(&state, &req) {
        return *r;
    }
    let stored = read_for_update(&state).ok().flatten();
    let keyed = stored
        .as_ref()
        .and_then(|s| s.get("apiKey"))
        .is_some_and(truthy);
    let model = stored
        .as_ref()
        .and_then(|s| s.get("model"))
        .filter(|m| truthy(m))
        .cloned()
        .unwrap_or_else(|| Value::String(DEFAULT_AI_MODEL.into()));
    let body = json!({ "enabled": keyed, "model": if keyed { model } else { Value::Null } });
    axum::Json(body).into_response()
}
