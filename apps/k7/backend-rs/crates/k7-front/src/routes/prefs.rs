//! prefs.rs — `/api/preferences`, as `@studio/user-prefs` and the backend's schema answer it: one JSON
//! file per subject, written whole, refused when it would be stored short. The file's bytes are what the
//! backend writes, since its other routes read the same file (`packages/user-prefs/docs/store.md`).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::atomic::write_atomic;
use crate::session::{Signed, https, signed_in, with_cookies};
use crate::web::{error, read_json};

/// The filename for a subject: a readable slug AND a hash, as `fileFor` in `packages/user-prefs/store.ts`.
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
    let Some(signed) = signed_in(
        req.headers(),
        &state.settings,
        &state.revocations,
        https(req.headers(), &state.settings),
    ) else {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    };
    get_as(&state, &signed)
}

/// GET for a session already decided, the device door's among them.
pub(crate) fn get_as(state: &AppState, signed: &Signed) -> Response {
    let Some(sub) = signed.user.get("sub").and_then(Value::as_str) else {
        return with_cookies(
            error(StatusCode::UNAUTHORIZED, "no user on the session"),
            signed,
        );
    };
    let path = state.settings.prefs_dir.join(file_for(sub));
    let stored = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or_else(|| Value::Object(Map::new()));
    with_cookies(axum::Json(stored).into_response(), signed)
}

/// PUT: the whole document, validated against the backend's schema, then written beside and renamed.
pub async fn put(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let signed = signed_in(
        req.headers(),
        &state.settings,
        &state.revocations,
        https(req.headers(), &state.settings),
    );
    // Parsed before the session is checked, as the backend's shared parser runs ahead of every guard.
    let sent = match read_json(req).await {
        Ok((_, v)) => v,
        Err(r) => return r,
    };
    let Some(signed) = signed else {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    };
    put_as(&state, &signed, &sent)
}

/// PUT for a session already decided, of a body already read.
pub(crate) fn put_as(state: &AppState, signed: &Signed, sent: &Value) -> Response {
    let Some(sub) = signed
        .user
        .get("sub")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return with_cookies(
            error(StatusCode::UNAUTHORIZED, "no user on the session"),
            signed,
        );
    };
    let kept = match crate::prefs_schema::parse(sent) {
        Ok(k) => k,
        Err(issues) => return with_cookies(error(StatusCode::BAD_REQUEST, &issues), signed),
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
        return with_cookies(error(StatusCode::BAD_REQUEST, &message), signed);
    }
    if write_atomic(
        &state.settings.prefs_dir,
        &file_for(&sub),
        &pretty(&Value::Object(kept)),
    )
    .is_err()
    {
        let message =
            "the preference was applied but could not be stored — it will not outlive this session";
        return with_cookies(error(StatusCode::INTERNAL_SERVER_ERROR, message), signed);
    }
    with_cookies(
        axum::Json(serde_json::json!({ "ok": true })).into_response(),
        signed,
    )
}

/// `JSON.stringify(value, null, 2)` and a newline.
pub fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(&crate::prefs_schema::js_numbers(value))
        .unwrap_or_else(|_| "{}".into());
    text.push('\n');
    text
}
