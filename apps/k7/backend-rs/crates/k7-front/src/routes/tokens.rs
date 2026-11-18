//! tokens.rs — the library's personal access tokens: minted, listed and
//! revoked by their owner behind the SESSION only, in the file and format the TypeScript backend checks them in.
//! What a reader gets wrong: a bearer never reaches these routes, `claim` leaves it to the backend.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hyper::StatusCode;
use hyper::header::{CACHE_CONTROL, HeaderValue};
use k7_auth::api_token::{API_TOKEN_TTL_MS, TokenFile, mint};
use serde_json::{Map, Value, json};

use super::auth::ctx::{Ctx, now_ms};
use crate::AppState;
use crate::web::{error, read_json};

/// `library-tokens.json` under the auth folder, shared with the TypeScript backend.
pub fn token_file(auth_dir: &std::path::Path) -> TokenFile {
    TokenFile::new(auth_dir.join("library-tokens.json"))
}

/// What the owner sees of a token: never the token itself.
fn public_record(r: &Map<String, Value>) -> Value {
    json!({
        "id": r.get("id"),
        "label": r.get("label"),
        "created": r.get("created"),
        "expires": r.get("expires"),
        "lastUsed": r.get("lastUsed").cloned().unwrap_or(Value::Null),
    })
}

/// A label as typed, at most 80 UTF-16 units; a pair cut in two loses its half.
fn label_of(raw: Option<&Value>, fallback: &str) -> String {
    match raw.and_then(Value::as_str).map(crate::js::trim) {
        Some(t) if !t.is_empty() => {
            let mut units = 0;
            t.chars()
                .take_while(|c| {
                    units += c.len_utf16();
                    units <= 80
                })
                .collect()
        }
        _ => fallback.to_string(),
    }
}

/// The session check, which the backend runs first; a bearer never reaches here (`claim`).
fn gate(ctx: &Ctx, state: &AppState) -> Result<Map<String, Value>, Box<Response>> {
    ctx.user(state)
        .ok_or_else(|| Box::new(error(StatusCode::UNAUTHORIZED, "unauthenticated")))
}

/// `subjectOf(req)`: the `sub`, else the email, refused when empty.
fn subject(user: &Map<String, Value>) -> Result<String, Box<Response>> {
    match user
        .get("sub")
        .filter(|s| !s.is_null())
        .or_else(|| user.get("email").filter(|e| !e.is_null()))
    {
        Some(Value::String(id)) if !id.is_empty() => Ok(id.clone()),
        _ => Err(Box::new(error(
            StatusCode::UNAUTHORIZED,
            "no user on the session",
        ))),
    }
}

/// `gate`, then `subject`.
fn owner(ctx: &Ctx, state: &AppState) -> Result<String, Box<Response>> {
    subject(&gate(ctx, state)?)
}

/// A new token for `sub`, listed in the shared file before it is handed out.
fn issue(
    state: &AppState,
    sub: &str,
    label: &str,
) -> Result<(String, Map<String, Value>), Box<Response>> {
    let mut jti = [0u8; 16];
    getrandom::fill(&mut jti).expect("the OS gives random bytes");
    let now = now_ms();
    let (token, record) = mint(
        &state.settings.api_token_secret,
        sub,
        label,
        &URL_SAFE_NO_PAD.encode(jti),
        now,
        API_TOKEN_TTL_MS,
    );
    if let Err(e) = state.tokens.add(record.clone(), now) {
        eprintln!("library tokens: {e}");
        return Err(Box::new(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        )));
    }
    Ok((token, record))
}

/// `POST /api/token`: mint one, `{label}` in the body; the cross-site write refusal keeps other sites out.
pub async fn create(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let (headers, body) = match read_json(req).await {
        Ok(read) => read,
        Err(refused) => return refused,
    };
    let ctx = Ctx::open_rewritten(&headers, &state);
    let id = match owner(&ctx, &state) {
        Ok(id) => id,
        Err(refused) => return ctx.finish(*refused, &state),
    };
    let label = label_of(body.get("label"), "API token");
    let response = match issue(&state, &id, &label) {
        Ok((token, record)) => {
            let mut out = json!({ "token": token });
            if let (Value::Object(o), Value::Object(r)) = (&mut out, public_record(&record)) {
                o.extend(r);
            }
            Json(out).into_response()
        }
        Err(refused) => *refused,
    };
    ctx.finish(response, &state)
}

/// `GET /api/token`: the owner's tokens, without the tokens themselves.
pub async fn list(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let headers = req.headers().clone();
    let ctx = Ctx::open_rewritten(&headers, &state);
    let id = match owner(&ctx, &state) {
        Ok(id) => id,
        Err(refused) => return ctx.finish(*refused, &state),
    };
    let tokens: Vec<Value> = state
        .tokens
        .list(&id, now_ms())
        .iter()
        .map(public_record)
        .collect();
    let mut response = Json(json!({ "tokens": tokens })).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    ctx.finish(response, &state)
}

/// `DELETE /api/token/:id`: revoke one of the owner's.
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    req: Request,
) -> Response {
    let headers = req.headers().clone();
    let ctx = Ctx::open_rewritten(&headers, &state);
    let id = match owner(&ctx, &state) {
        Ok(id) => id,
        Err(refused) => return ctx.finish(*refused, &state),
    };
    let response = match state.tokens.revoke(&id, &token, now_ms()) {
        Ok(true) => {
            // The médiathèque's cache may still hold it; the backend's own check reads the file at once.
            if let Some(m) = &state.seams {
                m.forget_bearers();
            }
            Json(json!({ "ok": true })).into_response()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "no such token"),
        Err(e) => {
            eprintln!("library tokens: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    };
    ctx.finish(response, &state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_trimmed_and_capped_as_javascript_does() {
        assert_eq!(label_of(Some(&json!("  cli \u{feff}")), "API token"), "cli");
        assert_eq!(label_of(Some(&json!("   ")), "API token"), "API token");
        assert_eq!(label_of(Some(&json!(5)), "API token"), "API token");
        assert_eq!(label_of(None, "API token"), "API token");
        assert_eq!(label_of(Some(&json!("x".repeat(100))), "").len(), 80);
        let emoji = format!("{}{}", "x".repeat(79), "🎛");
        assert_eq!(label_of(Some(&json!(emoji)), ""), "x".repeat(79));
    }
}
