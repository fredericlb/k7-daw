//! wire.rs — the agent's reading of the hub: the hub's text frames typed, the tunnel's URL and its pacing
//! constants. The frames' bytes are `k7-engine`'s own (`tunnel/frames`), the hub's code reused, so the two ends
//! cannot drift; the shapes the hub only SENDS are read here, leniently, as the TypeScript agent reads them.

use std::time::Duration;

use serde_json::Value;

pub use k7_engine::tunnel::frames::{
    BinaryHeader, CHUNK_BYTES, TUNNEL_PATH, TUNNEL_PROTOCOL_HEADER, TUNNEL_SECRET_HEADER,
    TUNNEL_TOKEN_HEADER, binary_frame_type, decode_binary, encode_binary, text_frame_type,
};

/// Past this many unsent bytes an upload waits: small, since every other channel's frames queue behind it.
pub const HIGH_WATER_BYTES: usize = 1024 * 1024;
/// How long a channel's close waits for the frames queued ahead of it to be delivered.
pub const CLOSE_DRAIN: Duration = Duration::from_secs(3);

/// A root the server shares with a channel, as `open` carries it.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenRoot {
    pub remote: String,
    pub name: String,
    pub writable: bool,
}

/// What the hub tells the agent, as text.
#[derive(Debug, Clone, PartialEq)]
pub enum HubMessage {
    Open {
        ch: i64,
        roots: Vec<OpenRoot>,
        jobs: bool,
        synced: bool,
    },
    Close {
        ch: i64,
    },
    Data {
        ch: i64,
        text: String,
    },
    StatReply {
        req: i64,
        entries: Value,
    },
    PutReply {
        req: i64,
        stat: Value,
        error: String,
    },
    GetError {
        req: i64,
        error: String,
    },
}

/// A frame that is no frame this agent reads, by the type it says to log.
pub fn parse_hub_message(text: &str) -> Result<HubMessage, String> {
    let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
        return Err(text_frame_type(text));
    };
    let int = |k: &str| m.get(k).and_then(Value::as_i64).unwrap_or(0);
    let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let t = m.get("t").and_then(Value::as_str).unwrap_or("");
    Ok(match t {
        "open" => HubMessage::Open {
            ch: int("ch"),
            roots: m
                .get("roots")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|r| OpenRoot {
                    remote: r["remote"].as_str().unwrap_or("").to_string(),
                    name: r["name"].as_str().unwrap_or("").to_string(),
                    writable: r["writable"].as_bool().unwrap_or(false),
                })
                .collect(),
            jobs: m.get("kind").and_then(Value::as_str) == Some("jobs"),
            synced: m.get("synced").and_then(Value::as_bool).unwrap_or(false),
        },
        "close" => HubMessage::Close { ch: int("ch") },
        "data" => HubMessage::Data {
            ch: int("ch"),
            text: s("text"),
        },
        "stat-reply" => HubMessage::StatReply {
            req: int("req"),
            entries: m.get("entries").cloned().unwrap_or(Value::Null),
        },
        "put-reply" => HubMessage::PutReply {
            req: int("req"),
            stat: m.get("stat").cloned().unwrap_or(Value::Null),
            error: s("error"),
        },
        "get-error" => HubMessage::GetError {
            req: int("req"),
            error: s("error"),
        },
        other => return Err(other.to_string()),
    })
}

/// `https://h/x` → `wss://h/x/api/engine-tunnel?id=…`; None for an address that is no web address.
pub fn tunnel_url(registry_url: &str, id: &str) -> Option<String> {
    let u = crate::urls::parse(registry_url)?;
    let host = crate::urls::host_of(&u);
    let path = u.path().trim_end_matches('/');
    let scheme = if u.scheme() == "https" { "wss" } else { "ws" };
    let mut enc = String::new();
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            enc.push(b as char);
        } else {
            enc.push_str(&format!("%{b:02X}"));
        }
    }
    Some(format!("{scheme}://{host}{path}{TUNNEL_PATH}?id={enc}"))
}
