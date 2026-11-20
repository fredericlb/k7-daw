//! channel.rs — one virtual connection to an engine carried by a supervisor's tunnel, as `channel.ts` shapes it:
//! CONNECTING until the agent answers, OPEN, then CLOSED once either end lets go or the tunnel drops. Its
//! unsent bytes are its OWN, so one stalled channel is told apart from the tunnel's others.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use tokio::sync::mpsc;

use super::hub_tunnel::Tunnel;
use crate::frame::Frame;

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 3;

/// What a channel's owner reads.
#[derive(Debug)]
pub enum ChannelEvent {
    Open,
    Frame(Frame),
    Close,
}

/// The half the tunnel holds.
pub struct Shared {
    state: AtomicU8,
    pub(crate) unsent: Arc<AtomicUsize>,
    tx: mpsc::UnboundedSender<ChannelEvent>,
}

impl Shared {
    pub(crate) fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<ChannelEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let s = Shared {
            state: AtomicU8::new(CONNECTING),
            unsent: Arc::new(AtomicUsize::new(0)),
            tx,
        };
        (Arc::new(s), rx)
    }

    /// The agent reached the engine.
    pub(crate) fn opened(&self) {
        if self
            .state
            .compare_exchange(CONNECTING, OPEN, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let _ = self.tx.send(ChannelEvent::Open);
        }
    }

    /// An engine frame, while open.
    pub(crate) fn deliver(&self, f: Frame) {
        if self.state.load(Ordering::Acquire) == OPEN {
            let _ = self.tx.send(ChannelEvent::Frame(f));
        }
    }

    /// Either end let go, or the tunnel dropped.
    pub(crate) fn ended(&self) {
        if self.state.swap(CLOSED, Ordering::AcqRel) != CLOSED {
            let _ = self.tx.send(ChannelEvent::Close);
        }
    }

    pub fn is_open(&self) -> bool {
        self.state.load(Ordering::Acquire) == OPEN
    }
}

/// The sending half of a channel, cheap to clone.
#[derive(Clone)]
pub struct ChannelTx {
    pub ch: i64,
    pub(crate) shared: Arc<Shared>,
    pub(crate) tunnel: Arc<Tunnel>,
}

impl ChannelTx {
    pub fn is_open(&self) -> bool {
        self.shared.is_open()
    }

    pub fn buffered(&self) -> usize {
        self.shared.unsent.load(Ordering::Acquire)
    }

    pub fn send(&self, f: &Frame) {
        if !self.is_open() {
            return;
        }
        if f.binary {
            self.tunnel.send_binary(self.ch, &f.data);
        } else {
            self.tunnel
                .send_text(self.ch, &String::from_utf8_lossy(&f.data));
        }
    }

    pub fn close(&self) {
        if self.shared.state.load(Ordering::Acquire) == CLOSED {
            return;
        }
        self.tunnel.close(self.ch);
        self.shared.ended();
    }
}

/// A channel: its sending half and what it reads.
pub struct Channel {
    pub tx: ChannelTx,
    pub rx: mpsc::UnboundedReceiver<ChannelEvent>,
}
