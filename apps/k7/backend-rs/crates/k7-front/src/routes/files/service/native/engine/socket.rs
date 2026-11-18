//! One session's socket to the engine (2214): dialled lazily, re-dialled with a jittered doubling wait, frames held in an
//! outbox while it is down. The task owns the writer; the session only pushes frames onto an unbounded channel.
//! What a reader gets wrong: a reconnect inside this task keeps the link's number; only a new `Socket` is a new link.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use k7_file_service::engine::wire::{In, Outbox, decode_in};
use k7_file_service::engine::{LOCAL_RECONNECT_MAX_MS, jittered_backoff_ms};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

/// How long an upgrade the engine never answers is waited for, as the TypeScript's `handshakeTimeout`.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// What the socket tells its session.
pub enum Input {
    /// Opened, the outbox already replayed.
    Open,
    /// Went down; the task re-dials after its wait.
    Down,
    Frame(In),
    /// A refused dial or a failed handshake, worded for the console.
    Error(String),
}

pub type Handler = Arc<dyn Fn(Input) + Send + Sync>;

/// A socket and the task behind it; dropping it closes the connection.
pub struct Socket {
    pub link: u64,
    tx: mpsc::UnboundedSender<Value>,
    ready: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
    task: Option<JoinHandle<()>>,
}

impl Socket {
    pub fn dial(
        rt: &tokio::runtime::Handle,
        url: String,
        session: String,
        link: u64,
        on: Handler,
    ) -> Socket {
        let (tx, rx) = mpsc::unbounded_channel();
        let ready = Arc::new(AtomicBool::new(false));
        let queued = Arc::new(AtomicUsize::new(0));
        let task = rt.spawn(run(url, session, rx, ready.clone(), queued.clone(), on));
        Socket {
            link,
            tx,
            ready,
            queued,
            task: Some(task),
        }
    }

    /// Queue a frame: written when the socket is open, held in the outbox while it is not. False once the task is gone.
    pub fn send(&self, frame: Value) -> bool {
        self.queued.fetch_add(1, Ordering::Relaxed);
        self.tx.send(frame).is_ok()
    }

    /// Frames queued and not yet taken by the writer or the outbox.
    #[cfg(test)]
    pub fn backlog(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Close once what is queued is written: the task ends when its channel is empty and closed, where a drop cuts it short.
    pub fn finish(mut self) {
        self.task = None;
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}

/// The largest frame read from an engine (D1): the plugin catalogue, the biggest real one, is under a megabyte; a bigger frame closes the socket.
const MAX_FRAME: usize = 16 << 20;

/// How long a write may take before the engine is read as stalled and the socket as down (D4).
const SEND: Duration = if cfg!(test) {
    Duration::from_millis(1500)
} else {
    Duration::from_secs(10)
};

fn limits() -> tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
    tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME))
}

/// One write, bounded by `SEND`; a timeout is a failure like any other.
async fn write_bounded<S>(write: &mut S, text: String) -> bool
where
    S: SinkExt<Message> + Unpin,
{
    matches!(
        tokio::time::timeout(SEND, write.send(Message::text(text))).await,
        Ok(Ok(()))
    )
}

fn random() -> f64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("the system's random source");
    (u64::from_le_bytes(b) >> 11) as f64 / (1u64 << 53) as f64
}

async fn run(
    url: String,
    session: String,
    mut rx: mpsc::UnboundedReceiver<Value>,
    ready: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
    on: Handler,
) {
    let mut outbox = Outbox::default();
    let mut attempt = 0u32;
    loop {
        let dialled = tokio::time::timeout(
            HANDSHAKE,
            tokio_tungstenite::connect_async_with_config(&url, Some(limits()), false),
        )
        .await;
        match dialled {
            Ok(Ok((ws, _))) => {
                let opened = Instant::now();
                let mut heard = false;
                let (mut write, mut read) = ws.split();
                let mut alive = true;
                for f in outbox.drain(Instant::now()) {
                    if !write_bounded(&mut write, f.to_string()).await {
                        alive = false;
                        break;
                    }
                }
                if alive {
                    ready.store(true, Ordering::Relaxed);
                    on(Input::Open);
                }
                while alive {
                    tokio::select! {
                        out = rx.recv() => match out {
                            Some(f) => {
                                queued.fetch_sub(1, Ordering::Relaxed);
                                if !write_bounded(&mut write, f.to_string()).await {
                                    // Not written: held for the next socket, the newest command being the one that still counts.
                                    outbox.push(f, Instant::now());
                                    alive = false;
                                }
                            }
                            None => return,
                        },
                        got = read.next() => match got {
                            Some(Ok(Message::Text(t))) => {
                                heard = true;
                                on(Input::Frame(decode_in(t.as_str(), &session)));
                            }
                            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => alive = false,
                            Some(Ok(_)) => {}
                        },
                    }
                }
                ready.store(false, Ordering::Relaxed);
                // The wait starts over only for a socket that earned it: one that stayed open or carried a frame (D3).
                if heard || opened.elapsed() >= Duration::from_millis(LOCAL_RECONNECT_MAX_MS) {
                    attempt = 0;
                }
                on(Input::Down);
            }
            Ok(Err(e)) => on(Input::Error(e.to_string())),
            Err(_) => on(Input::Error("the upgrade was never answered".into())),
        }
        if !pause(&mut rx, &mut outbox, &queued, attempt).await {
            return;
        }
        attempt = attempt.saturating_add(1);
    }
}

/// Wait out the backoff, keeping what is sent meanwhile; false when the session let go of the socket.
async fn pause(
    rx: &mut mpsc::UnboundedReceiver<Value>,
    outbox: &mut Outbox,
    queued: &AtomicUsize,
    attempt: u32,
) -> bool {
    let wait = jittered_backoff_ms(attempt, LOCAL_RECONNECT_MAX_MS, random());
    let sleep = tokio::time::sleep(Duration::from_millis(wait as u64));
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = &mut sleep => return true,
            out = rx.recv() => match out {
                Some(f) => {
                    queued.fetch_sub(1, Ordering::Relaxed);
                    outbox.push(f, Instant::now());
                }
                None => return false,
            },
        }
    }
}
