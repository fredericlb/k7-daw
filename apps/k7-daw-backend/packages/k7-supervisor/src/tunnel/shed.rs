//! shed.rs — what the tunnel sends on to a slow peer: a frame the next one supersedes is DROPPED once too much
//! sits unsent, everything else always goes. Only two kinds qualify: the engine's `state` frame and its audio
//! stream's frames, the two the engine itself skips for a socket that is behind. Per channel, on one task.

/// Past this many unsent bytes toward a peer, a droppable frame is not sent.
pub const DROP_ABOVE_BYTES: usize = 256 * 1024;
/// Reported at least this often under a backlog that never clears: a minute of `state` frames.
pub const SHED_REPORT_EVERY: u64 = 1200;
/// The audio stream's magic, "LSTR" as a little-endian u32 (k7-runtime-server's `wire/stream.rs`).
const STREAM_MAGIC: [u8; 4] = *b"RTSL";

/// A frame the next one makes worthless: the engine's `state`, or a frame of its audio stream.
pub fn droppable_engine_frame(data: &[u8], binary: bool) -> bool {
    if binary {
        return data.starts_with(&STREAM_MAGIC);
    }
    // Cheap first: a frame that does not even spell the word is not one.
    if !data.windows(7).any(|w| w == b"\"state\"") {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(data)
        .ok()
        .is_some_and(|j| j.get("type").and_then(|t| t.as_str()) == Some("state"))
}

pub struct FrameShedder {
    shed: u64,
    caught_up: bool,
}

impl Default for FrameShedder {
    fn default() -> Self {
        FrameShedder {
            shed: 0,
            caught_up: true,
        }
    }
}

impl FrameShedder {
    /// Whether to send this frame, with `buffered` bytes still unsent toward the peer.
    pub fn admit(&mut self, data: &[u8], binary: bool, buffered: usize) -> bool {
        self.caught_up = buffered <= DROP_ABOVE_BYTES;
        if !self.caught_up && droppable_engine_frame(data, binary) {
            self.shed += 1;
            return false;
        }
        true
    }

    /// The frames dropped since the last report, once the peer caught up or a minute's worth piled up; else 0.
    pub fn take_shed(&mut self) -> u64 {
        if self.shed == 0 || (!self.caught_up && self.shed < SHED_REPORT_EVERY) {
            return 0;
        }
        std::mem::take(&mut self.shed)
    }
}
