//! gate.rs — what the relay lets through for an account holding NO claim on the engine:
//! the probe a picker needs and its answers; anything more is HELD while a claim may be landing,
//! then the socket closes with 4403. A `whoami` loses its token for an account that may not use the engine.

use std::collections::VecDeque;

use crate::clock::Clock;
use crate::frame::Frame;
use crate::js;

/// What a browser may send before it holds a claim: what `probe.ts` and `compat.ts` send.
const PROBE_REQUESTS: [&str; 4] = ["whoami", "pair", "list-plugins", "plugin-scan"];
/// What an engine may answer a browser without a claim.
const PROBE_ANSWERS: [&str; 4] = ["whoami", "plugins", "plugin-scan", "error"];
/// How long frames past the probe wait for a claim before the socket is closed.
pub const CLAIM_GRACE_MS: i64 = 10_000;
/// The close code of a socket that never held a claim: the 403 of a WebSocket.
pub const NO_CLAIM_CLOSE: u16 = 4403;
/// How often a waiting gate asks for the claim again.
pub const POLL_MS: u64 = 200;
/// How long a claim seen holding is trusted for the engine's frames.
const CLAIM_REUSE_MS: i64 = 250;
const MAX_HELD_FRAMES: usize = 256;
const MAX_HELD_BYTES: usize = 8 * 1024 * 1024;

/// What the gate asks and where it sends.
pub trait Sinks: Send {
    /// Whether the ticket's account holds a live claim on this engine now.
    fn holds(&self) -> bool;
    /// Whether the ticket's account may use this engine, and so read its token.
    fn may_read_token(&self) -> bool;
    fn to_engine(&mut self, f: Frame);
    fn close(&mut self, code: u16, reason: &str);
}

pub struct Gate<S: Sinks> {
    sinks: S,
    held: VecDeque<Frame>,
    held_bytes: usize,
    since: i64,
    waiting: bool,
    closed: bool,
    held_at: Option<i64>,
    grace_ms: i64,
    clock: Clock,
}

fn kind_in(f: &Frame, set: &[&str]) -> bool {
    f.kind().is_some_and(|k| set.contains(&k.as_str()))
}

/// The frame without the engine token a `whoami` carries; any other frame as it came.
pub fn without_token(f: Frame) -> Frame {
    let Some(mut m) = f.object() else { return f };
    if m.get("type").and_then(|t| t.as_str()) != Some("whoami") || !m.contains_key("token") {
        return f;
    }
    m.shift_remove("token");
    Frame::text(js::stringify(&serde_json::Value::Object(m)))
}

impl<S: Sinks> Gate<S> {
    pub fn new(sinks: S, grace_ms: i64, clock: Clock) -> Self {
        Gate {
            sinks,
            held: VecDeque::new(),
            held_bytes: 0,
            since: 0,
            waiting: false,
            closed: false,
            held_at: None,
            grace_ms,
            clock,
        }
    }

    pub fn sinks(&mut self) -> &mut S {
        &mut self.sinks
    }

    /// Whether frames wait for a claim, and `tick` must be called every POLL_MS.
    pub fn waiting(&self) -> bool {
        self.waiting
    }

    /// A frame from the browser: forwarded, held for a claim, or the end of the socket.
    pub fn from_client(&mut self, f: Frame) {
        if self.closed {
            return;
        }
        if self.sinks.holds() {
            self.flush();
            self.sinks.to_engine(f);
            return;
        }
        // Order is kept: once one frame waits, every later one waits behind it, probe or not.
        if self.held.is_empty() && kind_in(&f, &PROBE_REQUESTS) {
            self.sinks.to_engine(f);
            return;
        }
        self.hold(f);
    }

    /// The frame from the engine as it may reach the browser, or None.
    pub fn from_engine(&mut self, f: Frame) -> Option<Frame> {
        if self.closed {
            return None;
        }
        if !(self.holds_recently() || kind_in(&f, &PROBE_ANSWERS)) {
            return None;
        }
        // Only a text `whoami` carries the token.
        if f.binary || self.sinks.may_read_token() {
            return Some(f);
        }
        // A frame that cannot be read cannot be stripped: one that may carry the token does not pass.
        if f.object().is_none() && f.data.windows(7).any(|w| w == b"\"token\"") {
            return None;
        }
        Some(without_token(f))
    }

    fn holds_recently(&mut self) -> bool {
        let now = self.clock.now_ms();
        if self.held_at.is_some_and(|t| now - t < CLAIM_REUSE_MS) {
            return true;
        }
        if !self.sinks.holds() {
            return false;
        }
        self.held_at = Some(now);
        true
    }

    pub fn dispose(&mut self) {
        self.closed = true;
        self.waiting = false;
        self.held.clear();
    }

    fn hold(&mut self, f: Frame) {
        let size = f.len();
        if self.held.len() >= MAX_HELD_FRAMES || self.held_bytes + size > MAX_HELD_BYTES {
            return self.shut("too much sent before claiming this engine");
        }
        if self.held.is_empty() {
            self.since = self.clock.now_ms();
            self.waiting = true;
        }
        self.held.push_back(f);
        self.held_bytes += size;
    }

    /// Called every POLL_MS while `waiting`.
    pub fn tick(&mut self) {
        if self.closed {
            self.waiting = false;
        } else if self.sinks.holds() {
            self.flush();
        } else if self.clock.now_ms() - self.since >= self.grace_ms {
            self.shut("no claim on this engine: select it before driving it");
        }
    }

    fn flush(&mut self) {
        self.waiting = false;
        self.held_bytes = 0;
        while let Some(f) = self.held.pop_front() {
            self.sinks.to_engine(f);
        }
    }

    fn shut(&mut self, reason: &str) {
        self.dispose();
        self.sinks.close(NO_CLAIM_CLOSE, reason);
    }
}

#[cfg(test)]
mod tests;
