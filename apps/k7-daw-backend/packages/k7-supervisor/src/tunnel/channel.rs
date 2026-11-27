//! channel.rs — one virtual channel the hub opened: its two queues (a frame waiting on a download is never
//! overtaken by the next), its path map, and, on an engine channel, the real socket to the local engine. A
//! close is delivered AFTER the frames queued before it, within a bound (`apps/k7-daw-backend/docs/engine/tunnel.md`).
//! What a reader gets wrong: `shut` is the only way a channel ends, and it never waits for anything.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Message;

use super::agent::Shared;
use super::flag::raised;
use super::path_map::PathMap;
use super::queue::SerialQueue;
use super::shed::FrameShedder;
use super::wire::{CLOSE_DRAIN, HIGH_WATER_BYTES, encode_binary};

/// Dialing the engine, handshake included.
const ENGINE_DIAL: Duration = Duration::from_secs(10);

/// A frame's bytes, as text or binary.
pub enum Payload {
    Text(String),
    Bin(Vec<u8>),
}

impl Payload {
    fn len(&self) -> usize {
        match self {
            Payload::Text(t) => t.len(),
            Payload::Bin(b) => b.len(),
        }
    }
}

/// Bytes a queued frame holds, given back once its task ends however it ends.
struct Queued(Arc<AtomicUsize>, usize);

impl Queued {
    fn new(total: &Arc<AtomicUsize>, n: usize) -> Queued {
        total.fetch_add(n, Ordering::SeqCst);
        Queued(total.clone(), n)
    }
}

impl Drop for Queued {
    fn drop(&mut self) {
        self.0.fetch_sub(self.1, Ordering::SeqCst);
    }
}

pub struct Channel {
    pub ch: i64,
    pub jobs: bool,
    /// The server copies every file before naming it (`sync`), so frames to the engine are only rewritten.
    pub synced: bool,
    pub map: Option<Arc<PathMap>>,
    to_engine: SerialQueue,
    to_hub: SerialQueue,
    /// Read and written only by the `to_hub` queue's task.
    shed: Mutex<FrameShedder>,
    /// Bytes queued each way and not yet handed on: a close drains them only below the high water.
    to_engine_bytes: Arc<AtomicUsize>,
    to_hub_bytes: Arc<AtomicUsize>,
    open: watch::Sender<bool>,
    closed: watch::Sender<bool>,
    engine: mpsc::UnboundedSender<Message>,
}

impl Channel {
    pub fn new(
        ch: i64,
        jobs: bool,
        synced: bool,
        map: Option<Arc<PathMap>>,
    ) -> (Arc<Channel>, mpsc::UnboundedReceiver<Message>) {
        let (engine, rx) = mpsc::unbounded_channel();
        let c = Channel {
            ch,
            jobs,
            synced,
            map,
            to_engine: SerialQueue::new(),
            to_hub: SerialQueue::new(),
            shed: Mutex::default(),
            to_engine_bytes: Arc::default(),
            to_hub_bytes: Arc::default(),
            open: watch::channel(false).0,
            closed: watch::channel(false).0,
            engine,
        };
        (Arc::new(c), rx)
    }

    /// Block a queued frame until the engine socket is up, or the channel is gone.
    async fn wait_open(&self) -> bool {
        let (mut open, mut closed) = (self.open.subscribe(), self.closed.subscribe());
        tokio::select! {
            _ = raised(&mut open) => !*closed.borrow(),
            _ = raised(&mut closed) => false,
        }
    }

    pub fn is_shut(&self) -> bool {
        *self.closed.borrow()
    }

    /// Ends the channel: its queues drop what they hold, its engine socket closes. Never waits.
    pub fn shut(&self) {
        self.closed.send_replace(true);
        self.to_engine.close();
        self.to_hub.close();
    }
}

impl Shared {
    /// The hub's frame for the engine: queued behind the ones before it, after its files are here.
    pub fn to_engine(self: &Arc<Self>, ch: i64, data: Payload) {
        let Some(c) = self.channel(ch) else {
            return;
        };
        let held = Queued::new(&c.to_engine_bytes, data.len());
        let (sh, chan) = (self.clone(), c.clone());
        c.to_engine.push(async move {
            let _held = held;
            if let Err(e) = sh.deliver_to_engine(&chan, data).await {
                (sh.opts.log)(&format!("ch{} → engine: {e}", chan.ch));
            }
        });
    }

    async fn deliver_to_engine(
        self: &Arc<Self>,
        c: &Arc<Channel>,
        data: Payload,
    ) -> Result<(), String> {
        if !c.jobs && !c.wait_open().await {
            return Ok(());
        }
        let out = match (data, &c.map) {
            (Payload::Text(t), Some(map)) => {
                Payload::Text(map.outbound(&t, c.jobs || !c.synced).await?)
            }
            (data, _) => data,
        };
        if c.jobs {
            // Started once its files are here, and NOT awaited: a job takes minutes.
            if let Payload::Text(t) = out {
                self.run_job(c, &t);
            }
            return Ok(());
        }
        let msg = match out {
            Payload::Text(t) => Message::text(t),
            Payload::Bin(b) => Message::binary(b),
        };
        let _ = c.engine.send(msg);
        Ok(())
    }

    /// The engine's frame for the hub, queued behind the ones before it.
    pub fn to_hub(self: &Arc<Self>, c: &Arc<Channel>, data: Payload) {
        let held = Queued::new(&c.to_hub_bytes, data.len());
        let (sh, chan) = (self.clone(), c.clone());
        c.to_hub.push(async move {
            let _held = held;
            sh.deliver_to_hub(&chan, data).await;
        });
    }

    async fn deliver_to_hub(self: &Arc<Self>, c: &Arc<Channel>, data: Payload) {
        if !c.jobs {
            let (bytes, binary) = match &data {
                Payload::Text(t) => (t.as_bytes(), false),
                Payload::Bin(b) => (b.as_slice(), true),
            };
            // An engine channel's meters and audio wait for no one: behind a slow uplink they are dropped.
            if !c.shed.lock().unwrap_or_else(|e| e.into_inner()).admit(
                bytes,
                binary,
                self.link.buffered(),
            ) {
                return;
            }
        }
        let shed = c.shed.lock().unwrap_or_else(|e| e.into_inner()).take_shed();
        if shed > 0 {
            (self.opts.log)(&format!(
                "ch{} → server: dropped {shed} meter/audio frame(s) the uplink could not carry",
                c.ch
            ));
        }
        match data {
            Payload::Bin(b) => {
                self.link
                    .send_binary(encode_binary(&json!({"t": "data", "ch": c.ch}), &b));
            }
            Payload::Text(t) => {
                let text = match &c.map {
                    Some(map) => map.inbound(&t).await,
                    None => t,
                };
                self.tell(&json!({"t": "data", "ch": c.ch, "text": text}));
            }
        }
    }

    /// The hub let go: the frames queued for the engine are delivered first, within the bound.
    pub fn close_engine(self: &Arc<Self>, c: &Arc<Channel>) {
        c.to_hub.close();
        if c.jobs || c.to_engine_bytes.load(Ordering::SeqCst) > HIGH_WATER_BYTES {
            c.shut();
            return;
        }
        let chan = c.clone();
        c.to_engine.drain(CLOSE_DRAIN, move || chan.shut());
    }

    /// The engine let go: what it sent before is delivered to the hub first, within the bound.
    fn close_hub(self: &Arc<Self>, c: &Arc<Channel>, reason: Option<String>) {
        c.to_engine.close();
        let (sh, chan) = (self.clone(), c.clone());
        let tell_close = move || {
            chan.shut();
            let mut m = json!({"t": "close", "ch": chan.ch});
            if let Some(r) = reason.filter(|r| !r.is_empty()) {
                m["reason"] = r.into();
            }
            sh.tell(&m);
        };
        if c.to_hub_bytes.load(Ordering::SeqCst) > HIGH_WATER_BYTES {
            tell_close();
            return;
        }
        c.to_hub.drain(CLOSE_DRAIN, tell_close);
    }

    /// The socket to the local engine for one channel, until either side lets go.
    pub async fn engine_socket(
        self: Arc<Self>,
        c: Arc<Channel>,
        mut out: mpsc::UnboundedReceiver<Message>,
    ) {
        let port = (self.opts.engine_port)();
        let mut closed = c.closed.subscribe();
        let dial = async {
            let tcp = TcpStream::connect(("127.0.0.1", port))
                .await
                .map_err(|e| e.to_string())?;
            client_async(format!("ws://127.0.0.1:{port}"), tcp)
                .await
                .map(|(ws, _)| ws)
                .map_err(|e| e.to_string())
        };
        let dialed = tokio::select! {
            _ = raised(&mut closed) => return,
            r = timeout(ENGINE_DIAL, dial) => r.unwrap_or_else(|_| Err("no answer from the engine".into())),
        };
        let (mut sink, mut stream) = match dialed {
            Ok(ws) => ws.split(),
            Err(why) => return self.engine_ended(&c, Some(why)),
        };
        c.open.send_replace(true);
        self.tell(&json!({"t": "opened", "ch": c.ch}));
        let reason = loop {
            tokio::select! {
                _ = raised(&mut closed) => {
                    // What was handed on before the channel ended still reaches the engine.
                    let last = async {
                        while let Ok(m) = out.try_recv() {
                            sink.send(m).await?;
                        }
                        sink.close().await
                    };
                    let _ = timeout(Duration::from_secs(1), last).await;
                    return;
                }
                sent = out.recv() => match sent {
                    Some(m) => if let Err(e) = sink.send(m).await { break Some(e.to_string()); },
                    None => return,
                },
                got = stream.next() => match got {
                    Some(Ok(Message::Text(t))) => self.to_hub(&c, Payload::Text(t.to_string())),
                    Some(Ok(Message::Binary(b))) => self.to_hub(&c, Payload::Bin(b.to_vec())),
                    Some(Ok(Message::Close(_))) | None => break None,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => break Some(e.to_string()),
                },
            }
        };
        self.engine_ended(&c, reason);
    }

    /// The engine went away on its own: the hub hears it, once, if the channel is still the hub's.
    fn engine_ended(self: &Arc<Self>, c: &Arc<Channel>, reason: Option<String>) {
        if self.take_channel(c) {
            self.close_hub(c, reason);
        }
    }
}
