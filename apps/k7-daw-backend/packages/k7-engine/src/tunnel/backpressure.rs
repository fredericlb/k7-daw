//! backpressure.rs — what a relay does with a frame for a peer that stopped reading, as `backpressure.ts`
//! decides it: past 8 MiB unsent a frame a newer one replaces is dropped, past 64 MiB of what may not be
//! dropped the peer is closed, a reply is never dropped (`apps/k7-daw-frontend/packages/engine-tunnel/docs/backpressure.md`).

use std::collections::VecDeque;

use crate::frame::{Frame, MAX_PARSED_BYTES};

pub const SOFT_LIMIT_BYTES: usize = 8 * 1024 * 1024;
pub const HARD_LIMIT_BYTES: usize = 64 * 1024 * 1024;
/// "Try again later".
pub const OVERLOAD_CLOSE: u16 = 1013;
/// The engine's text pushes that a newer one of the same kind replaces.
const DROPPABLE_TYPES: [&str; 3] = ["state", "io-levels", "plugin-params-live"];
/// `STREAM_MAGIC` of `apps/k7-daw-frontend/packages/engine-protocol/stream.ts`, little-endian at the frame's start.
const STREAM_MAGIC: u32 = 0x4c53_5452;
/// Frames held before the far end is connected, and their bytes.
pub const MAX_HELD_FRAMES: usize = 256;
pub const MAX_HELD_BYTES: usize = 8 * 1024 * 1024;

/// Whether a newer frame replaces this one.
pub fn is_droppable(f: &Frame) -> bool {
    if f.binary {
        return f.data.len() >= 4
            && u32::from_le_bytes([f.data[0], f.data[1], f.data[2], f.data[3]]) == STREAM_MAGIC;
    }
    // A string's `.length` in JavaScript, bytes here: a text frame near the bound is never a push.
    if f.data.len() > MAX_PARSED_BYTES {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(&f.data)
        .ok()
        .and_then(|m| {
            m.get("type")
                .and_then(|t| t.as_str())
                .map(|t| DROPPABLE_TYPES.contains(&t))
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    Send,
    Drop,
    Close,
}

/// What to do with a frame for a peer with `buffered` bytes unsent; classified only under pressure.
pub fn admit(buffered: usize, f: &Frame) -> Admit {
    if buffered < SOFT_LIMIT_BYTES {
        return Admit::Send;
    }
    if is_droppable(f) {
        return Admit::Drop;
    }
    if buffered >= HARD_LIMIT_BYTES {
        Admit::Close
    } else {
        Admit::Send
    }
}

/// A bounded queue of frames waiting for a peer to connect.
#[derive(Default)]
pub struct HeldFrames {
    frames: VecDeque<Frame>,
    bytes: usize,
}

impl HeldFrames {
    /// False past the bounds: the client is then closed.
    pub fn push(&mut self, f: Frame) -> bool {
        if self.frames.len() >= MAX_HELD_FRAMES || self.bytes + f.len() > MAX_HELD_BYTES {
            return false;
        }
        self.bytes += f.len();
        self.frames.push_back(f);
        true
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn drain(&mut self) -> Vec<Frame> {
        self.bytes = 0;
        self.frames.drain(..).collect()
    }
}
