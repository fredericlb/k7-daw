//! frame.rs — one WebSocket frame as the relays move it: its bytes and whether it was binary, and the
//! JSON object a small text frame carries, read the way `JSON.parse` reads it for the gates' decisions.

use serde_json::{Map, Value};
use tokio_tungstenite::tungstenite::{Bytes, Message, Utf8Bytes};

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub data: Bytes,
    pub binary: bool,
}

/// A frame past this many bytes is never parsed to find its type.
pub const MAX_PARSED_BYTES: usize = 64 * 1024;

impl Frame {
    pub fn text(s: impl Into<String>) -> Self {
        Frame {
            data: Bytes::from(s.into().into_bytes()),
            binary: false,
        }
    }

    pub fn binary(b: impl Into<Bytes>) -> Self {
        Frame {
            data: b.into(),
            binary: true,
        }
    }

    /// A data message of a socket as a frame; control messages are none.
    pub fn of(m: Message) -> Option<Self> {
        match m {
            Message::Text(t) => Some(Frame {
                data: Bytes::from(t),
                binary: false,
            }),
            Message::Binary(b) => Some(Frame {
                data: b,
                binary: true,
            }),
            _ => None,
        }
    }

    /// The message a socket sends; a text frame's bytes are valid UTF-8, read lossily otherwise.
    pub fn message(&self) -> Message {
        if self.binary {
            return Message::Binary(self.data.clone());
        }
        match Utf8Bytes::try_from(self.data.clone()) {
            Ok(t) => Message::Text(t),
            Err(_) => Message::Text(String::from_utf8_lossy(&self.data).into_owned().into()),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// A small text frame's JSON object, or None for a binary frame, a large one, or anything else.
    pub fn object(&self) -> Option<Map<String, Value>> {
        if self.binary || self.data.len() > MAX_PARSED_BYTES {
            return None;
        }
        match serde_json::from_str(&String::from_utf8_lossy(&self.data)) {
            Ok(Value::Object(m)) => Some(m),
            _ => None,
        }
    }

    /// A text frame's `type`, as the gates read it.
    pub fn kind(&self) -> Option<String> {
        self.object()?.get("type")?.as_str().map(str::to_string)
    }
}
