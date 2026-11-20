//! registry.rs — the registry's routes: the engines' open heartbeat, deregister and
//! hello; the browser's list (each usable entry with a relay ticket), select, release and WebRTC offer, behind
//! the session, with the statuses, bodies and log lines of `apps/k7-daw-backend/docs/engine/heartbeat.md`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use k7_engine::js::{invalid_type, number_issues, string_issues};
use k7_engine::registry::heartbeat::read_heartbeat;
use k7_engine::registry::{ClaimRequest, ClaimResult, HeartbeatInput, Refused};
use serde_json::{Map, Value, json};

use super::{client_ip, engine_of, holder, with_session};
use crate::AppState;
use crate::web::{error, read_json};

/// Who a refusal is about, as the log names it.
fn who(id: &str, hostname: &str, ip: &str) -> String {
    let host = if hostname.is_empty() {
        String::new()
    } else {
        format!(" ({hostname})")
    };
    let from = if ip.is_empty() {
        String::new()
    } else {
        format!(" from {ip}")
    };
    format!("engine {id}{host}{from}")
}

/// Each way a heartbeat is refused, as its status and the sentence both logs carry.
fn refusal(why: Refused) -> (StatusCode, &'static str) {
    match why {
        Refused::Secret => (
            StatusCode::FORBIDDEN,
            "this id is bound to another machine's tunnel secret — a copied engine-id.json, or somebody else's id",
        ),
        Refused::Token => (
            StatusCode::CONFLICT,
            "id/token mismatch — this id is registered with another token; it is freed 15 s after that one's last heartbeat",
        ),
        Refused::Full => (
            StatusCode::SERVICE_UNAVAILABLE,
            "the registry lists as many engines as it takes",
        ),
        Refused::Rate => (
            StatusCode::TOO_MANY_REQUESTS,
            "too many new engines from this address in the last minute",
        ),
    }
}

/// `validate`'s answer: every issue's message, joined, and no path.
pub(super) fn invalid(issues: Vec<String>) -> Response {
    error(StatusCode::BAD_REQUEST, &issues.join("; "))
}

/// A body's field checked by `check`, its issues collected.
pub(super) fn field<T>(issues: &mut Vec<String>, r: Result<T, Vec<String>>) -> Option<T> {
    r.map_err(|ms| issues.extend(ms)).ok()
}

/// `z.string().min(1)`.
pub(super) fn required(v: Option<&Value>) -> Result<String, Vec<String>> {
    string_issues(v, 1, usize::MAX, None)
}

/// `.optional()` around `check`: absent passes.
pub(super) fn optional<T>(
    v: Option<&Value>,
    check: impl Fn(Option<&Value>) -> Result<T, Vec<String>>,
) -> Result<Option<T>, Vec<String>> {
    match v {
        None => Ok(None),
        some => check(some).map(Some),
    }
}

pub async fn hello() -> Response {
    Json(json!({ "service": "k7-registry" })).into_response()
}

pub async fn heartbeat(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    let engine = engine_of(&state);
    let ip = client_ip(&req, peer, &state.settings);
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let raw_id = body.get("id").and_then(Value::as_str).unwrap_or("?");
    let raw_host = body.get("hostname").and_then(Value::as_str).unwrap_or("");
    let b = match read_heartbeat(&body) {
        Ok(b) => b,
        Err(e) => {
            engine.complain.say(
                &format!("{raw_id}|refused"),
                &format!(
                    "refused a heartbeat from {}: {e}",
                    who(raw_id, raw_host, &ip)
                ),
            );
            return error(StatusCode::BAD_REQUEST, &format!("heartbeat refused — {e}"));
        }
    };
    let id = b.id.clone();
    let host = b.hostname.clone();
    let cleaned = b.cleaned.clone();
    if !cleaned.is_empty() {
        engine.complain.say(
            &format!("{id}|cleaned|{}", cleaned.join("|")),
            &format!(
                "took a heartbeat from {} with fields cleaned: {}",
                who(&id, &host, &ip),
                cleaned.join("; ")
            ),
        );
    }
    let core = &engine.core;
    if let Err(why) = core.registry.upsert(HeartbeatInput {
        beat: b,
        public_ip_hint: Some(ip.clone()),
        // The client as read, which is loopback only from a loopback socket: through the dev Vite a LAN engine is its LAN address.
        from_loopback: k7_engine::registry::is_loopback_address(Some(&ip)),
    }) {
        let (status, sentence) = refusal(why);
        let tag = match why {
            Refused::Secret => "secret",
            Refused::Token => "token",
            Refused::Full => "full",
            Refused::Rate => "rate",
        };
        engine.complain.say(
            &format!("{id}|{tag}"),
            &format!(
                "refused a heartbeat from {}: {sentence}",
                who(&id, &host, &ip)
            ),
        );
        return error(status, sentence);
    }
    let mut out = Map::new();
    out.insert("ok".into(), json!(true));
    if !cleaned.is_empty() {
        out.insert("cleaned".into(), json!(cleaned));
    }
    Json(Value::Object(out)).into_response()
}

pub async fn deregister(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let mut issues = Vec::new();
    let id = field(&mut issues, required(body.get("id")));
    let token = field(&mut issues, required(body.get("token")));
    let secret = field(
        &mut issues,
        optional(body.get("tunnelSecret"), |v| string_issues(v, 0, 256, None)),
    );
    let (Some(id), Some(token), Some(secret)) = (id, token, secret) else {
        return invalid(issues);
    };
    if !engine_of(&state)
        .core
        .registry
        .remove(&id, &token, secret.as_deref())
    {
        return error(StatusCode::FORBIDDEN, "id/token mismatch");
    }
    Json(json!({ "ok": true })).into_response()
}

pub async fn engines(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    let ip = client_ip(&req, peer, &state.settings);
    with_session(&state, &req, || list_for(&state, &ip))
}

/// The engines `s`'s account may see from `ip`, each usable one with a relay ticket.
pub(super) fn list_for(state: &AppState, ip: &str) -> Response {
    let viewer = holder();
    let core = &engine_of(state).core;
    let now = core.clock.now_ms();
    let list: Vec<Value> = core
        .registry
        .list(Some(ip), &viewer)
        .into_iter()
        .map(|e| {
            let mut o = e.json;
            if core.registry.may_use(&e.id, &viewer) {
                o.insert("access".into(), json!("owner"));
                o.insert(
                    "proxyTicket".into(),
                    json!(core.tickets.issue(&e.id, &viewer, now)),
                );
                if core.hub.has(&e.id) {
                    o.insert("rtc".into(), json!(true));
                }
            } else {
                // An engine off this machine is listed for its LAN hint alone, to the network it is on.
                if !e.same_source_ip
                    && let Some(Value::Object(a)) = o.get_mut("addresses")
                {
                    a.insert("lanIps".into(), json!([]));
                }
                o.insert("access".into(), json!("unowned"));
            }
            Value::Object(o)
        })
        .collect();
    Json(Value::Array(list)).into_response()
}

pub async fn select(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if !crate::session::holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    select_inner(&state, req).await
}

/// A claim for the device's account, whose identity never comes from the body.
async fn select_inner(state: &AppState, req: Request) -> Response {
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let mut issues = Vec::new();
    let id = field(&mut issues, required(body.get("id")));
    let token = field(&mut issues, required(body.get("token")));
    let project = field(
        &mut issues,
        match body.get("project") {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(p)) => Ok(Some(p.clone())),
            got => Err(vec![invalid_type("string", got)]),
        },
    );
    let secret = field(
        &mut issues,
        optional(body.get("secret"), |v| {
            string_issues(v, 0, usize::MAX, None)
        }),
    );
    field(
        &mut issues,
        optional(body.get("latencyMs"), |v| {
            number_issues(v, false, f64::MIN, f64::MAX)
        }),
    );
    let (Some(id), Some(token), Some(project), Some(secret), true) =
        (id, token, project, secret, issues.is_empty())
    else {
        return invalid(issues);
    };
    select_for(state, id, token, project, secret)
}

fn select_for(
    state: &AppState,
    id: String,
    token: String,
    project: Option<String>,
    secret: Option<String>,
) -> Response {
    let engine = engine_of(state);
    let reg = &engine.core.registry;
    let say = |key: String, line: String| {
        engine.complain.say(
            &key,
            &format!("select of {} refused: {line}", who(&id, "", "")),
        )
    };
    if reg.get(&id).is_none() {
        say(
            format!("{id}|select-gone"),
            "no heartbeat from it in the last 15 s".into(),
        );
        return error(
            StatusCode::NOT_FOUND,
            "engine gone — its machine has not sent a heartbeat in the last 15 s; its .studio.log says why",
        );
    }
    if !reg.verify_token(&id, &token) {
        say(
            format!("{id}|select-token"),
            "the browser's token is not the registered one".into(),
        );
        return error(
            StatusCode::FORBIDDEN,
            "bad token — this engine registered with another token; reload the engine list",
        );
    }
    // WHO comes from the session, never from the body.
    let holder = holder();
    if !reg.may_use(&id, &holder) {
        say(
            format!("{id}|select-unowned"),
            "it does not run on this machine".into(),
        );
        let body = json!({ "error": "unowned", "message": "this engine does not run on this device, and a device uses its own engine only" });
        return (StatusCode::FORBIDDEN, Json(body)).into_response();
    }
    match reg.claim(
        &id,
        ClaimRequest {
            project,
            holder,
            secret,
        },
    ) {
        ClaimResult::Lost {
            lost,
            held_by,
            took_for,
        } => {
            let mut o = Map::new();
            o.insert("error".into(), json!("project taken"));
            o.insert("heldBy".into(), json!(held_by));
            o.insert("lost".into(), json!(lost.as_str()));
            if let Some(t) = took_for {
                o.insert("tookFor".into(), json!(t));
            }
            (StatusCode::CONFLICT, Json(Value::Object(o))).into_response()
        }
        ClaimResult::Ok { secret, took_over } => {
            let mut o = Map::new();
            o.insert("ok".into(), json!(true));
            o.insert("secret".into(), json!(secret));
            if let Some((holder, project)) = took_over {
                o.insert(
                    "tookOver".into(),
                    json!({ "holder": holder, "project": project }),
                );
            }
            Json(Value::Object(o)).into_response()
        }
    }
}

pub async fn release(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if !crate::session::holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    release_inner(&state, req).await
}

/// A claim let go, the body's secret naming it.
async fn release_inner(state: &AppState, req: Request) -> Response {
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let mut issues = Vec::new();
    let id = field(&mut issues, required(body.get("id")));
    let secret = field(&mut issues, required(body.get("secret")));
    match (id, secret) {
        (Some(id), Some(secret)) => {
            // 200 either way: this arrives by `sendBeacon` from a page already going.
            engine_of(state).core.registry.release(&id, &secret);
            Json(json!({ "ok": true })).into_response()
        }
        _ => invalid(issues),
    }
}

/// An SDP offer is a few kilobytes; the bound keeps an open route from buffering more.
const MAX_SDP: usize = 64 * 1024;

pub async fn rtc(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    if !crate::session::holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let sdp = match string_issues(body.get("sdp"), 1, MAX_SDP, None) {
        Ok(s) => s,
        Err(issues) => return invalid(issues),
    };
    let core = &engine_of(&state).core;
    // A data channel reaches the engine as its own machine would, so only a claim holder gets one.
    if !core.registry.holds(&id, &holder()) {
        error(
            StatusCode::FORBIDDEN,
            "claim this engine before opening a direct link",
        )
    } else {
        match core.hub.signal(&id, &sdp, Duration::from_secs(10)).await {
            Ok(sdp) => Json(json!({ "sdp": sdp })).into_response(),
            // 503 and not 500: the engine is simply not reachable this way, and the relay still is.
            Err(e) => error(StatusCode::SERVICE_UNAVAILABLE, &e),
        }
    }
}
