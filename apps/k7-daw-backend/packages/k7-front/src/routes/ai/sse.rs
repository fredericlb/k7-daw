//! sse.rs — a streamed Messages answer read as `@anthropic-ai/sdk` 0.128 reads it: its line and SSE decoders,
//! the events it dispatches by name, and `MessageStream`'s accumulation of one message from them.
//! What a reader gets wrong: a block's fields keep the order the server sent them in, since the whole turn goes
//! back to the model on a repair and that request must be byte for byte the SDK's (`docs/files.md`).

use serde_json::{Map, Value};

use super::claude::Failure;
use crate::js::truthy;

/// The SSE `event:` names the SDK parses and yields; any other is dropped, `error` aside.
const YIELDED: &[&str] = &[
    "completion",
    "message_start",
    "message_delta",
    "message_stop",
    "content_block_start",
    "content_block_delta",
    "content_block_stop",
    "message",
];

/// `LineDecoder`: bytes into lines on `\n`, `\r\n` or a lone `\r`, each decoded as lossy UTF-8.
#[derive(Default)]
pub struct Lines {
    buf: Vec<u8>,
}

impl Lines {
    /// The lines a chunk completes; a `\r` at its very end waits for the next byte.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < self.buf.len() {
            match self.buf[i] {
                b'\n' => {
                    out.push(String::from_utf8_lossy(&self.buf[start..i]).into_owned());
                    i += 1;
                    start = i;
                }
                b'\r' if i + 1 == self.buf.len() => break,
                b'\r' => {
                    out.push(String::from_utf8_lossy(&self.buf[start..i]).into_owned());
                    i += if self.buf[i + 1] == b'\n' { 2 } else { 1 };
                    start = i;
                }
                _ => i += 1,
            }
        }
        self.buf.drain(..start);
        out
    }

    /// What is left once the body ends, as `flush` decodes it: a last line.
    pub fn flush(&mut self) -> Vec<String> {
        if self.buf.is_empty() {
            return Vec::new();
        }
        if self.buf.last() == Some(&b'\r') {
            self.buf.pop();
        }
        let line = String::from_utf8_lossy(&self.buf).into_owned();
        self.buf.clear();
        vec![line]
    }
}

/// One server-sent event: its name and its data lines joined.
#[derive(Debug, PartialEq)]
pub struct Event {
    pub event: Option<String>,
    pub data: String,
}

/// `SSEDecoder`: fields gathered until a blank line.
#[derive(Default)]
pub struct Decoder {
    event: Option<String>,
    data: Vec<String>,
}

impl Decoder {
    pub fn line(&mut self, line: &str) -> Option<Event> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if self.event.as_deref().is_none_or(str::is_empty) && self.data.is_empty() {
                return None;
            }
            return Some(Event {
                event: self.event.take(),
                data: std::mem::take(&mut self.data).join("\n"),
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => self.data.push(value.to_string()),
            _ => {}
        }
        None
    }
}

/// `APIError.makeMessage`: the status, then the body's `message`, the body itself, or the text.
pub fn make_message(status: Option<u16>, error: Option<&Value>, message: Option<&str>) -> String {
    let msg = match error {
        Some(e) if e.get("message").is_some_and(truthy) => match &e["message"] {
            Value::String(s) => Some(s.clone()),
            m => Some(m.to_string()),
        },
        Some(e) if truthy(e) => Some(e.to_string()),
        _ => message.map(str::to_string),
    }
    .filter(|m| !m.is_empty());
    match (status, msg) {
        (Some(s), Some(m)) => format!("{s} {m}"),
        (Some(s), None) => format!("{s} status code (no body)"),
        (None, Some(m)) => m,
        (None, None) => "(no status code or body)".into(),
    }
}

/// `MessageStream`'s snapshot of the message being streamed, and the one `message_stop` sealed.
#[derive(Default)]
pub struct Message {
    snapshot: Option<Value>,
    received: Option<Value>,
}

impl Message {
    /// One event: the text it adds to a text block, when it does (the SDK's `text` event).
    pub fn event(&mut self, ev: &Event) -> Result<Option<String>, Failure> {
        let Some(name) = ev.event.as_deref() else {
            return Ok(None);
        };
        if name == "error" {
            let body = serde_json::from_str::<Value>(&ev.data)
                .unwrap_or_else(|_| Value::String(ev.data.clone()));
            return Err(Failure::Api(make_message(None, Some(&body), None)));
        }
        if !YIELDED.contains(&name) {
            return Ok(None);
        }
        let event: Value =
            serde_json::from_str(&ev.data).map_err(|e| Failure::Other(e.to_string()))?;
        self.accumulate(&event)
    }

    fn accumulate(&mut self, event: &Value) -> Result<Option<String>, Failure> {
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("undefined");
        if kind == "message_start" {
            if self.snapshot.is_some() {
                return Err(Failure::Other(format!(
                    "Unexpected event order, got {kind} before receiving \"message_stop\""
                )));
            }
            self.snapshot = Some(event.get("message").cloned().unwrap_or(Value::Null));
            return Ok(None);
        }
        let Some(snapshot) = self.snapshot.as_mut() else {
            return Err(Failure::Other(format!(
                "Unexpected event order, got {kind} before \"message_start\""
            )));
        };
        match kind {
            "message_stop" => self.received = Some(snapshot.clone()),
            "message_delta" => {
                let delta = event.get("delta");
                if let Some(o) = snapshot.as_object_mut() {
                    for k in ["stop_reason", "stop_sequence", "stop_details"] {
                        let v = delta.and_then(|d| d.get(k)).cloned().unwrap_or(Value::Null);
                        o.insert(k.into(), v);
                    }
                }
            }
            "content_block_start" => {
                let block = event.get("content_block").cloned().unwrap_or(Value::Null);
                if let Some(c) = snapshot.get_mut("content").and_then(Value::as_array_mut) {
                    c.push(block);
                }
            }
            "content_block_delta" => return Ok(delta(snapshot, event)),
            _ => {}
        }
        Ok(None)
    }

    /// `finalMessage()` once the body ended: the sealed message, or why there is none.
    pub fn finish(self) -> Result<Value, Failure> {
        if self.snapshot.is_none() {
            return Err(Failure::Other(
                "request ended without sending any chunks".into(),
            ));
        }
        self.received.ok_or_else(|| {
            Failure::Other("stream ended without producing a Message with role=assistant".into())
        })
    }
}

/// A block's string field, or `""`.
fn text_of(block: &Map<String, Value>, key: &str) -> String {
    block
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// One `content_block_delta` folded into its block; the text it adds when the last block is text.
fn delta(snapshot: &mut Value, event: &Value) -> Option<String> {
    let d = event.get("delta")?;
    let kind = d.get("type").and_then(Value::as_str).unwrap_or_default();
    let content = snapshot.get_mut("content").and_then(Value::as_array_mut)?;
    let index = event
        .get("index")
        .and_then(Value::as_u64)
        .map(|i| i as usize);
    let block = index
        .and_then(|i| content.get_mut(i))
        .and_then(Value::as_object_mut);
    if let Some(b) = block {
        let is = |t: &str| b.get("type").and_then(Value::as_str) == Some(t);
        match kind {
            "text_delta" if is("text") => {
                let add = d.get("text").and_then(Value::as_str).unwrap_or_default();
                let text = text_of(b, "text") + add;
                b.insert("text".into(), Value::String(text));
            }
            "citations_delta" if is("text") => {
                let mut all = b
                    .get("citations")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                all.push(d.get("citation").cloned().unwrap_or(Value::Null));
                b.insert("citations".into(), Value::Array(all));
            }
            "thinking_delta" if is("thinking") => {
                let add = d
                    .get("thinking")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let thinking = text_of(b, "thinking") + add;
                b.insert("thinking".into(), Value::String(thinking));
            }
            "signature_delta" if is("thinking") => {
                let sig = d.get("signature").cloned().unwrap_or(Value::Null);
                b.insert("signature".into(), sig);
            }
            _ => {}
        }
    }
    let last_is_text = content
        .last()
        .and_then(|b| b.get("type"))
        .and_then(Value::as_str)
        == Some("text");
    (kind == "text_delta" && last_is_text).then(|| {
        d.get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_on_every_ending_and_wait_for_a_last_cr() {
        let mut l = Lines::default();
        assert_eq!(l.push(b"a\nb\r\nc\rd\r"), vec!["a", "b", "c"]);
        assert_eq!(l.push(b"\ne"), vec!["d"]);
        assert_eq!(l.flush(), vec!["e"]);
    }

    #[test]
    fn an_event_needs_its_blank_line() {
        let mut d = Decoder::default();
        assert_eq!(d.line("event: ping"), None);
        assert_eq!(d.line("data: {}"), None);
        assert_eq!(d.line(": comment"), None);
        assert_eq!(d.line("data:x"), None);
        let ev = d.line("").unwrap();
        assert_eq!(ev.event.as_deref(), Some("ping"));
        assert_eq!(ev.data, "{}\nx");
        assert_eq!(d.line(""), None);
    }

    #[test]
    fn messages_as_the_sdk_spells_them() {
        let body = serde_json::json!({"type": "error", "error": {"type": "x", "message": "m"}});
        assert_eq!(
            make_message(Some(400), Some(&body), None),
            format!("400 {body}")
        );
        assert_eq!(
            make_message(Some(500), None, Some("")),
            "500 status code (no body)"
        );
        let text = Value::String("down".into());
        assert_eq!(make_message(None, Some(&text), None), "\"down\"");
        let inner = serde_json::json!({"message": "busy"});
        assert_eq!(make_message(None, Some(&inner), None), "busy");
    }
}
