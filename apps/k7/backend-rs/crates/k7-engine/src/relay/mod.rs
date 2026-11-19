//! relay — a browser's socket relayed to an engine: through the engine's tunnel first,
//! else dialled at the address its heartbeat came FROM; what the browser sends before the dial is held, every
//! frame crosses the gate, both ends are pinged, and either end closing closes the other (`docs/engine/proxy.md`).

pub mod gate;

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::json;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use crate::Core;
use crate::frame::Frame;
use crate::registry::{Entry, hosts_for, on_this_machine};
use crate::tunnel::backpressure::{Admit, HeldFrames, OVERLOAD_CLOSE, admit};
use crate::tunnel::channel::{ChannelEvent, ChannelTx};
use crate::tunnel::frames::Kind;
use crate::ws::{self, Event, Peer};
use gate::{CLAIM_GRACE_MS, Gate, POLL_MS, Sinks};

/// The whole budget for reaching an engine, and one address's share of it.
pub const ENGINE_REACH_BUDGET_MS: u64 = 3000;
pub const ENGINE_DIAL_TIMEOUT_MS: u64 = 1500;
/// How often both ends are pinged; one that missed the previous ping is cut.
pub const HEARTBEAT_MS: u64 = 30_000;

/// What the relay sends to: a socket it dialled, or a channel through the engine's tunnel.
#[derive(Clone)]
enum Up {
    Ws(Peer),
    Channel(ChannelTx),
}

impl Up {
    fn is_open(&self) -> bool {
        match self {
            Up::Ws(p) => p.is_open(),
            Up::Channel(c) => c.is_open(),
        }
    }
    fn buffered(&self) -> usize {
        match self {
            Up::Ws(p) => p.buffered(),
            Up::Channel(c) => c.buffered(),
        }
    }
    fn send(&self, f: &Frame) {
        match self {
            Up::Ws(p) => p.send(f),
            Up::Channel(c) => c.send(f),
        }
    }
    /// A peer that stopped reading: a browser is told why, a channel just ends.
    fn overloaded(&self) {
        match self {
            Up::Ws(p) => p.close(OVERLOAD_CLOSE, "peer too slow"),
            Up::Channel(c) => c.close(),
        }
    }
    fn shut(&self) {
        match self {
            Up::Ws(p) => p.shut(),
            Up::Channel(c) => c.close(),
        }
    }
}

/// One frame, bounded; on overload `to` is told and `from` shut.
fn forward(to: &Up, from: &Up, f: &Frame) {
    if !to.is_open() {
        return;
    }
    match admit(to.buffered(), f) {
        Admit::Send => to.send(f),
        Admit::Drop => {}
        Admit::Close => {
            to.overloaded();
            from.shut();
        }
    }
}

struct Relay {
    core: Arc<Core>,
    id: String,
    holder: String,
    client: Up,
    upstream: Option<Up>,
    pending: HeldFrames,
}

impl Sinks for Relay {
    fn holds(&self) -> bool {
        self.core.registry.holds(&self.id, &self.holder)
    }
    // Asked per frame: an engine that stops running here stops the token at once.
    fn may_read_token(&self) -> bool {
        self.core.registry.may_use(&self.id, &self.holder)
    }
    fn to_engine(&mut self, f: Frame) {
        match &self.upstream {
            Some(up) => forward(up, &self.client, &f),
            None => {
                if !self.pending.push(f) {
                    self.client.overloaded();
                }
            }
        }
    }
    fn close(&mut self, code: u16, reason: &str) {
        if let Up::Ws(c) = &self.client {
            c.close(code, reason);
        }
        if let Some(up) = &self.upstream {
            up.shut();
        }
    }
}

/// What a dial reached: the socket's sending half and its events, or a channel.
enum Reached {
    Ws(Peer, mpsc::Receiver<Event>),
    Channel(crate::tunnel::channel::Channel),
}

/// The engine's tunnel first, then every address in turn.
async fn dial(core: Arc<Core>, e: Entry) -> Option<Reached> {
    let budget = Duration::from_millis(ENGINE_REACH_BUDGET_MS);
    if core.hub.has(&e.id)
        && let Some(ch) = core
            .hub
            .open_ready(&e.id, Vec::new(), budget, Kind::Engine)
            .await
    {
        return Some(Reached::Channel(ch));
    }
    dial_direct(&e, core.clock.now_ms()).await
}

/// The first address that answers, sequentially, within ONE budget for the whole list.
async fn dial_direct(e: &Entry, now: i64) -> Option<Reached> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ENGINE_REACH_BUDGET_MS);
    for host in hosts_for(e.public_ip_hint.as_deref(), on_this_machine(e, now)) {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        let url = format!("ws://{host}:{}", e.port as u64);
        let attempt = async {
            let (mut ws, _) = tokio_tungstenite::connect_async_with_config(
                url.as_str(),
                Some(ws::config()),
                true,
            )
            .await
            .ok()?;
            // The token is the server's proof over the LAN; the engine's reply is consumed here, never piped.
            let pair = json!({ "type": "pair", "key": e.token }).to_string();
            futures_util::SinkExt::send(&mut ws, Message::Text(pair.into()))
                .await
                .ok()?;
            loop {
                match ws.next().await? {
                    Ok(Message::Text(_) | Message::Binary(_)) => return Some(ws),
                    Ok(_) => {}
                    Err(_) => return None,
                }
            }
        };
        let timeout = left.min(Duration::from_millis(ENGINE_DIAL_TIMEOUT_MS));
        if let Ok(Some(ws)) = tokio::time::timeout(timeout, attempt).await {
            let (peer, events) = ws::spawn(ws);
            return Some(Reached::Ws(peer, events));
        }
    }
    None
}

/// Serves a browser's relay socket, for the account its ticket names, until either end lets go.
pub async fn serve<S>(
    core: Arc<Core>,
    id: String,
    holder: String,
    engine: Entry,
    ws: WebSocketStream<S>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (client_peer, mut from_client) = ws::spawn(ws);
    let client = Up::Ws(client_peer.clone());
    let relay = Relay {
        core: core.clone(),
        id,
        holder,
        client: client.clone(),
        upstream: None,
        pending: HeldFrames::default(),
    };
    let mut gate = Gate::new(relay, CLAIM_GRACE_MS, core.clock.clone());
    let mut dialing = Box::pin(dial(core.clone(), engine));
    let mut dialed = false;
    let mut up_ws: Option<mpsc::Receiver<Event>> = None;
    let mut up_ch: Option<mpsc::UnboundedReceiver<ChannelEvent>> = None;
    let mut poll = tokio::time::interval(Duration::from_millis(POLL_MS));
    let mut ping = tokio::time::interval(Duration::from_millis(HEARTBEAT_MS));
    ping.tick().await;
    let (mut client_alive, mut up_alive) = (true, true);
    loop {
        tokio::select! {
            ev = from_client.recv() => match ev {
                Some(Event::Frame(f)) => gate.from_client(f),
                Some(Event::Pong) => client_alive = true,
                Some(Event::Closed(_)) | None => break,
            },
            reached = &mut dialing, if !dialed => {
                dialed = true;
                let up = match reached {
                    None => {
                        client_peer.close(1011, "engine unreachable from the server");
                        continue;
                    }
                    Some(Reached::Ws(p, events)) => { up_ws = Some(events); Up::Ws(p) }
                    Some(Reached::Channel(c)) => { up_ch = Some(c.rx); Up::Channel(c.tx) }
                };
                if !client_peer.is_open() {
                    up.shut();
                    break;
                }
                let s = gate.sinks();
                for f in s.pending.drain() {
                    forward(&up, &client, &f);
                }
                s.upstream = Some(up);
            },
            ev = recv_ws(&mut up_ws) => match ev {
                Some(Event::Frame(f)) => relay_back(&mut gate, &client, f),
                Some(Event::Pong) => up_alive = true,
                Some(Event::Closed(_)) | None => { client_peer.shut(); up_ws = None; }
            },
            ev = recv_ch(&mut up_ch) => match ev {
                Some(ChannelEvent::Frame(f)) => relay_back(&mut gate, &client, f),
                Some(ChannelEvent::Open) => {}
                Some(ChannelEvent::Close) | None => { client_peer.shut(); up_ch = None; }
            },
            _ = poll.tick(), if gate.waiting() => gate.tick(),
            _ = ping.tick() => {
                // A phone gone without a FIN would otherwise hold the pair and its session for hours.
                if !std::mem::replace(&mut client_alive, false) { client_peer.terminate(); }
                else { client_peer.ping(); }
                if let Some(Up::Ws(p)) = &gate.sinks().upstream {
                    if !std::mem::replace(&mut up_alive, false) { p.terminate(); } else { p.ping(); }
                }
            }
        }
    }
    gate.dispose();
    if let Some(up) = &gate.sinks().upstream {
        up.shut();
    }
    // A dial still under way is finished and let go, so no channel stays open on the machine.
    if !dialed {
        tokio::spawn(async move {
            match dialing.await {
                Some(Reached::Ws(p, _)) => p.shut(),
                Some(Reached::Channel(c)) => c.tx.close(),
                None => {}
            }
        });
    }
}

fn relay_back(gate: &mut Gate<Relay>, client: &Up, f: Frame) {
    if let Some(out) = gate.from_engine(f)
        && let Some(up) = gate.sinks().upstream.clone()
    {
        forward(client, &up, &out);
    }
}

async fn recv_ws(r: &mut Option<mpsc::Receiver<Event>>) -> Option<Event> {
    match r {
        Some(r) => r.recv().await,
        None => std::future::pending().await,
    }
}

async fn recv_ch(r: &mut Option<mpsc::UnboundedReceiver<ChannelEvent>>) -> Option<ChannelEvent> {
    match r {
        Some(r) => r.recv().await,
        None => std::future::pending().await,
    }
}
