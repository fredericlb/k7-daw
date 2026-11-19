//! routes.rs — the machine's engine for k7's own pages (`/api/local/engine`: the snapshot, pushed, and the commands a
//! page may send) and for a page probing its host (`/api/local`).
//! A device that runs no engine here (Android: the app hosts the supervisor) still answers the probe.

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use hyper::StatusCode;
use hyper::body::Bytes;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::Local;
use crate::AppState;
use crate::session::{Signed, signed, with_cookies};
use crate::streams::{ChannelBody, closed};
use crate::web::{error, read_json, read_json_within};

/// What a page may ask; `weburi.add` hands the engine to an instance, so the page confirms it first.
const PAGE_COMMANDS: [&str; 14] = [
    "engine.start",
    "engine.stop",
    "engine.restart",
    "audio.priority",
    "lan.open",
    "plugins.rescan",
    "instruments.download",
    "instruments.remove",
    "weburi.add",
    "weburi.remove",
    "slot.enable",
    "slot.disable",
    "slot.remove",
    "slot.footswitch",
];

/// A compiled slot's limit: a chain is far bigger than any other JSON body a page sends.
const SLOT_LIMIT: usize = 16 * 1024 * 1024;
/// A comment line now and then keeps proxies and the browser from dropping a quiet stream.
const HEARTBEAT: Duration = Duration::from_secs(25);

fn local(state: &AppState) -> &Arc<Local> {
    state
        .local
        .as_ref()
        .expect("claim routes /api/local/engine* here only when the front runs the engine")
}

/// The engine this front runs, None on a device whose engine is hosted elsewhere.
fn running(state: &AppState) -> Option<&Arc<Local>> {
    state.local.as_ref().filter(|l| l.owned())
}

/// `requireAuth`'s session; None answers its 401 (`unauthorized`).
fn session(state: &AppState, req: &Request) -> Option<Signed> {
    signed(req.headers(), &state.settings, &state.revocations)
}

fn unauthorized() -> Response {
    error(StatusCode::UNAUTHORIZED, "unauthenticated")
}

/// `GET /api/local`: answered by a machine's own backend only, which is how a page knows it is on one.
pub async fn info(State(state): State<Arc<AppState>>) -> Response {
    let body = match running(&state) {
        Some(l) => json!({ "engine": true, "version": l.version }),
        None => json!({ "engine": false, "version": state.settings.release }),
    };
    axum::Json(body).into_response()
}

/// `GET /api/local/engine`: `{snapshot, down}`.
pub async fn get(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let Some(signed) = session(&state, &req) else {
        return unauthorized();
    };
    let body = local(&state).engine.state().to_json();
    with_cookies(axum::Json(body).into_response(), &signed)
}

fn frame(state: &k7_local::State) -> Bytes {
    Bytes::from(format!("data: {}\n\n", state.to_json()))
}

/// `GET /api/local/engine/events`: the state now, then each change, as Server-Sent Events.
pub async fn events(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let Some(signed) = session(&state, &req) else {
        return unauthorized();
    };
    let mut changes = local(&state).engine.subscribe();
    let mut closing = closed(state.closing.subscribe());
    let (tx, rx) = mpsc::channel::<Bytes>(16);
    tokio::spawn(async move {
        let first = frame(&changes.borrow_and_update());
        if tx.send(Bytes::from_static(b"retry: 2000\n")).await.is_err()
            || tx.send(first).await.is_err()
        {
            return;
        }
        let mut beat = tokio::time::interval_at(tokio::time::Instant::now() + HEARTBEAT, HEARTBEAT);
        loop {
            let next = tokio::select! {
                changed = changes.changed() => match changed {
                    Ok(()) => frame(&changes.borrow_and_update()),
                    Err(_) => return,
                },
                _ = beat.tick() => Bytes::from_static(b": hb\n\n"),
                _ = tx.closed() => return,
                _ = &mut closing => return,
            };
            if tx.send(next).await.is_err() {
                return;
            }
        }
    });
    let mut response = Response::new(Body::new(ChannelBody(rx)));
    let h = response.headers_mut();
    h.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    h.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    with_cookies(response, &signed)
}

/// `POST /api/local/engine/cmd`: one allowed command, handed to the supervisor.
pub async fn cmd(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let Some(signed) = session(&state, &req) else {
        return unauthorized();
    };
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return with_cookies(r, &signed),
    };
    let cmd = body.get("cmd").and_then(Value::as_str);
    let arg = body.get("arg");
    let answer = match cmd {
        Some(c) if PAGE_COMMANDS.contains(&c) => match arg {
            None => {
                local(&state).engine.send(c, None);
                axum::Json(json!({ "ok": true })).into_response()
            }
            Some(Value::String(a)) => {
                local(&state).engine.send(c, Some(a));
                axum::Json(json!({ "ok": true })).into_response()
            }
            Some(_) => error(StatusCode::BAD_REQUEST, "arg must be a string"),
        },
        _ => error(StatusCode::BAD_REQUEST, "command not allowed"),
    };
    with_cookies(answer, &signed)
}

/// A random version-4 UUID, as `randomUUID` spells one.
fn uuid() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("no OS randomness");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The slot staged where the supervisor reads it, readable by this user only.
fn stage(body: &Value) -> std::io::Result<std::path::PathBuf> {
    let path = std::env::temp_dir().join(format!("k7-slot-{}.json", uuid()));
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    let mut f = open.open(&path)?;
    f.write_all(body.to_string().as_bytes())?;
    Ok(path)
}

/// `POST /api/local/engine/slots`: a compiled slot staged in a file and handed over by path, as `slot.save`.
pub async fn slots(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let Some(signed) = session(&state, &req) else {
        return unauthorized();
    };
    let body = match read_json_within(req, SLOT_LIMIT).await {
        Ok((_, b)) => b,
        Err(r) => return with_cookies(r, &signed),
    };
    let id = body.get("id").and_then(Value::as_str).map(str::to_string);
    // `typeof doc === "object"`, which null and an array pass too.
    let doc = matches!(
        body.get("doc"),
        Some(Value::Object(_) | Value::Array(_) | Value::Null)
    );
    let answer = match id {
        Some(id) if body.is_object() && doc => match stage(&body) {
            Ok(path) => {
                local(&state)
                    .engine
                    .send("slot.save", Some(&path.display().to_string()));
                axum::Json(json!({ "ok": true, "id": id })).into_response()
            }
            Err(e) => error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("could not stage the slot: {e}"),
            ),
        },
        _ => error(StatusCode::BAD_REQUEST, "a slot is {id, …, doc}"),
    };
    with_cookies(answer, &signed)
}
