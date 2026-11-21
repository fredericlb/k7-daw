//! door.rs — the door this device opens on the LAN while its switch is on: `hello`, the screens' presence and the
//! served project's files, for a trusted device on a private network. Every request carries `X-K7-Engine`, this
//! device's engine token, which only a peer the engine trusted could learn (`pairing.md`).
//! What a reader gets wrong: this router has its own guard, because the loopback guard cannot apply to a LAN peer.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post, put};
use hyper::{Method, StatusCode};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use super::confine::{Served, refusal};
use super::{API, Lan, now_ms};
use crate::AppState;
use crate::routes::files::service;
use crate::session::LOCAL_EMAIL;
use crate::web::{collect_within, error, read_json};

/// What the served project looks like to a peer: its name and the surfaces it declares.
#[derive(Clone, Debug)]
pub struct ServedInfo {
    pub served: Served,
    pub label: String,
    pub app: String,
    pub surfaces: Value,
}

/// A served project is asked again after this long, so a poll every two seconds costs one read.
const SERVED_FRESH: Duration = Duration::from_secs(1);
/// At most this many screens are kept: a peer holding the token still does not choose the front's memory.
const MAX_SCREENS: usize = 64;

/// Private or loopback, as the engine reads a trusted peer (`isPrivateAddress`).
pub fn is_private_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_private_peer(IpAddr::V4(v4)),
            None => {
                let first = v6.segments()[0];
                v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
            }
        },
    }
}

fn lan_of(state: &AppState) -> &Arc<Lan> {
    state
        .lan
        .as_ref()
        .expect("the door runs only where the family is on")
}

/// The router the door serves: its own guard, no session, no CORS.
pub fn router(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/lan/hello", get(hello))
        .route("/lan/screens/{screen}", put(beat))
        .route("/lan/screens/{screen}/bye", post(bye))
        .route("/lan/files/{*rest}", any(files))
        .route_layer(from_fn_with_state(state.clone(), gate))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .with_state(state)
}

/// Private peers with this device's engine token only, and only while the switch is on.
async fn gate(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let lan = lan_of(&state);
    if !lan.is_open(&state) {
        return error(
            StatusCode::FORBIDDEN,
            "this device is not on the local network",
        );
    }
    if !is_private_peer(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "private networks only");
    }
    let token = state
        .engine
        .as_ref()
        .and_then(|e| e.core.registry.local_entry())
        .map(|e| e.token);
    let given = req
        .headers()
        .get("x-k7-engine")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match token {
        Some(t) if !t.is_empty() && bool::from(t.as_bytes().ct_eq(given.as_bytes())) => {
            next.run(req).await
        }
        _ => error(
            StatusCode::UNAUTHORIZED,
            "this device's engine token is required",
        ),
    }
}

/// A GET answered by this device's own file service, as the one local account; None when it does not answer or no hub runs.
async fn files_json(state: &AppState, target: &str) -> Option<Value> {
    let files = state.files.as_ref()?;
    let req = Request::builder()
        .method(Method::GET)
        .uri(target)
        .body(Body::empty())
        .ok()?;
    let reply = service::answer(files, &files.hub.names, LOCAL_EMAIL, target, req)
        .await
        .ok()?;
    if !reply.status().is_success() {
        return None;
    }
    let bytes = collect_within(reply.into_body(), 4 << 20).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

impl Lan {
    /// The project the engine holds with its declared surfaces, None when none or when it is the bench.
    pub(crate) async fn served(&self, state: &AppState) -> Option<ServedInfo> {
        {
            let kept = self.served.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((at, info)) = kept.as_ref()
                && at.elapsed() < SERVED_FRESH
            {
                return info.clone();
            }
        }
        let info = read_served(state).await;
        *self.served.lock().unwrap_or_else(|p| p.into_inner()) =
            Some((Instant::now(), info.clone()));
        info
    }
}

async fn read_served(state: &AppState) -> Option<ServedInfo> {
    let held = files_json(state, "/engine/served").await?;
    let rows = files_json(state, "/workspaces/surfaces").await?;
    crossing(&held, &rows)
}

/// `/engine/served` crossed with `/workspaces/surfaces` (`{ projects: [...] }`): the held project's row, None for the bench.
fn crossing(held: &Value, rows: &Value) -> Option<ServedInfo> {
    let workspace = held.pointer("/project/workspace")?.as_str()?.to_string();
    let path = held.pointer("/project/path")?.as_str()?.to_string();
    let row = rows.get("projects")?.as_array()?.iter().find(|r| {
        r["workspace"].as_str() == Some(&workspace) && r["path"].as_str() == Some(&path)
    })?;
    let label = if path == "." {
        workspace.clone()
    } else {
        format!("{workspace} / {path}")
    };
    Some(ServedInfo {
        served: Served { workspace, path },
        label,
        app: row["app"].as_str().unwrap_or("k7").to_string(),
        surfaces: row["surfaces"].clone(),
    })
}

async fn hello(State(state): State<Arc<AppState>>) -> Response {
    let lan = lan_of(&state);
    let Some(entry) = state
        .engine
        .as_ref()
        .and_then(|e| e.core.registry.local_entry())
    else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no engine is registered here",
        );
    };
    let served = lan.served(&state).await;
    let project = served.as_ref().map_or(Value::Null, |s| {
        json!({
            "workspace": s.served.workspace, "path": s.served.path,
            "label": s.label, "app": s.app,
        })
    });
    let key = served.as_ref().map(|s| s.served.key());
    Json(json!({
        "device": {
            "name": entry.hostname, "engineId": entry.id,
            "build": entry.build, "api": API, "proto": entry.protocol.unwrap_or(0.0) as u32,
        },
        "project": project,
        "surfaces": served.map_or(json!([]), |s| s.surfaces),
        "screens": lan.screens.list(key.as_deref(), now_ms()),
    }))
    .into_response()
}

/// `PUT /lan/screens/:screen {surface, device}`: a remote says it shows `surface`.
async fn beat(
    State(state): State<Arc<AppState>>,
    Path(screen): Path<String>,
    req: Request,
) -> Response {
    let lan = lan_of(&state);
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let text = |k: &str| {
        body.get(k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 128)
    };
    let (Some(surface), Some(device)) = (text("surface"), text("device")) else {
        return error(StatusCode::BAD_REQUEST, "surface and device are required");
    };
    if screen.len() > 64
        || !screen
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return error(
            StatusCode::BAD_REQUEST,
            "a screen id is letters, digits, - and _",
        );
    }
    let Some(served) = lan.served(&state).await else {
        return error(StatusCode::CONFLICT, "no project is open on this device");
    };
    if lan.screens.count() >= MAX_SCREENS {
        return error(StatusCode::TOO_MANY_REQUESTS, "too many screens");
    }
    lan.screens
        .beat(&screen, surface, device, &served.served.key(), now_ms());
    Json(json!({ "ok": true })).into_response()
}

async fn bye(State(state): State<Arc<AppState>>, Path(screen): Path<String>) -> Response {
    lan_of(&state).screens.bye(&screen);
    Json(json!({ "ok": true })).into_response()
}

/// `GET /api/lan/screens` on this device's own page: the screens other devices show, for the panes it must not open.
pub async fn screens_here(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if !crate::session::holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let lan = lan_of(&state);
    let served = lan.served(&state).await;
    let key = served.map(|s| s.served.key());
    Json(json!({ "screens": lan.screens.list(key.as_deref(), now_ms()) })).into_response()
}

/// `/lan/files/*`: the file service of the served project's workspace, minus what opens or configures (`confine.rs`).
async fn files(
    State(state): State<Arc<AppState>>,
    Path(rest): Path<String>,
    req: Request,
) -> Response {
    let lan = lan_of(&state);
    let target = match req.uri().query() {
        Some(q) => format!("/{rest}?{q}"),
        None => format!("/{rest}"),
    };
    let route = format!("/{rest}");
    let served = lan.served(&state).await.map(|s| s.served);
    let workspace = req
        .headers()
        .get("x-studio-workspace")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if let Some(why) = refusal(req.method(), &route, workspace.as_deref(), served.as_ref()) {
        return error(StatusCode::FORBIDDEN, why);
    }
    let Some(files) = state.files.as_ref() else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "no file service here");
    };
    match service::answer(files, &files.hub.names, LOCAL_EMAIL, &target, req).await {
        Ok(r) => r,
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "file service unavailable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_held_project_is_crossed_with_its_row_of_the_surfaces_listing() {
        let held = json!({ "project": { "workspace": "band", "path": "mix.k7" } });
        let rows = json!({ "projects": [
            { "workspace": "other", "path": "x.k7", "app": "k7", "surfaces": [] },
            { "workspace": "band", "path": "mix.k7", "app": "k7", "surfaces": [{ "id": "desk", "type": "turntables" }] },
        ] });
        let info = crossing(&held, &rows).expect("the row is found");
        assert_eq!(info.label, "band / mix.k7");
        assert_eq!(info.surfaces[0]["id"], "desk");
        // Nothing held, a project the listing lacks (the bench) and a bare array all list nothing.
        assert!(crossing(&json!({ "project": null }), &rows).is_none());
        let bench = json!({ "project": { "workspace": "Banc", "path": "live.k7" } });
        assert!(crossing(&bench, &rows).is_none());
        assert!(crossing(&held, &rows["projects"]).is_none());
    }

    #[test]
    fn private_and_loopback_peers_only() {
        for ip in [
            "10.1.2.3",
            "172.16.0.9",
            "192.168.1.5",
            "169.254.3.3",
            "127.0.0.1",
            "::1",
            "fd00::1",
            "fe80::2",
            "::ffff:192.168.0.2",
        ] {
            assert!(is_private_peer(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "172.32.0.1", "2001:db8::1", "::ffff:8.8.8.8"] {
            assert!(!is_private_peer(ip.parse().unwrap()), "{ip}");
        }
    }
}
