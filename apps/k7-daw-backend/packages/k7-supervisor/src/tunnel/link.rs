//! link.rs — the agent's one socket to the hub as its senders see it: a queue the writer task drains, the bytes
//! still unsent (the WebSocket's `bufferedAmount`, which tokio-tungstenite does not give), and whether it is up.
//! One per agent, outliving the sockets it is attached to in turn; a send on a down link says so.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::{Notify, mpsc};

pub enum Out {
    Text(String),
    Binary(Vec<u8>),
    Ping,
}

#[derive(Default)]
pub struct Link {
    tx: Mutex<Option<mpsc::UnboundedSender<Out>>>,
    buffered: AtomicUsize,
    drained: Notify,
}

impl Link {
    fn tx(&self) -> MutexGuard<'_, Option<mpsc::UnboundedSender<Out>>> {
        self.tx.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The writer's queue of the socket now up; what the last one left unsent is forgotten.
    pub fn attach(&self, tx: mpsc::UnboundedSender<Out>) {
        self.buffered.store(0, Ordering::SeqCst);
        *self.tx() = Some(tx);
    }

    pub fn detach(&self) {
        *self.tx() = None;
        self.buffered.store(0, Ordering::SeqCst);
        self.drained.notify_waiters();
    }

    pub fn is_open(&self) -> bool {
        self.tx().as_ref().is_some_and(|t| !t.is_closed())
    }

    /// Bytes handed to the writer and not yet written.
    pub fn buffered(&self) -> usize {
        self.buffered.load(Ordering::SeqCst)
    }

    fn send(&self, out: Out, n: usize) -> bool {
        let guard = self.tx();
        let Some(tx) = guard.as_ref() else {
            return false;
        };
        self.buffered.fetch_add(n, Ordering::SeqCst);
        if tx.send(out).is_err() {
            self.buffered.fetch_sub(n, Ordering::SeqCst);
            return false;
        }
        true
    }

    pub fn send_text(&self, text: String) -> bool {
        let n = text.len();
        self.send(Out::Text(text), n)
    }

    pub fn send_binary(&self, bytes: Vec<u8>) -> bool {
        let n = bytes.len();
        self.send(Out::Binary(bytes), n)
    }

    pub fn send_ping(&self) -> bool {
        self.send(Out::Ping, 0)
    }

    /// The writer has put `n` bytes on the socket.
    pub fn written(&self, n: usize) {
        let _ = self
            .buffered
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |b| {
                Some(b.saturating_sub(n))
            });
        self.drained.notify_waiters();
    }

    /// Wait until no more than `limit` bytes are unsent, or the link is down.
    pub async fn wait_below(&self, limit: usize) {
        while self.is_open() && self.buffered() > limit {
            let woken = self.drained.notified();
            if self.buffered() <= limit {
                return;
            }
            let _ = tokio::time::timeout(Duration::from_millis(50), woken).await;
        }
    }
}
