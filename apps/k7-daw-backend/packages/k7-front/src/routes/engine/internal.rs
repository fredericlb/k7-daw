//! internal.rs — the engine half of the internal seam (`apps/k7/backend-rs/docs/engine/reach.md`), which the
//! hub's own services call over loopback: views of the registry, the render machine, and loopback bindings.
//! Gated by a loopback peer AND the shared secret; outside the web chain.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use hyper::header::{CACHE_CONTROL, HeaderValue};
use hyper::{Method, StatusCode};
use k7_engine::registry::is_loopback_address;
use k7_engine::tunnel::frames::{Kind, Root};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use super::engine_of;
use crate::AppState;
use crate::web::{error, read_json};

const BASE: &str = "/api/internal/engine";

/// The seam's engine paths the core answers; `/local` is the local engine's, and stays the backend's here.
pub fn owns(method: &Method, path: &str) -> bool {
    let Some(rest) = path.strip_prefix(BASE) else {
        return false;
    };
    let get = *method == Method::GET || *method == Method::HEAD;
    match rest {
        "/engines" | "/hub" => get,
        "/bindings" => *method == Method::POST,
        _ if rest.starts_with("/engines/") => get && !rest[9..].contains('/'),
        _ if rest.starts_with("/bindings/") => {
            *method == Method::DELETE && !rest[10..].contains('/')
        }
        _ => false,
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    let p = |m: axum::routing::MethodRouter<Arc<AppState>>| m.fallback(crate::declined);
    Router::new()
        .route(&format!("{BASE}/engines"), p(get(engines)))
        .route(&format!("{BASE}/engines/{{id}}"), p(get(engine)))
        .route(&format!("{BASE}/hub"), p(get(hub)))
        .route(&format!("{BASE}/bindings"), p(post(bind)))
        .route(&format!("{BASE}/bindings/{{key}}"), p(delete(unbind)))
        .route_layer(axum::middleware::from_fn(no_store))
}

/// The gate: a loopback peer (403), then the secret in constant time (401).
pub async fn gate(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let peer = crate::peer::canonical(peer.ip());
    if !is_loopback_address(Some(&peer)) {
        return error(
            StatusCode::FORBIDDEN,
            "the internal API answers this machine only",
        );
    }
    let given = req
        .headers()
        .get("x-k7-internal")
        .map(|v| v.as_bytes())
        .unwrap_or_default();
    let secret = state.front_secret.as_bytes();
    if secret.is_empty() || !bool::from(given.ct_eq(secret)) {
        return error(
            StatusCode::UNAUTHORIZED,
            "the internal API needs this process's secret",
        );
    }
    next.run(req).await
}

/// A passed request is never cached.
async fn no_store(req: Request, next: Next) -> Response {
    let mut r = next.run(req).await;
    r.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// The gate, applied to the seam's router.
pub fn gated(state: Arc<AppState>, r: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    r.route_layer(from_fn_with_state(state, gate))
}

fn holder(req: &Request) -> String {
    let q = req.uri().query().unwrap_or("");
    url::form_urlencoded::parse(q.as_bytes())
        .find(|(k, _)| k == "holder")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}

/// One engine as the seam answers it: `EngineView`.
fn view(state: &AppState, id: &str, hostname: &str, score: Option<f64>, holder: &str) -> Value {
    let core = &engine_of(state).core;
    json!({
        "id": id,
        "hostname": hostname,
        "role": if core.registry.may_use(id, holder) { "owner" } else { "unowned" },
        "holds": core.registry.holds(id, holder),
        "tunnel": core.hub.has(id),
        "score": score.map(k7_engine::js::num),
    })
}

async fn engines(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let holder = holder(&req);
    let list = engine_of(&state).core.registry.list(None, &holder);
    let views: Vec<Value> = list
        .iter()
        .map(|e| view(&state, &e.id, &e.hostname, e.score, &holder))
        .collect();
    Json(json!({ "engines": views })).into_response()
}

async fn engine(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let Some(e) = engine_of(&state).core.registry.get(&id) else {
        return error(
            StatusCode::NOT_FOUND,
            &format!("no engine '{id}' is registered"),
        );
    };
    let score = e
        .performance
        .as_ref()
        .and_then(|p| p.get("score"))
        .and_then(Value::as_f64);
    Json(view(&state, &e.id, &e.hostname, score, &holder(&req))).into_response()
}

async fn hub(State(state): State<Arc<AppState>>) -> Response {
    let hub = &engine_of(&state).core.hub;
    Json(json!({
        "epoch": hub.epoch(),
    }))
    .into_response()
}

/// What `bindBody` reads: the engine, its roots, the kind, synced, and a key the caller held.
type Binding = (String, Vec<Root>, Kind, bool, Option<String>);

/// A key its caller held before a restart, never one it made up: as long as the drawn ones.
fn held_key(k: &str) -> bool {
    (32..=128).contains(&k.len())
        && k.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `bindBody`: the binding asked for, or the first issue.
fn read_binding(b: &Value) -> Result<Binding, String> {
    let engine_id = match b.get("engineId") {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(Value::String(_)) => {
            return Err("Too small: expected string to have >=1 characters".into());
        }
        got => return Err(k7_engine::js::invalid_type("string", got)),
    };
    let Some(Value::Array(raw)) = b.get("roots") else {
        return Err(k7_engine::js::invalid_type("array", b.get("roots")));
    };
    if raw.len() > 64 {
        return Err("Too big: expected array to have <=64 items".into());
    }
    let mut roots = Vec::new();
    for r in raw {
        let s = |k: &str| {
            r.get(k)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        let (Some(remote), Some(name), Some(writable)) = (
            s("remote"),
            s("name"),
            r.get("writable").and_then(Value::as_bool),
        ) else {
            return Err("Invalid input".into());
        };
        let per_child = match r.get("perChild") {
            None => None,
            Some(Value::Bool(p)) => Some(*p),
            Some(_) => return Err("Invalid input".into()),
        };
        roots.push(Root {
            remote,
            name,
            writable,
            per_child,
        });
    }
    let kind = match b.get("kind").and_then(Value::as_str) {
        Some("engine") => Kind::Engine,
        Some("jobs") => Kind::Jobs,
        _ => return Err("Invalid option: expected one of \"engine\"|\"jobs\"".into()),
    };
    let synced = match b.get("synced") {
        None => false,
        Some(Value::Bool(s)) => *s,
        Some(got) => return Err(k7_engine::js::invalid_type("boolean", Some(got))),
    };
    let key = match b.get("key") {
        None => None,
        Some(Value::String(k)) if held_key(k) => Some(k.clone()),
        Some(Value::String(_)) => return Err("Invalid string: not a held key".into()),
        Some(got) => return Err(k7_engine::js::invalid_type("string", Some(got))),
    };
    Ok((engine_id, roots, kind, synced, key))
}

async fn bind(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    match read_binding(&body) {
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
        Ok((engine_id, roots, kind, synced, key)) => {
            let hub = &engine_of(&state).core.hub;
            let path = match key {
                Some(key) => hub.bind_local_as(key, &engine_id, roots, kind, synced),
                None => hub.bind_local(&engine_id, roots, kind, synced),
            };
            let body = json!({ "path": path, "epoch": hub.epoch() });
            (StatusCode::CREATED, Json(body)).into_response()
        }
    }
}

async fn unbind(State(state): State<Arc<AppState>>, Path(key): Path<String>) -> Response {
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return error(StatusCode::BAD_REQUEST, "not a binding key");
    }
    let hub = &engine_of(&state).core.hub;
    hub.unbind_local(&format!(
        "{}/local/{key}",
        k7_engine::tunnel::frames::TUNNEL_PATH
    ));
    StatusCode::NO_CONTENT.into_response()
}
