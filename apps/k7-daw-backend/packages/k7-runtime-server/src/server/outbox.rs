//! outbox.rs — what the engine pushes at one socket: a queue its writer task drains, so no caller ever waits
//! on a peer. A reply is kept up to a large cap; a state frame is droppable, skipped while the socket is behind.
//! What a reader gets wrong: the C++ server shared one queue and one writer across all sockets; here each
//! socket owns both, so a stuck peer delays only itself (`docs/server.md` beside this crate).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, watch};

/// A droppable frame is skipped for a socket with more than this unsent.
pub const MAX_SOCKET_BACKLOG: usize = 64 * 1024;
/// A reply is refused only past this, which means the writer has been stuck for a long while.
pub const MAX_QUEUED_REPLY_BYTES: usize = 32 * 1024 * 1024;

/// One thing the writer sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Text(Arc<str>),
    Binary(Arc<[u8]>),
    Ping,
    Close(u16, &'static str),
}

#[derive(Default)]
struct State {
    queue: VecDeque<Frame>,
    bytes: usize,
    // A close is queued or the socket is gone: nothing more is taken.
    shut: bool,
    warned_full: bool,
}

/// One socket's queue, shared by whoever posts and the task that writes.
pub struct Outbox {
    state: Mutex<State>,
    wake: Notify,
    gone: watch::Sender<bool>,
}

impl Default for Outbox {
    fn default() -> Self {
        Outbox {
            state: Mutex::new(State::default()),
            wake: Notify::new(),
            gone: watch::channel(false).0,
        }
    }
}

impl Outbox {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// ANY THREAD. Queue `payload`; false when it was refused (a droppable one past the backlog, a reply past the cap, a closed socket).
    pub fn post(&self, payload: &Arc<str>, droppable: bool) -> bool {
        {
            let mut s = self.lock();
            if s.shut {
                return false;
            }
            let over = if droppable {
                s.bytes > MAX_SOCKET_BACKLOG
            } else {
                s.bytes + payload.len() > MAX_QUEUED_REPLY_BYTES
            };
            if over {
                if !droppable && !s.warned_full {
                    s.warned_full = true;
                    eprintln!(
                        "[outbox] full — replies are being dropped until the writer catches up"
                    );
                }
                return false;
            }
            s.bytes += payload.len();
            s.queue.push_back(Frame::Text(payload.clone()));
        }
        self.wake.notify_one();
        true
    }

    /// ANY THREAD. A binary frame, refused while the socket has more than `max_backlog` bytes unsent.
    pub fn post_binary(&self, payload: &Arc<[u8]>, max_backlog: usize) -> bool {
        {
            let mut s = self.lock();
            if s.shut || s.bytes > max_backlog {
                return false;
            }
            s.bytes += payload.len();
            s.queue.push_back(Frame::Binary(payload.clone()));
        }
        self.wake.notify_one();
        true
    }

    /// A ping, behind what is queued; false when the socket is closed.
    pub fn ping(&self) -> bool {
        self.push_control(Frame::Ping, false)
    }

    /// A close frame, behind what is queued: nothing is taken after it, and the writer ends once it is sent.
    pub fn close(&self, code: u16, reason: &'static str) -> bool {
        self.push_control(Frame::Close(code, reason), true)
    }

    fn push_control(&self, frame: Frame, shut: bool) -> bool {
        {
            let mut s = self.lock();
            if s.shut {
                return false;
            }
            s.shut = shut;
            s.queue.push_back(frame);
        }
        self.wake.notify_one();
        true
    }

    /// The socket is gone: what is queued is dropped, and the writer and the reader are told.
    pub fn shutdown(&self) {
        {
            let mut s = self.lock();
            s.shut = true;
            s.queue.clear();
            s.bytes = 0;
        }
        self.gone.send_replace(true);
        self.wake.notify_one();
    }

    pub fn is_gone(&self) -> bool {
        *self.gone.borrow()
    }

    /// Resolves once `shutdown` ran.
    pub async fn gone(&self) {
        let mut rx = self.gone.subscribe();
        let _ = rx.wait_for(|g| *g).await;
    }

    /// Bytes queued and not yet sent.
    pub fn queued_bytes(&self) -> usize {
        self.lock().bytes
    }

    /// The writer's next frame, waiting for one; `None` once the socket is gone.
    pub async fn next(&self) -> Option<Frame> {
        loop {
            {
                let mut s = self.lock();
                if self.is_gone() {
                    return None;
                }
                if let Some(frame) = s.queue.pop_front() {
                    let sent = match &frame {
                        Frame::Text(t) => t.len(),
                        Frame::Binary(b) => b.len(),
                        _ => 0,
                    };
                    if sent > 0 {
                        s.bytes = s.bytes.saturating_sub(sent);
                        if s.bytes == 0 {
                            s.warned_full = false;
                        }
                    }
                    return Some(frame);
                }
            }
            self.wake.notified().await;
        }
    }
}
