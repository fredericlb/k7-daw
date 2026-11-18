//! ownership.rs — what an engine's OWNER does with it, and what an ADMIN may:
//! the shares read and replaced, the engine given up, the owners listed. Anyone else gets `404 not-yours`, the
//! same as for an engine nobody owns, so a stranger cannot probe which ids are owned (`docs/engine/ownership.md`).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use k7_engine::js::{invalid_type, string_issues};
use k7_engine::registry::owners::{MAX_SHARED, Role, clean_shares, role_of};
use serde_json::{Value, json};

use super::registry::{field, invalid, optional};
use super::{engine_of, holder_of, with_session};
use crate::AppState;
use crate::session::{is_admin, signed, with_cookies};
use crate::web::{error, read_json};

fn not_yours(status: StatusCode) -> Response {
    error(status, "not-yours")
}

/// Whether the session's account owns `id`, as `roleOf` with no server reads it.
fn owns(state: &AppState, id: &str, user: &serde_json::Map<String, Value>) -> bool {
    let owner = engine_of(state).core.registry.owner_of(id);
    role_of(owner.as_ref(), &holder_of(user), false) == Role::Owner
}

pub async fn owners(State(state): State<Arc<AppState>>, req: Request) -> Response {
    with_session(&state, &req, |s| {
        if !is_admin(&s.user, &state.settings) {
            return not_yours(StatusCode::FORBIDDEN);
        }
        let all: Vec<Value> = engine_of(&state)
            .core
            .registry
            .owners()
            .into_iter()
            .map(|(id, o, host)| {
                let mut m = serde_json::Map::new();
                m.insert("id".into(), json!(id));
                m.extend(o.json());
                m.insert("online".into(), json!(host.is_some()));
                m.insert("hostname".into(), json!(host));
                m.insert(
                    "online".into(),
                    json!(m.get("hostname").is_some_and(|h| !h.is_null())),
                );
                Value::Object(m)
            })
            .collect();
        Json(Value::Array(all)).into_response()
    })
}

pub async fn shares(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    with_session(&state, &req, |s| {
        let o = engine_of(&state).core.registry.owner_of(&id);
        match o {
            Some(o) if owns(&state, &id, &s.user) || is_admin(&s.user, &state.settings) => {
                Json(Value::Object(o.json())).into_response()
            }
            _ => not_yours(StatusCode::NOT_FOUND),
        }
    })
}

pub async fn put_shares(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let Some(signed) = signed(req.headers(), &state.settings, &state.revocations) else {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    };
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let mut issues = Vec::new();
    let emails = field(
        &mut issues,
        match body.get("emails") {
            Some(Value::Array(a)) => {
                let mut out = Vec::new();
                let mut bad = Vec::new();
                for v in a {
                    match string_issues(Some(v), 0, 254, None) {
                        Ok(s) => out.push(s),
                        Err(ms) => bad.extend(ms),
                    }
                }
                if a.len() > MAX_SHARED {
                    bad.push(format!(
                        "Too big: expected array to have <={MAX_SHARED} items"
                    ));
                }
                if bad.is_empty() { Ok(out) } else { Err(bad) }
            }
            got => Err(vec![invalid_type("array", got)]),
        },
    );
    let everyone = field(
        &mut issues,
        optional(body.get("everyone"), |v| match v {
            Some(Value::Bool(b)) => Ok(*b),
            got => Err(vec![invalid_type("boolean", got)]),
        }),
    );
    let (Some(emails), Some(everyone), true) = (emails, everyone, issues.is_empty()) else {
        return with_cookies(invalid(issues), &signed);
    };
    let answer = if !owns(&state, &id, &signed.user) {
        not_yours(StatusCode::NOT_FOUND)
    } else {
        match clean_shares(&emails) {
            Err(bad) => error(StatusCode::BAD_REQUEST, &format!("not an email: {bad}")),
            Ok(clean) => {
                let reg = &engine_of(&state).core.registry;
                reg.share(&id, &clean, everyone);
                // A withdrawn share ends that account's claim now, not when its tab next renews.
                reg.evict_unshared(&id);
                let o = reg.owner_of(&id);
                Json(json!({
                    "ok": true,
                    "shared": o.as_ref().map(|o| o.shared.clone()).unwrap_or_default(),
                    "everyone": o.is_some_and(|o| o.everyone),
                }))
                .into_response()
            }
        }
    };
    with_cookies(answer, &signed)
}

pub async fn give_up(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    with_session(&state, &req, |s| {
        let reg = &engine_of(&state).core.registry;
        if reg.owner_of(&id).is_none()
            || !(owns(&state, &id, &s.user) || is_admin(&s.user, &state.settings))
        {
            return not_yours(StatusCode::NOT_FOUND);
        }
        reg.release_owner(&id);
        Json(json!({ "ok": true })).into_response()
    })
}
