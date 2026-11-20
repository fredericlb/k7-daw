//! ws.rs — one WebSocket as the relays drive it: a reader task handing frames over a bounded queue, a writer
//! task draining an outbox whose unsent bytes are COUNTED (`bufferedAmount`, which backpressure reads), and a
//! close that ends a peer gone silent. Every socket here is served this way, so none blocks another.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};

use crate::frame::Frame;
use crate::tunnel::frames::MAX_FRAME_BYTES;

/// How long a close waits for the peer's own before the socket is cut, as `ws`'s closeTimeout.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(30);
/// Frames read ahead of the owner: past it the reader waits, and TCP holds the peer back.
const READ_AHEAD: usize = 64;

/// The frame cap of every socket here: `MAX_FRAME_BYTES`, not tungstenite's 64 MiB.
pub fn config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES))
}

enum Out {
    Msg(Message, usize, Option<Arc<AtomicUsize>>),
    Close(Option<(u16, String)>),
    Terminate,
}

/// What a socket's owner reads.
pub enum Event {
    Frame(Frame),
    Pong,
    /// The socket ended; the code its peer closed with, when it sent one.
    Closed(Option<u16>),
}

/// The sending half of one socket, cheap to clone.
#[derive(Clone)]
pub struct Peer {
    tx: mpsc::UnboundedSender<Out>,
    queued: Arc<AtomicUsize>,
    open: Arc<AtomicBool>,
}

impl Peer {
    /// Whether frames sent now still go out.
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    /// Bytes handed to the socket and not yet written out.
    pub fn buffered(&self) -> usize {
        self.queued.load(Ordering::Acquire)
    }

    pub fn send(&self, f: &Frame) {
        self.send_message(f.message(), f.len(), None);
    }

    /// A message also counted in `extra` until written, as one tunnel channel's own bytes are.
    pub fn send_message(&self, m: Message, n: usize, extra: Option<Arc<AtomicUsize>>) {
        if !self.is_open() {
            return;
        }
        self.queued.fetch_add(n, Ordering::AcqRel);
        if let Some(e) = &extra {
            e.fetch_add(n, Ordering::AcqRel);
        }
        if self.tx.send(Out::Msg(m, n, extra.clone())).is_err() {
            self.queued.fetch_sub(n, Ordering::AcqRel);
            if let Some(e) = extra {
                e.fetch_sub(n, Ordering::AcqRel);
            }
        }
    }

    pub fn ping(&self) {
        self.send_message(Message::Ping(Default::default()), 0, None);
    }

    /// A close frame with `code`, then the end once the peer answers or CLOSE_TIMEOUT passes.
    pub fn close(&self, code: u16, reason: &str) {
        if self.open.swap(false, Ordering::AcqRel) {
            let _ = self.tx.send(Out::Close(Some((code, reason.to_string()))));
        }
    }

    /// A close frame with no status, as `ws.close()` sends it.
    pub fn shut(&self) {
        if self.open.swap(false, Ordering::AcqRel) {
            let _ = self.tx.send(Out::Close(None));
        }
    }

    /// Cut at once, no close frame.
    pub fn terminate(&self) {
        self.open.store(false, Ordering::Release);
        let _ = self.tx.send(Out::Terminate);
    }
}

/// Serves `ws`: its sending half, and what it reads, ending with one `Closed`.
pub fn spawn<S>(ws: WebSocketStream<S>) -> (Peer, mpsc::Receiver<Event>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Out>();
    let (events, out) = mpsc::channel(READ_AHEAD);
    let queued = Arc::new(AtomicUsize::new(0));
    let open = Arc::new(AtomicBool::new(true));
    let peer = Peer {
        tx,
        queued: queued.clone(),
        open: open.clone(),
    };

    let writer = tokio::spawn(async move {
        let mut done: Vec<(usize, Option<Arc<AtomicUsize>>)> = Vec::new();
        while let Some(first) = rx.recv().await {
            let mut next = Some(first);
            let mut ok = true;
            // A batch: every message ready now is fed, then flushed once.
            while let Some(out) = next.take() {
                match out {
                    Out::Msg(m, n, extra) => {
                        ok = ok && sink.feed(m).await.is_ok();
                        done.push((n, extra));
                    }
                    Out::Close(c) => {
                        let frame = c.map(|(code, reason)| CloseFrame {
                            code: CloseCode::from(code),
                            reason: reason.into(),
                        });
                        let _ = sink.send(Message::Close(frame)).await;
                        settle(&queued, &mut done);
                        return true;
                    }
                    Out::Terminate => {
                        settle(&queued, &mut done);
                        return false;
                    }
                }
                if done.len() < 256 {
                    next = rx.try_recv().ok();
                }
            }
            ok = ok && sink.flush().await.is_ok();
            settle(&queued, &mut done);
            if !ok {
                return false;
            }
        }
        false
    });

    let closer = peer.clone();
    tokio::spawn(async move {
        let mut writer = writer;
        let mut code = None;
        loop {
            tokio::select! {
                read = stream.next() => match read {
                    Some(Ok(Message::Text(t))) => {
                        if events.send(Event::Frame(Frame { data: t.into(), binary: false })).await.is_err() { break; }
                    }
                    Some(Ok(Message::Binary(b))) => {
                        if events.send(Event::Frame(Frame { data: b, binary: true })).await.is_err() { break; }
                    }
                    Some(Ok(Message::Pong(_))) => { let _ = events.send(Event::Pong).await; }
                    Some(Ok(Message::Close(c))) => { code = c.map(|c| u16::from(c.code)); }
                    Some(Ok(_)) => {}
                    // A frame past the cap is closed 1009, as `ws` closes past its `maxPayload`, its payload unread.
                    Some(Err(tokio_tungstenite::tungstenite::Error::Capacity(_))) => {
                        closer.close(1009, "Max payload size exceeded");
                        let _ = tokio::time::timeout(CLOSE_TIMEOUT, &mut writer).await;
                        break;
                    }
                    Some(Err(_)) | None => break,
                },
                // The writer ended: a terminate, a failed write, or a close sent whose answer is awaited below.
                w = &mut writer => {
                    if matches!(w, Ok(true)) {
                        let _ = tokio::time::timeout(CLOSE_TIMEOUT, async {
                            while let Some(Ok(m)) = stream.next().await {
                                if let Message::Close(c) = m { code = c.map(|c| u16::from(c.code)); }
                            }
                        }).await;
                    }
                    break;
                }
            }
        }
        writer.abort();
        open.store(false, Ordering::Release);
        let _ = events.send(Event::Closed(code)).await;
    });
    (peer, out)
}

fn settle(queued: &AtomicUsize, done: &mut Vec<(usize, Option<Arc<AtomicUsize>>)>) {
    for (n, extra) in done.drain(..) {
        queued.fetch_sub(n, Ordering::AcqRel);
        if let Some(e) = extra {
            e.fetch_sub(n, Ordering::AcqRel);
        }
    }
}
