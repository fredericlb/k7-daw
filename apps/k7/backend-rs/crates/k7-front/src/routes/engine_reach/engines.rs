//! engines.rs — how the engine jobs reach an engine: the registry's view of it and a
//! channel through the hub, from `k7-engine` in-process when the front holds the engine core, over the internal
//! seam's engine half at the backend otherwise (`apps/k7/backend-rs/docs/engine/reach.md`).
//! What a reader gets wrong: an in-process channel drops what is sent before the engine answers, so a `Link` holds it.

use std::sync::Arc;
use std::time::Duration;

use hyper::Method;
use k7_engine::frame::Frame;
use k7_engine::tunnel::backpressure::{Admit, HeldFrames, admit};
use k7_engine::tunnel::channel::{Channel, ChannelEvent};
use k7_engine::tunnel::frames::{Kind, Root};
use k7_engine::tunnel::spelling::{SEP, tunnel_spelling};
use k7_engine::ws::{self, Event};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::seam::Seam;
use crate::AppState;

/// How long a question to the backend's engine seam may take.
const ASK: Duration = Duration::from_secs(10);

/// One engine as an account sees it (`EngineView`).
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: String,
    pub role: String,
    pub holds: bool,
    pub tunnel: bool,
    pub renderer: bool,
    pub score: Option<f64>,
    /// The chains the ENGINE's machine separates with, by `want` — absent from an older supervisor.
    pub chains: Option<k7_engine::registry::heartbeat::Chains>,
}

/// What a link's owner reads.
#[derive(Debug)]
pub enum LinkEvent {
    Frame(Frame),
    Closed,
}

enum Out {
    Frame(Frame),
    Close,
}

/// The sending half of a link, cheap to clone.
#[derive(Clone)]
pub struct LinkTx(mpsc::UnboundedSender<Out>);

impl LinkTx {
    pub fn send(&self, f: Frame) {
        let _ = self.0.send(Out::Frame(f));
    }

    pub fn text(&self, s: String) {
        self.send(Frame::text(s));
    }

    /// Ends the channel; what was sent before it goes first.
    pub fn close(&self) {
        let _ = self.0.send(Out::Close);
    }
}

/// A channel to one engine, whichever process holds the hub.
pub struct Link {
    pub tx: LinkTx,
    pub rx: mpsc::UnboundedReceiver<LinkEvent>,
}

impl Link {
    /// A link whose far end is `out` and `events`, for a pump task and for tests.
    fn pair() -> (
        Link,
        mpsc::UnboundedReceiver<Out>,
        mpsc::UnboundedSender<LinkEvent>,
    ) {
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        (
            Link {
                tx: LinkTx(out_tx),
                rx: ev_rx,
            },
            out_rx,
            ev_tx,
        )
    }
}

/// Where the engines are reached: the core this front holds, or the backend's seam.
pub enum Reach<'a> {
    Core(&'a Arc<k7_engine::Core>),
    Seam(&'a Seam),
}

/// The reach of a request's front: the core when it holds one, the backend's seam otherwise.
pub fn reach(state: &AppState) -> Reach<'_> {
    match &state.engine {
        Some(e) => Reach::Core(&e.core),
        None => Reach::Seam(&crate::routes::engine_reach::seams_of(state).engine_seam),
    }
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

fn view_of(v: &Value) -> Option<View> {
    let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    let chains = v.get("chains").and_then(Value::as_object).map(|c| {
        let one = |k: &str| c.get(k).and_then(Value::as_str).map(str::to_string);
        k7_engine::registry::heartbeat::Chains {
            stems: one("stems"),
            instrumental: one("instrumental"),
        }
    });
    Some(View {
        id: v.get("id")?.as_str()?.to_string(),
        role: v
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("none")
            .to_string(),
        holds: flag("holds"),
        tunnel: flag("tunnel"),
        renderer: flag("renderer"),
        score: v.get("score").and_then(Value::as_f64),
        chains,
    })
}

fn core_view(core: &k7_engine::Core, id: &str, score: Option<f64>, holder: &str) -> View {
    View {
        id: id.to_string(),
        role: core.registry.role(id, holder, false).as_str().to_string(),
        holds: core.registry.holds(id, holder),
        tunnel: core.hub.has(id),
        renderer: core.hub.is_renderer(id),
        score,
        // The machine's own chains travel on its heartbeat (stemSplit.md).
        chains: core.registry.get(id).and_then(|e| e.chains),
    }
}

impl Reach<'_> {
    /// Engine `id` as `holder` sees it, None when no live engine has that id.
    pub async fn view(&self, id: &str, holder: &str) -> Result<Option<View>, String> {
        match self {
            Reach::Core(core) => Ok(core.registry.get(id).map(|e| {
                let score = e
                    .performance
                    .as_ref()
                    .and_then(|p| p.get("score"))
                    .and_then(Value::as_f64);
                core_view(core, &e.id, score, holder)
            })),
            Reach::Seam(seam) => {
                let path = format!(
                    "/api/internal/engine/engines/{}?holder={}",
                    enc(id),
                    enc(holder)
                );
                match seam.ask_at(Method::GET, &path, None, ASK).await {
                    Ok((404, _)) => Ok(None),
                    Ok((200, v)) => Ok(view_of(&v)),
                    _ => Err("the engine seam did not answer for an engine".into()),
                }
            }
        }
    }

    /// A channel to engine `id` (None: the render machine) sharing `roots`; None when it holds no tunnel here.
    pub async fn open(
        &self,
        id: Option<&str>,
        roots: Vec<Root>,
        kind: Kind,
        timeout: Duration,
    ) -> Result<Option<Link>, String> {
        match self {
            Reach::Core(core) => {
                let Some(id) = id.map(str::to_string).or_else(|| core.hub.renderer()) else {
                    return Ok(None);
                };
                let Some(ch) = core.hub.open(&id, roots.clone(), kind, false) else {
                    return Ok(None);
                };
                let (link, out, events) = Link::pair();
                tokio::spawn(pump_channel(ch, roots, kind == Kind::Jobs, out, events));
                Ok(Some(link))
            }
            Reach::Seam(seam) => open_at_seam(seam, id, roots, kind, timeout).await,
        }
    }
}

/// `EngineSeam.open`: a one-shot binding, its upgrade, then the binding dropped; the hub reads it at the upgrade only.
async fn open_at_seam(
    seam: &Seam,
    id: Option<&str>,
    roots: Vec<Root>,
    kind: Kind,
    timeout: Duration,
) -> Result<Option<Link>, String> {
    let body = json!({ "engineId": id, "roots": roots, "kind": kind.as_str() });
    let path = match seam
        .ask_at(
            Method::POST,
            "/api/internal/engine/bindings",
            Some(body),
            ASK,
        )
        .await
    {
        Ok((201, v)) => v
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => return Err("the engine seam refused a binding".into()),
    };
    let url = format!("ws://{}{path}", seam.upstream());
    let dialled = tokio::time::timeout(
        timeout,
        tokio_tungstenite::connect_async_with_config(url.as_str(), Some(ws::config()), true),
    )
    .await;
    let key = path.rsplit('/').next().unwrap_or("").to_string();
    let _ = seam
        .ask_at(
            Method::DELETE,
            &format!("/api/internal/engine/bindings/{key}"),
            None,
            ASK,
        )
        .await;
    let Ok(Ok((socket, _))) = dialled else {
        return Ok(None);
    };
    let (link, out, events) = Link::pair();
    tokio::spawn(pump_socket(socket, out, events));
    Ok(Some(link))
}

/// An in-process channel, held until the engine answers and respelt for a jobs channel, as the hub's loopback pipe does.
async fn pump_channel(
    mut ch: Channel,
    roots: Vec<Root>,
    jobs: bool,
    mut out: mpsc::UnboundedReceiver<Out>,
    events: mpsc::UnboundedSender<LinkEvent>,
) {
    let tx = ch.tx.clone();
    let mut held = HeldFrames::default();
    let forward = |f: &Frame| match admit(tx.buffered(), f) {
        Admit::Send => {
            tx.send(f);
            true
        }
        Admit::Drop => true,
        Admit::Close => false,
    };
    loop {
        tokio::select! {
            o = out.recv() => match o {
                Some(Out::Frame(f)) => {
                    let f = if jobs && !f.binary {
                        Frame::text(tunnel_spelling(&String::from_utf8_lossy(&f.data), &roots, SEP))
                    } else {
                        f
                    };
                    let kept = if tx.is_open() { forward(&f) } else { held.push(f) };
                    if !kept {
                        tx.close();
                        let _ = events.send(LinkEvent::Closed);
                        return;
                    }
                }
                Some(Out::Close) | None => {
                    tx.close();
                    return;
                }
            },
            e = ch.rx.recv() => match e {
                Some(ChannelEvent::Open) => {
                    if !held.drain().iter().all(&forward) {
                        tx.close();
                        let _ = events.send(LinkEvent::Closed);
                        return;
                    }
                }
                Some(ChannelEvent::Frame(f)) => {
                    let _ = events.send(LinkEvent::Frame(f));
                }
                Some(ChannelEvent::Close) | None => {
                    let _ = events.send(LinkEvent::Closed);
                    return;
                }
            },
        }
    }
}

/// A socket to a bound loopback path at the backend.
async fn pump_socket<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
    mut out: mpsc::UnboundedReceiver<Out>,
    events: mpsc::UnboundedSender<LinkEvent>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (peer, mut from) = ws::spawn(socket);
    loop {
        tokio::select! {
            o = out.recv() => match o {
                Some(Out::Frame(f)) => peer.send(&f),
                Some(Out::Close) | None => {
                    peer.shut();
                    return;
                }
            },
            e = from.recv() => match e {
                Some(Event::Frame(f)) => {
                    let _ = events.send(LinkEvent::Frame(f));
                }
                Some(Event::Pong) => {}
                Some(Event::Closed(_)) | None => {
                    let _ = events.send(LinkEvent::Closed);
                    return;
                }
            },
        }
    }
}
