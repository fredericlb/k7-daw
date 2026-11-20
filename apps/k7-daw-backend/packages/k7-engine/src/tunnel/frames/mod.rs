//! frames.rs — the tunnel's wire, `frames.ts` to the byte: JSON control messages as TEXT frames, and bytes
//! (a file chunk, a binary engine frame) as BINARY frames led by a 4-byte big-endian header length and a
//! JSON header. Nothing off the network is trusted for shape (`apps/k7-daw-frontend/packages/engine-tunnel/docs/tunnel.md`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::js;

pub const TUNNEL_PATH: &str = "/api/engine-tunnel";
pub const TUNNEL_TOKEN_HEADER: &str = "x-studio-engine-token";
pub const TUNNEL_SECRET_HEADER: &str = "x-studio-tunnel-secret";
pub const TUNNEL_PROTOCOL_HEADER: &str = "x-studio-engine-protocol";
/// File chunks are cut to this.
pub const CHUNK_BYTES: usize = 256 * 1024;
/// The largest frame the hub and the relay accept.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// A tree the server shares with a channel, and what the engine's machine calls it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Root {
    /// The absolute path on the SERVER.
    pub remote: String,
    /// A folder name under the agent's cache, never a path.
    pub name: String,
    pub writable: bool,
    #[serde(rename = "perChild", skip_serializing_if = "Option::is_none", default)]
    pub per_child: Option<bool>,
}

/// What a channel carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Engine,
    Jobs,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Engine => "engine",
            Kind::Jobs => "jobs",
        }
    }
}

/// `Number.isSafeInteger`.
fn safe_int(v: Option<&Value>) -> Option<i64> {
    let n = v?.as_f64()?;
    (n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0).then_some(n as i64)
}

fn opt_str(v: Option<&Value>) -> Option<Option<String>> {
    match v {
        None => Some(None),
        Some(Value::String(s)) => Some(Some(s.clone())),
        Some(_) => None,
    }
}

/// Agent → hub, checked for shape.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentMessage {
    Opened {
        ch: i64,
    },
    Close {
        ch: i64,
        reason: Option<String>,
    },
    Data {
        ch: i64,
        text: String,
    },
    Stat {
        req: i64,
        ch: i64,
        paths: Vec<String>,
    },
    Get {
        req: i64,
        ch: i64,
        path: String,
    },
    RtcAnswer {
        req: i64,
        sdp: Option<String>,
        error: Option<String>,
    },
}

/// `parseAgentMessage`: an agent's text frame, or None when it is not JSON or not one of the shapes.
pub fn parse_agent_message(text: &str) -> Option<AgentMessage> {
    let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
        return None;
    };
    let int = |k: &str| safe_int(m.get(k));
    let s = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_string);
    Some(match m.get("t")?.as_str()? {
        "opened" => AgentMessage::Opened { ch: int("ch")? },
        "close" => AgentMessage::Close {
            ch: int("ch")?,
            reason: opt_str(m.get("reason"))?,
        },
        "data" => AgentMessage::Data {
            ch: int("ch")?,
            text: s("text")?,
        },
        "stat" => {
            let paths = m.get("paths")?.as_array()?;
            let paths: Option<Vec<String>> = paths
                .iter()
                .map(|p| p.as_str().map(str::to_string))
                .collect();
            AgentMessage::Stat {
                req: int("req")?,
                ch: int("ch")?,
                paths: paths?,
            }
        }
        "get" => AgentMessage::Get {
            req: int("req")?,
            ch: int("ch")?,
            path: s("path")?,
        },
        "rtc-answer" => AgentMessage::RtcAnswer {
            req: int("req")?,
            sdp: opt_str(m.get("sdp"))?,
            error: opt_str(m.get("error"))?,
        },
        _ => return None,
    })
}

/// A binary frame's header, checked for shape.
#[derive(Clone, Debug, PartialEq)]
pub enum BinaryHeader {
    Data {
        ch: i64,
    },
    Chunk {
        req: i64,
        last: bool,
    },
    Put {
        req: i64,
        ch: i64,
        path: String,
        last: bool,
    },
}

/// `decodeBinary`: the header and the payload's offset, or None.
pub fn decode_binary(frame: &[u8]) -> Option<(BinaryHeader, usize)> {
    let (len, start) = header_bounds(frame)?;
    let Ok(Value::Object(h)) =
        serde_json::from_str::<Value>(&String::from_utf8_lossy(&frame[4..start]))
    else {
        return None;
    };
    let _ = len;
    let int = |k: &str| safe_int(h.get(k));
    let last = h.get("last").and_then(Value::as_bool);
    let header = match h.get("t").and_then(Value::as_str)? {
        "data" => BinaryHeader::Data { ch: int("ch")? },
        "chunk" => BinaryHeader::Chunk {
            req: int("req")?,
            last: last?,
        },
        "put" => BinaryHeader::Put {
            req: int("req")?,
            ch: int("ch")?,
            path: h.get("path")?.as_str()?.to_string(),
            last: last?,
        },
        _ => return None,
    };
    Some((header, start))
}

/// The header's length and where the payload starts, when the frame holds both.
fn header_bounds(frame: &[u8]) -> Option<(usize, usize)> {
    if frame.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    (4 + len <= frame.len()).then_some((len, 4 + len))
}

/// `encodeBinary`.
pub fn encode_binary(header: &Value, payload: &[u8]) -> Vec<u8> {
    let head = js::stringify(header);
    let mut out = Vec::with_capacity(4 + head.len() + payload.len());
    out.extend_from_slice(&(head.len() as u32).to_be_bytes());
    out.extend_from_slice(head.as_bytes());
    out.extend_from_slice(payload);
    out
}

/// `textFrameType`: a frame's `t`, or why it has none — for a log line said once per type.
pub fn text_frame_type(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => m
            .get("t")
            .and_then(Value::as_str)
            .map_or("(no type)".into(), str::to_string),
        Ok(_) => "(not a JSON object)".into(),
        Err(_) => "(not JSON)".into(),
    }
}

/// `binaryFrameType`.
pub fn binary_frame_type(frame: &[u8]) -> String {
    if frame.len() < 4 {
        return "(too short)".into();
    }
    match header_bounds(frame) {
        None => "(header past the end)".into(),
        Some((_, start)) => text_frame_type(&String::from_utf8_lossy(&frame[4..start])),
    }
}

/// A file as the hub reports it.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteStat {
    File {
        path: String,
        size: u64,
        mtime_ms: f64,
    },
    Dir {
        path: String,
    },
    Missing {
        path: String,
    },
}

impl RemoteStat {
    pub fn json(&self) -> Value {
        match self {
            RemoteStat::File {
                path,
                size,
                mtime_ms,
            } => {
                json!({ "path": path, "kind": "file", "size": size, "mtimeMs": js::num(*mtime_ms) })
            }
            RemoteStat::Dir { path } => json!({ "path": path, "kind": "dir" }),
            RemoteStat::Missing { path } => json!({ "path": path, "kind": "missing" }),
        }
    }
}

/// `{ t: "open", ch, roots, kind, synced? }`.
pub fn open_message(ch: i64, roots: &[Root], kind: Kind, synced: bool) -> String {
    let mut m = Map::new();
    m.insert("t".into(), json!("open"));
    m.insert("ch".into(), json!(ch));
    m.insert(
        "roots".into(),
        serde_json::to_value(roots).unwrap_or_default(),
    );
    m.insert("kind".into(), json!(kind.as_str()));
    if synced {
        m.insert("synced".into(), json!(true));
    }
    js::stringify(&Value::Object(m))
}

#[cfg(test)]
mod tests;
