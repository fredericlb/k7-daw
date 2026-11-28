//! protocol — the wire contract with the app that runs us: NDJSON, a whole SNAPSHOT per `state` frame and
//! only when its bytes changed, a `result` per command, and commands back with one string argument. The
//! wire's one definition is `apps/k7-daw-backend/packages/k7-supervisor/schema/hostProtocol.schema.json`; the snapshot
//! is held to its fixtures byte for byte (`tests/all/protocol.rs`).

mod snapshot;
pub mod wire;

pub use snapshot::*;

use serde_json::{Map, Value, json};

/// What joins a command's parts: U+001F, which no device name, session id or plugin id contains.
pub const UNIT_SEP: char = '\u{1f}';

/// One command from the app; `id` is echoed on its result.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    pub id: i64,
    pub cmd: String,
    pub arg: String,
}

/// The `result` frame answering command `id`; `message` absent when empty.
pub fn result_frame(id: i64, ok: bool, message: &str) -> Value {
    let mut f = Map::new();
    f.insert("t".into(), json!("result"));
    f.insert("id".into(), json!(id));
    f.insert("ok".into(), json!(ok));
    if !message.is_empty() {
        f.insert("message".into(), json!(message));
    }
    Value::Object(f)
}

/// A line off the command pipe as a command; None for a blank, unparseable or foreign line.
pub fn parse_command(line: &str) -> Option<Command> {
    if line.trim().is_empty() {
        return None;
    }
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        // A malformed line is the app's bug, not a reason to die: said, and the channel kept open.
        crate::log::stderr_only(&format!("[host] ignoring unparseable command: {line}"));
        return None;
    };
    if msg.get("t").and_then(Value::as_str) != Some("cmd") {
        return None;
    }
    Some(Command {
        cmd: msg.get("cmd")?.as_str()?.to_string(),
        id: msg
            .get("id")
            .and_then(Value::as_f64)
            .map(|n| n as i64)
            .unwrap_or(0),
        arg: msg
            .get("arg")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

/// `s` split on U+001F, empty parts dropped.
pub fn split_units(s: &str) -> Vec<String> {
    s.split(UNIT_SEP)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}
