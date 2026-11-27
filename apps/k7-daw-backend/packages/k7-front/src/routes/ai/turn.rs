//! turn.rs — ONE model turn on the account's key and model, and the NDJSON stream a route answers it through, with a heartbeat.
//! `organize.rs` runs the conversation and every check; this side owns what must not reach a page: the key, the model, the output limits.
//! Choices: `apps/k7-daw-frontend/docs/aiOrganize.md`.

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::response::Response;
use hyper::StatusCode;
use hyper::body::Bytes;
use serde_json::{Map, Value, json};
use song::modules::ui::ai_answer::AiEvent;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::claude::{Claude, Failure};
use crate::js::utf16_len;
use crate::web::error;

/// The heartbeat's period: the last status again, for an idle-timing proxy.
pub const HEARTBEAT: Duration = Duration::from_secs(15);

/// One turn: the constant prompt, the answer's schema and the conversation so far.
#[derive(Debug, Clone)]
pub struct Turn {
    pub system: String,
    pub schema: Value,
    pub messages: Vec<Value>,
}

/// What the browser is told, one JSON line at a time; the last status is kept for the heartbeat.
#[derive(Clone)]
pub struct Say {
    tx: UnboundedSender<Bytes>,
    last: Arc<Mutex<Option<Bytes>>>,
}

impl Say {
    /// One event as a line.
    pub fn line(&self, event: &AiEvent) -> Bytes {
        let mut text = serde_json::to_string(event).unwrap_or_default();
        text.push('\n');
        let b = Bytes::from(text);
        let _ = self.tx.send(b.clone());
        b
    }

    /// Progress; the heartbeat repeats the last one.
    pub fn status(&self, text: &str) {
        let b = self.line(&AiEvent::Status { text: text.into() });
        if let Ok(mut last) = self.last.lock() {
            *last = Some(b);
        }
    }

    pub fn error(&self, message: &str) {
        self.line(&AiEvent::Error {
            error: message.into(),
        });
    }

    /// The status again, or a ping while there is none.
    fn beat(&self) {
        let last = self.last.lock().ok().and_then(|l| l.clone());
        match last {
            Some(b) => {
                let _ = self.tx.send(b);
            }
            None => {
                self.line(&AiEvent::Ping);
            }
        }
    }
}

/// The NDJSON stream of one piece of work; a closed tab drops the work, and the request to Claude with it.
pub fn stream<F, Fut>(heartbeat: Duration, work: F) -> Response
where
    F: FnOnce(Say) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let (tx, mut rx) = unbounded_channel::<Bytes>();
    tokio::spawn(run(work, tx, heartbeat));
    let stream = futures_util::stream::poll_fn(move |cx| {
        rx.poll_recv(cx)
            .map(|b| b.map(Ok::<_, std::convert::Infallible>))
    });
    Response::builder()
        .header("content-type", "application/x-ndjson")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "no stream"))
}

/// The work with the heartbeat beside it.
async fn run<F, Fut>(work: F, tx: UnboundedSender<Bytes>, heartbeat: Duration)
where
    F: FnOnce(Say) -> Fut,
    Fut: Future<Output = ()>,
{
    let say = Say {
        tx: tx.clone(),
        last: Arc::default(),
    };
    let work = work(say.clone());
    tokio::pin!(work);
    let mut beat = tokio::time::interval_at(tokio::time::Instant::now() + heartbeat, heartbeat);
    loop {
        tokio::select! {
            _ = &mut work => return,
            _ = tx.closed() => return,
            _ = beat.tick() => say.beat(),
        }
    }
}

/// Ask once: the answer's text and content blocks, or why there is no text.
pub async fn ask(
    claude: &Claude,
    key: &str,
    model: &str,
    turn: &Turn,
    say: &Say,
) -> Result<(String, Value), String> {
    let body = Bytes::from(request_body(model, turn).to_string());
    let (mut chars, mut last) = (0usize, 0usize);
    let mut on_text = |delta: &str| {
        chars += utf16_len(delta);
        if chars - last < 2000 {
            return;
        }
        last = chars;
        let k = (chars as f64 / 1000.0).round();
        say.status(&format!("Writing… {k}k characters"));
    };
    let msg = claude
        .stream(key, body, &mut on_text)
        .await
        .map_err(|f| f.message())?;
    let text = answer_text(&msg).map_err(|f| f.message())?;
    let content = msg.get("content").cloned().unwrap_or(Value::Null);
    Ok((text, content))
}

/// The body `messages.stream` sends for one turn, its keys in the SDK's order.
pub fn request_body(model: &str, turn: &Turn) -> Value {
    let mut output = Map::new();
    // Haiku takes no effort setting; every other current model thinks harder for `high`.
    if !model.contains("haiku") {
        output.insert("effort".into(), "high".into());
    }
    output.insert(
        "format".into(),
        json!({ "type": "json_schema", "schema": turn.schema }),
    );
    json!({
        "model": model,
        "max_tokens": 64000,
        "system": [{ "type": "text", "text": turn.system, "cache_control": { "type": "ephemeral" } }],
        "messages": turn.messages,
        "output_config": output,
        "stream": true,
    })
}

/// The turn's text, or why there is none — a refusal and a cut-off say different things.
fn answer_text(msg: &Value) -> Result<String, Failure> {
    match msg.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => {
            let why = msg
                .get("stop_details")
                .and_then(|d| d.get("explanation"))
                .and_then(Value::as_str)
                .filter(|e| !e.is_empty());
            let why = why.map(|e| format!(": {e}")).unwrap_or_default();
            return Err(Failure::Other(format!("Claude declined{why}")));
        }
        Some("max_tokens") => {
            return Err(Failure::Other(
                "the answer ran past the output limit".into(),
            ));
        }
        _ => {}
    }
    let blocks = msg.get("content").and_then(Value::as_array);
    Ok(blocks
        .into_iter()
        .flatten()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_only_what_the_account_and_the_server_decide() {
        let turn = Turn {
            system: "S".into(),
            schema: json!({ "type": "object" }),
            messages: vec![json!({ "role": "user", "content": "go" })],
        };
        let body = request_body("claude-opus-5", &turn);
        assert_eq!(body["model"], "claude-opus-5");
        assert_eq!(body["max_tokens"], 64000);
        assert_eq!(body["output_config"]["effort"], "high");
        assert!(request_body("claude-haiku-4-5", &turn)["output_config"]["effort"].is_null());
    }

    #[test]
    fn a_refusal_and_a_cut_off_say_different_things() {
        let refused = json!({ "stop_reason": "refusal", "stop_details": { "explanation": "no" } });
        assert_eq!(
            answer_text(&refused).unwrap_err().message(),
            "Claude declined: no"
        );
        let cut = json!({ "stop_reason": "max_tokens" });
        assert_eq!(
            answer_text(&cut).unwrap_err().message(),
            "the answer ran past the output limit"
        );
        let said = json!({ "content": [{ "type": "text", "text": "a" }, { "type": "thinking" }, { "type": "text", "text": "b" }] });
        assert_eq!(answer_text(&said).unwrap(), "ab");
    }
}
