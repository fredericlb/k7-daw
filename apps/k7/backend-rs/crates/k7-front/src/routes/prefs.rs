//! prefs.rs — `/api/preferences`: one JSON file per subject, written whole, refused when it would be stored
//! short (a key the schema drops would lose the person's data).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::atomic::write_atomic;
use crate::session::{LOCAL_SUB, holds};
use crate::web::{error, read_json};

/// The filename for a subject: a readable slug AND a hash.
pub fn file_for(sub: &str) -> String {
    let slug: String = sub
        .encode_utf16()
        .map(|u| match char::from_u32(u32::from(u)) {
            Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-' => c,
            _ => '_',
        })
        .take(40)
        .collect();
    let hash = Sha256::digest(sub.as_bytes());
    let hex: String = hash.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("{slug}-{hex}.json")
}

/// GET: the stored document, `{}` for a subject who never saved.
pub async fn get(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if !holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let path = state.settings.prefs_dir.join(file_for(LOCAL_SUB));
    let stored = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or_else(|| Value::Object(Map::new()));
    axum::Json(stored).into_response()
}

/// PUT: the whole document, validated against the backend's schema, then written beside and renamed.
pub async fn put(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let signed = holds(req.headers(), &state.settings);
    // Parsed before the session is checked, as the backend's shared parser runs ahead of every guard.
    let sent = match read_json(req).await {
        Ok((_, v)) => v,
        Err(r) => return r,
    };
    if !signed {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let kept = match crate::prefs_schema::parse(&sent) {
        Ok(k) => k,
        Err(issues) => return error(StatusCode::BAD_REQUEST, &issues),
    };
    let dropped: Vec<&str> = sent
        .as_object()
        .map(|o| {
            o.keys()
                .filter(|k| !kept.contains_key(*k))
                .map(String::as_str)
                .collect()
        })
        .unwrap_or_default();
    if !dropped.is_empty() {
        let message = format!(
            "this service does not store {} — declare it in its preferences schema",
            dropped.join(", ")
        );
        return error(StatusCode::BAD_REQUEST, &message);
    }
    if write_atomic(
        &state.settings.prefs_dir,
        &file_for(LOCAL_SUB),
        &pretty(&Value::Object(kept)),
    )
    .is_err()
    {
        let message =
            "the preference was applied but could not be stored — it will not outlive this session";
        return error(StatusCode::INTERNAL_SERVER_ERROR, message);
    }
    axum::Json(serde_json::json!({ "ok": true })).into_response()
}

/// `JSON.stringify(value, null, 2)` and a newline.
pub fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(&crate::prefs_schema::js_numbers(value))
        .unwrap_or_else(|_| "{}".into());
    text.push('\n');
    text
}
