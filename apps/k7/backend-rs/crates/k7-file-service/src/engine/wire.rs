//! The wire of one session's socket: the frames out (stamped with the session), the frames in (kept to this session,
//! decoded as the clients decode them) and the outbox a down socket holds. Pure functions over JSON, no socket.
//! What a reader gets wrong: a frame that names ANOTHER session is dropped, but one naming none is everyone's.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{OUTBOX_MAX, OUTBOX_TTL_MS, ScanStatus};

/// `forSession`: a frame is read unless it carries a session that is not `mine`.
pub fn for_session(frame: &Value, mine: &str) -> bool {
    match frame.get("session") {
        Some(Value::String(s)) => s == mine,
        _ => true,
    }
}

/// The session stamped on a frame, first, and omitted for the bare session as the clients omit it.
pub fn stamp(session: &str, frame: Value) -> Value {
    let Value::Object(body) = frame else {
        return frame;
    };
    if session.is_empty() {
        return Value::Object(body);
    }
    let mut out = serde_json::Map::with_capacity(body.len() + 1);
    out.insert("session".into(), Value::String(session.to_string()));
    out.extend(body);
    Value::Object(out)
}

/// `{type: "project", doc}`, the document folded as the engine plays it (`withGroupMix`).
pub fn project(doc: Value) -> Value {
    json!({ "type": "project", "doc": doc })
}

pub fn transport(action: &str) -> Value {
    json!({ "type": "transport", "action": action })
}

pub fn record_enable(on: bool) -> Value {
    json!({ "type": "record-enable", "enabled": on })
}

pub fn flush() -> Value {
    json!({ "type": "flush" })
}

pub fn list_sessions() -> Value {
    json!({ "type": "list-sessions" })
}

pub fn plugin_scan() -> Value {
    json!({ "type": "plugin-scan" })
}

pub fn list_plugins() -> Value {
    json!({ "type": "list-plugins" })
}

pub fn drop_session(session: &str) -> Value {
    json!({ "type": "drop-session", "session": session })
}

/// One row of a `sessions` answer.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRow {
    pub id: String,
    pub loaded: bool,
}

/// A recorded take, as `recorded` carries it: a bare file name is the engine's, a path is refused later.
#[derive(Debug, Clone, PartialEq)]
pub struct Take {
    pub track: String,
    pub file: String,
    pub start: f64,
    pub length: f64,
}

/// A frame read, as much as the link keeps of it.
#[derive(Debug, Clone, PartialEq)]
pub enum In {
    State {
        playing: bool,
        record_enabled: bool,
    },
    Scan(ScanStatus),
    Sessions(Vec<SessionRow>),
    Plugins(Value),
    Recorded(Take),
    Error(String),
    /// Any other type: the stand-in forwards it only while something listens.
    Other(String, Value),
    /// Another session's, or not a frame at all.
    Ignored,
}

fn count(v: Option<&Value>) -> u64 {
    match v.and_then(Value::as_f64) {
        Some(n) if n >= 0.0 => n as u64,
        _ => 0,
    }
}

/// `decodePluginScan`: a `plugin-scan` frame read defensively.
pub fn decode_scan(m: &Value) -> Option<ScanStatus> {
    if m.get("type")?.as_str()? != "plugin-scan" {
        return None;
    }
    Some(ScanStatus {
        scanning: m.get("scanning").and_then(Value::as_bool) == Some(true),
        done: count(m.get("done")),
        total: count(m.get("total")),
        current: m
            .get("current")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        version: m
            .get("version")
            .and_then(Value::as_f64)
            .filter(|n| *n >= 0.0)
            .unwrap_or(0.0) as i64,
    })
}

fn string(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// A frame as text, for `session`'s socket.
pub fn decode_in(text: &str, mine: &str) -> In {
    let Ok(msg) = serde_json::from_str::<Value>(text) else {
        return In::Ignored;
    };
    if !msg.is_object() || !for_session(&msg, mine) {
        return In::Ignored;
    }
    let kind = msg
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match kind.as_str() {
        "state" => In::State {
            playing: truthy(msg.get("playing")),
            record_enabled: truthy(msg.get("recordEnabled")),
        },
        "plugin-scan" => decode_scan(&msg).map_or(In::Ignored, In::Scan),
        "sessions" => In::Sessions(
            msg.get("sessions")
                .and_then(Value::as_array)
                .map(|rows| {
                    rows.iter()
                        .map(|r| SessionRow {
                            id: string(r.get("id")),
                            loaded: truthy(r.get("loaded")),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        ),
        "plugins" => In::Plugins(msg.get("plugins").cloned().unwrap_or_else(|| json!([]))),
        "recorded" => In::Recorded(Take {
            track: string(msg.get("track")),
            file: string(msg.get("file")),
            start: msg.get("start").and_then(Value::as_f64).unwrap_or(0.0),
            length: msg.get("length").and_then(Value::as_f64).unwrap_or(0.0),
        }),
        "error" => In::Error(match msg.get("message") {
            None | Some(Value::Null) => "unknown error".to_string(),
            m => string(m),
        }),
        _ => In::Other(kind, msg),
    }
}

/// JavaScript's `!!` over a JSON value.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// What a socket holds while it is down: bounded, the oldest out, nothing replayed past its age.
pub struct Outbox {
    held: VecDeque<(Instant, Value)>,
    max: usize,
    ttl: Duration,
}

impl Default for Outbox {
    fn default() -> Self {
        Outbox::new(OUTBOX_MAX, Duration::from_millis(OUTBOX_TTL_MS))
    }
}

impl Outbox {
    pub fn new(max: usize, ttl: Duration) -> Outbox {
        Outbox {
            held: VecDeque::new(),
            max,
            ttl,
        }
    }

    pub fn push(&mut self, frame: Value, now: Instant) {
        self.held.push_back((now, frame));
        // The newest command is the one that still describes what the person wants.
        if self.held.len() > self.max {
            self.held.pop_front();
        }
    }

    /// Empty the queue, handing back what is still worth sending, oldest first.
    pub fn drain(&mut self, now: Instant) -> Vec<Value> {
        let ttl = self.ttl;
        std::mem::take(&mut self.held)
            .into_iter()
            .filter(|(at, _)| now.saturating_duration_since(*at) < ttl)
            .map(|(_, f)| f)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.held.len()
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}
