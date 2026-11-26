//! tunnel.rs — ONE supervisor's tunnel as the hub sees it: the channels it
//! carries, each one's unsent bytes counted apart so a stalled reader's channel is closed alone, the file
//! requests the agent makes. Uploads are `put.rs`'s.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use super::backpressure::HARD_LIMIT_BYTES;
use super::channel::{Channel, ChannelTx, Shared};
use super::files::{read_file_chunks, stat_paths};
use super::frames::{
    AgentMessage, BinaryHeader, Kind, Root, binary_frame_type, decode_binary, encode_binary,
    open_message, parse_agent_message, text_frame_type,
};
use super::put::Uploads;
use super::uploads::UploadLimits;
use crate::clock::Clock;
use crate::frame::Frame;
use crate::js;
use crate::log::Sink;
use crate::ws::Peer;

/// Past this many unsent bytes a file chunk waits: every other channel's frames queue behind it.
pub const HIGH_WATER_BYTES: usize = 1024 * 1024;
/// How long a close waits for the frames queued ahead of it to be delivered, as `drain.ts`' CLOSE_DRAIN_MS.
pub const CLOSE_DRAIN: Duration = Duration::from_millis(3000);
/// Frame types said to be ignored, at most.
const MAX_IGNORED: usize = 256;

struct Carried {
    shared: Arc<Shared>,
    roots: Arc<Vec<Root>>,
}

pub(crate) struct State {
    channels: HashMap<i64, Carried>,
    next_ch: i64,
    ignored: HashSet<String>,
    pub(crate) uploads: Uploads,
}

pub struct Tunnel {
    pub engine_id: String,
    pub peer: Peer,
    /// Cleared by every ping, set by every pong.
    pub alive: AtomicBool,
    state: Mutex<State>,
    pub(crate) log: Sink,
    pub(crate) limits: UploadLimits,
    pub(crate) clock: Clock,
}

impl Tunnel {
    pub fn new(
        engine_id: String,
        peer: Peer,
        log: Sink,
        limits: UploadLimits,
        clock: Clock,
    ) -> Arc<Self> {
        Arc::new(Tunnel {
            engine_id,
            peer,
            alive: AtomicBool::new(true),
            state: Mutex::new(State {
                channels: HashMap::new(),
                next_ch: 1,
                ignored: HashSet::new(),
                uploads: Uploads::default(),
            }),
            log,
            limits,
            clock,
        })
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A control message to the agent.
    pub(crate) fn tell(&self, m: &Value) {
        let text = js::stringify(m);
        let n = text.len();
        self.peer.send_message(Message::Text(text.into()), n, None);
    }

    pub fn send_text(&self, ch: i64, text: &str) {
        let frame = js::stringify(&json!({ "t": "data", "ch": ch, "text": text }));
        let n = frame.len();
        self.send_counted(ch, Message::Text(frame.into()), n);
    }

    pub fn send_binary(&self, ch: i64, bytes: &[u8]) {
        let frame = encode_binary(&json!({ "t": "data", "ch": ch }), bytes);
        let n = frame.len();
        self.send_counted(ch, Message::Binary(frame.into()), n);
    }

    /// One channel's frame, counted until written; a channel past its limit is closed, the tunnel kept.
    fn send_counted(&self, ch: i64, m: Message, n: usize) {
        let shared = {
            let s = self.lock();
            match s.channels.get(&ch) {
                Some(c) if self.peer.is_open() => c.shared.clone(),
                _ => return,
            }
        };
        let unsent = shared.unsent.load(Ordering::Acquire);
        if unsent + n > HARD_LIMIT_BYTES {
            (self.log)(&format!(
                "{} ch{ch}: {unsent} bytes unsent, closed",
                self.engine_id
            ));
            self.close(ch);
            shared.ended();
            return;
        }
        self.peer.send_message(m, n, Some(shared.unsent.clone()));
    }

    pub fn close(&self, ch: i64) {
        if self.lock().channels.remove(&ch).is_some() {
            self.tell(&json!({ "t": "close", "ch": ch }));
        }
    }

    /// A new channel sharing `roots`, to the engine or the agent's own jobs; it reads `Open` once answered.
    pub fn open(self: &Arc<Self>, roots: Vec<Root>, kind: Kind, synced: bool) -> Channel {
        let (shared, rx) = Shared::new();
        let ch = {
            let mut s = self.lock();
            let ch = s.next_ch;
            s.next_ch += 1;
            s.channels.insert(
                ch,
                Carried {
                    shared: shared.clone(),
                    roots: Arc::new(roots.clone()),
                },
            );
            ch
        };
        let text = open_message(ch, &roots, kind, synced);
        let n = text.len();
        self.peer.send_message(Message::Text(text.into()), n, None);
        Channel {
            tx: ChannelTx {
                ch,
                shared,
                tunnel: self.clone(),
            },
            rx,
        }
    }

    /// The socket went: every channel ends, every half-sent upload is dropped.
    pub fn shutdown(&self) {
        let (channels, uploads) = {
            let mut s = self.lock();
            let channels: Vec<_> = s.channels.drain().map(|(_, c)| c.shared).collect();
            (channels, s.uploads.drain())
        };
        for c in channels {
            c.ended();
        }
        for u in uploads {
            u.abort();
        }
    }

    /// A frame this hub does not read, said once per type.
    fn ignore(&self, kind: &str, t: &str) {
        {
            // Bounded: the types are the agent's to invent.
            let mut s = self.lock();
            if s.ignored.len() >= MAX_IGNORED || !s.ignored.insert(format!("{kind}:{t}")) {
                return;
            }
        }
        (self.log)(&format!(
            "{}: ignored a {kind} frame of type '{t}' — unknown here, or not the shape this server reads",
            self.engine_id
        ));
    }

    fn roots_of(&self, ch: i64) -> Arc<Vec<Root>> {
        self.lock()
            .channels
            .get(&ch)
            .map(|c| c.roots.clone())
            .unwrap_or_default()
    }

    /// One frame from the agent, checked for shape before anything reads it.
    pub fn on_frame(self: &Arc<Self>, f: Frame) {
        if f.binary {
            self.on_binary(&f.data);
        } else {
            self.on_text(&String::from_utf8_lossy(&f.data));
        }
    }

    fn on_text(self: &Arc<Self>, text: &str) {
        let Some(m) = parse_agent_message(text) else {
            return self.ignore("text", &text_frame_type(text));
        };
        let shared = |ch: i64| self.lock().channels.get(&ch).map(|c| c.shared.clone());
        match m {
            AgentMessage::Opened { ch } => {
                if let Some(c) = shared(ch) {
                    c.opened();
                }
            }
            AgentMessage::Close { ch, reason } => {
                let Some(c) = self.lock().channels.remove(&ch) else {
                    return;
                };
                if let Some(r) = reason.filter(|r| !r.is_empty()) {
                    (self.log)(&format!("{} ch{ch}: {r}", self.engine_id));
                }
                c.shared.ended();
            }
            AgentMessage::Data { ch, text } => {
                if let Some(c) = shared(ch) {
                    c.deliver(Frame::text(text));
                }
            }
            AgentMessage::Stat { req, ch, paths } => {
                let (t, roots) = (self.clone(), self.roots_of(ch));
                tokio::spawn(async move {
                    let entries: Vec<Value> = stat_paths(&roots, &paths)
                        .await
                        .iter()
                        .map(|e| e.json())
                        .collect();
                    t.tell(&json!({ "t": "stat-reply", "req": req, "entries": entries }));
                });
            }
            AgentMessage::Get { req, ch, path } => {
                let (t, roots) = (self.clone(), self.roots_of(ch));
                tokio::spawn(async move { t.serve_get(req, &roots, &path).await });
            }
        }
    }

    async fn serve_get(self: Arc<Self>, req: i64, roots: &[Root], path: &str) {
        let t = self.clone();
        let sent = read_file_chunks(roots, path, |chunk, last| {
            let t = t.clone();
            async move {
                while t.peer.buffered() > HIGH_WATER_BYTES && t.peer.is_open() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                let frame =
                    encode_binary(&json!({ "t": "chunk", "req": req, "last": last }), &chunk);
                let n = frame.len();
                t.peer.send_message(Message::Binary(frame.into()), n, None);
            }
        })
        .await;
        if let Err(e) = sent {
            self.tell(&json!({ "t": "get-error", "req": req, "error": e }));
        }
    }

    fn on_binary(self: &Arc<Self>, frame: &[u8]) {
        let Some((header, at)) = decode_binary(frame) else {
            return self.ignore("binary", &binary_frame_type(frame));
        };
        let payload = frame[at..].to_vec();
        match header {
            BinaryHeader::Data { ch } => {
                let c = self.lock().channels.get(&ch).map(|c| c.shared.clone());
                if let Some(c) = c {
                    c.deliver(Frame::binary(payload));
                }
            }
            BinaryHeader::Put {
                req,
                ch,
                path,
                last,
            } => {
                let roots = self.roots_of(ch);
                self.on_put(req, roots, path, last, payload);
            }
            BinaryHeader::Chunk { .. } => {}
        }
    }
}
