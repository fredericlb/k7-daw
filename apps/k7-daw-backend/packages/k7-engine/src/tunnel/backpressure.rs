//! backpressure.rs — what a relay does with a frame for a peer that stopped reading: past 8 MiB unsent a frame a newer one replaces is dropped, past 64 MiB of what may not be
//! dropped the peer is closed, a reply is never dropped (`apps/k7-daw-backend/docs/engine/backpressure.md`).

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A stream frame as the engine's audio broadcast writes one: its magic first, little-endian.
    fn audio() -> Frame {
        let mut b = STREAM_MAGIC.to_le_bytes().to_vec();
        b.extend_from_slice(&[0; 28]);
        Frame::binary(b)
    }

    fn state() -> Frame {
        Frame::text(r#"{"levels":[0],"playing":true,"type":"state"}"#)
    }

    fn reply() -> Frame {
        Frame::text(r#"{"type":"plugins","plugins":[]}"#)
    }

    #[test]
    fn what_may_be_dropped_is_an_audio_broadcast_frame_a_state_or_a_meter_feed_never_a_reply() {
        assert!(is_droppable(&audio()));
        assert!(is_droppable(&state()));
        assert!(is_droppable(&Frame::text(r#"{"type":"io-levels"}"#)));
        assert!(is_droppable(&Frame::text(
            r#"{"type":"plugin-params-live"}"#
        )));
        assert!(!is_droppable(&reply()));
        assert!(!is_droppable(&Frame::binary(&b"not a stream frame"[..])));
        assert!(!is_droppable(&Frame::text("not json")));
    }

    #[test]
    fn everything_below_the_soft_limit_is_sent() {
        assert_eq!(admit(SOFT_LIMIT_BYTES - 1, &audio()), Admit::Send);
        assert_eq!(admit(SOFT_LIMIT_BYTES - 1, &reply()), Admit::Send);
    }

    #[test]
    fn a_replaceable_frame_past_the_soft_limit_is_dropped_and_a_reply_still_queued() {
        let past = SOFT_LIMIT_BYTES + 1;
        assert_eq!(admit(past, &audio()), Admit::Drop);
        assert_eq!(admit(past, &state()), Admit::Drop);
        assert_eq!(admit(past, &reply()), Admit::Send);
    }

    #[test]
    fn the_peer_is_given_up_on_only_when_replies_alone_pass_the_hard_limit() {
        assert_eq!(admit(HARD_LIMIT_BYTES, &reply()), Admit::Close);
        assert_eq!(admit(HARD_LIMIT_BYTES, &state()), Admit::Drop);
    }

    #[test]
    fn held_frames_refuse_past_their_frame_bound_and_give_back_what_they_held_in_order() {
        let mut h = HeldFrames::default();
        for i in 0..MAX_HELD_FRAMES {
            assert!(h.push(Frame::text(i.to_string())));
        }
        assert!(!h.push(Frame::text("one more")));
        let out = h.drain();
        assert_eq!(out.len(), MAX_HELD_FRAMES);
        assert_eq!(out[0], Frame::text("0"));
        assert!(h.push(Frame::text("again")));
    }

    #[test]
    fn held_frames_refuse_past_their_byte_bound() {
        let mut h = HeldFrames::default();
        assert!(h.push(Frame::binary(vec![0u8; MAX_HELD_BYTES])));
        assert!(!h.push(Frame::text("x")));
    }
}
