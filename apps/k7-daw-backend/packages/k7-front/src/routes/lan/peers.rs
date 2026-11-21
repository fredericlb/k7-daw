//! peers.rs — the devices this one can see: discovery's list, each asked who it is over its engine socket.
//! That `whoami` is also how this device learns the peer's engine token, which only a trusted peer is given, and
//! which the relay then shows at the peer's door; the page never holds it. What a reader gets wrong: a peer that
//! does not answer is listed, as unreached, since the page says why a device is not usable.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use hyper::StatusCode;
use serde::Serialize;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use super::Lan;
use crate::AppState;
use crate::session::holds;
use crate::web::error;

/// How long a device has to answer `whoami`.
const ASK_WITHIN: Duration = Duration::from_secs(3);
/// A resolved list is reused this long, whoever asks.
const FRESH: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub engine_id: String,
    pub name: String,
    pub host: String,
    pub door: u16,
    pub engine: u16,
    pub api: u32,
    pub proto: u32,
    /// The engine answered `whoami` within the wait.
    pub reached: bool,
    /// It gave this device its token, so the door answers it.
    pub trusted: bool,
    #[serde(skip)]
    pub token: String,
}

#[derive(Default)]
pub struct Resolved {
    at: Option<Instant>,
    views: Vec<PeerView>,
}

struct Who {
    id: String,
    token: String,
}

/// `host:port`, a v6 host bracketed.
pub fn authority(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// The engine's `whoami`: its id and, when it trusts this device, its token.
async fn whoami(host: &str, port: u16) -> Option<Who> {
    let url = format!("ws://{}/", authority(host, port));
    let ask = async {
        // No Origin is sent: a native client, which the engine lets in (`pairing.md`).
        let (mut ws, _) = tokio_tungstenite::connect_async(url).await.ok()?;
        ws.send(Message::text(r#"{"type":"whoami"}"#)).await.ok()?;
        while let Some(Ok(m)) = ws.next().await {
            let Message::Text(t) = m else { continue };
            let v: Value = serde_json::from_str(t.as_str()).ok()?;
            if v["type"] == "whoami" {
                let token = if v["needsPairing"] == true {
                    String::new()
                } else {
                    v["token"].as_str().unwrap_or("").to_string()
                };
                return Some(Who {
                    id: v["id"].as_str()?.to_string(),
                    token,
                });
            }
        }
        None
    };
    tokio::time::timeout(ASK_WITHIN, ask).await.ok().flatten()
}

impl Lan {
    /// Every peer heard, resolved, this device's own left out; kept two seconds.
    pub(crate) async fn peers(&self, state: &AppState) -> Vec<PeerView> {
        let mut kept = self.resolved.lock().await;
        if kept.at.is_some_and(|t| t.elapsed() < FRESH) {
            return kept.views.clone();
        }
        let own = state
            .engine
            .as_ref()
            .and_then(|e| e.core.registry.local_entry())
            .map(|e| e.id);
        let heard = self.discovery().peers();
        let asked = futures_util::future::join_all(heard.into_iter().map(|p| async move {
            let who = whoami(&p.host, p.engine).await;
            (p, who)
        }))
        .await;
        let mut views: Vec<PeerView> = Vec::new();
        for (p, who) in asked {
            let id = match &who {
                Some(w) => w.id.clone(),
                None if !p.engine_id.is_empty() => p.engine_id.clone(),
                None => format!("{}-{}", p.host, p.door),
            };
            if own.as_deref() == Some(id.as_str()) || views.iter().any(|v| v.engine_id == id) {
                continue;
            }
            let (reached, token) = who.map_or((false, String::new()), |w| (true, w.token));
            views.push(PeerView {
                engine_id: id,
                name: p.name,
                host: p.host,
                door: p.door,
                engine: p.engine,
                api: p.api,
                proto: p.proto,
                reached,
                trusted: !token.is_empty(),
                token,
            });
        }
        views.sort_by(|a, b| (&a.name, &a.engine_id).cmp(&(&b.name, &b.engine_id)));
        kept.views = views.clone();
        kept.at = Some(Instant::now());
        views
    }

    /// One peer by engine id; a list kept from before it appeared is asked again once.
    pub(crate) async fn peer(&self, state: &AppState, id: &str) -> Option<PeerView> {
        if let Some(v) = self
            .peers(state)
            .await
            .into_iter()
            .find(|v| v.engine_id == id)
        {
            return Some(v);
        }
        self.resolved.lock().await.at = None;
        self.peers(state)
            .await
            .into_iter()
            .find(|v| v.engine_id == id)
    }
}

fn lan(state: &AppState) -> &Arc<Lan> {
    state
        .lan
        .as_ref()
        .expect("claimed only where the family is on")
}

/// `GET /lan/peers`: `{ peers }`.
pub async fn list(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if !holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let peers = lan(&state).peers(&state).await;
    Json(json!({ "peers": peers })).into_response()
}

/// `GET /lan/self`: this device's name and engine id, null while it has no engine.
pub async fn this_device(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if !holds(req.headers(), &state.settings) {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let entry = state
        .engine
        .as_ref()
        .and_then(|e| e.core.registry.local_entry());
    Json(json!({
        "name": entry.as_ref().map(|e| e.hostname.clone()),
        "engineId": entry.map(|e| e.id),
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_v6_host_is_bracketed() {
        assert_eq!(authority("10.0.0.2", 80), "10.0.0.2:80");
        assert_eq!(authority("fe80::1", 80), "[fe80::1]:80");
        assert_eq!(authority("[fe80::1]", 80), "[fe80::1]:80");
    }
}
