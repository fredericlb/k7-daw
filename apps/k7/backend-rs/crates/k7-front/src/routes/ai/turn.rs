//! turn.rs — `POST /api/ai/organize/turn`: ONE model turn on the account's key and model, streamed to the browser
//! as NDJSON with a heartbeat. The browser owns the conversation and the checks (`panel-rules`' `aiLoop.ts`); this
//! side owns what must not reach a page: the key, the model, the output limits. Choices: `apps/k7/frontend/docs/aiOrganize.md`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::Response;
use hyper::StatusCode;
use hyper::body::Bytes;
use serde_json::{Map, Value, json};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::claude::{Claude, Failure};
use super::{AI_BODY_LIMIT, DEFAULT_AI_MODEL, read_for_update, sub_of};
use crate::AppState;
use crate::js::{truthy, utf16_len};
use crate::session::{signed, with_cookies};
use crate::web::{error, read_json_within};

/// The heartbeat's period: the last status again, for an idle-timing proxy.
pub const HEARTBEAT: Duration = Duration::from_secs(15);
/// A conversation is a request, an answer and a repair: five messages at most.
const MAX_MESSAGES: usize = 5;
/// The constant prompt is some 30 KB; a body claiming more is not the browser's.
const MAX_SYSTEM: usize = 200_000;

/// One turn as the browser asks for it.
#[derive(Debug, Clone)]
pub struct Turn {
    pub system: String,
    pub schema: Value,
    pub messages: Vec<Value>,
}

/// A block of the kinds the loop sends (text, images, and the model's own thinking handed back); no document, no tool result.
fn plain_block(b: &Value) -> bool {
    let kinds = ["text", "image", "thinking", "redacted_thinking"];
    matches!(b.get("type").and_then(Value::as_str), Some(k) if kinds.contains(&k))
}

/// The body checked: a constant prompt, a schema, and a conversation that starts and ends with the user.
pub fn parse_turn(body: &Value) -> Result<Turn, String> {
    let system = match body.get("system") {
        Some(Value::String(s)) if !s.is_empty() && s.len() <= MAX_SYSTEM => s.clone(),
        _ => {
            return Err(format!(
                "system: expected a text of 1 to {MAX_SYSTEM} bytes"
            ));
        }
    };
    let schema = match body.get("schema") {
        Some(s @ Value::Object(_)) => s.clone(),
        _ => return Err("schema: expected an object".into()),
    };
    let messages = match body.get("messages") {
        Some(Value::Array(m)) if (1..=MAX_MESSAGES).contains(&m.len()) => m.clone(),
        _ => return Err(format!("messages: expected 1 to {MAX_MESSAGES} messages")),
    };
    for (i, m) in messages.iter().enumerate() {
        let role = m.get("role").and_then(Value::as_str);
        let want = if i % 2 == 0 { "user" } else { "assistant" };
        if role != Some(want) {
            return Err(format!("messages[{i}]: expected the role {want}"));
        }
        let ok = match m.get("content") {
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(a)) => !a.is_empty() && a.iter().all(plain_block),
            _ => false,
        };
        if !ok {
            return Err(format!(
                "messages[{i}]: expected a text or a list of blocks"
            ));
        }
    }
    if messages.len() % 2 == 0 {
        return Err("messages: the last one is the user's".into());
    }
    // Only the keys the API reads: nothing else of the browser's body reaches it.
    let messages = messages
        .iter()
        .map(|m| json!({ "role": m["role"], "content": m["content"] }))
        .collect();
    Ok(Turn {
        system,
        schema,
        messages,
    })
}

/// POST `/organize/turn`: the body parsed ahead of every guard, then the session, the key, the shape, the stream.
pub async fn post_turn(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let session = signed(req.headers(), &state.settings, &state.revocations);
    let body = match read_json_within(req, AI_BODY_LIMIT).await {
        Ok((_, v)) => v,
        Err(r) => return r,
    };
    let Some(signed) = session else {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    };
    let stored = sub_of(&signed).and_then(|sub| read_for_update(&state, &sub).ok().flatten());
    let field = |k: &str| {
        stored
            .as_ref()
            .and_then(|s| s.get(k))
            .filter(|v| truthy(v))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let Some(key) = field("apiKey") else {
        let message = "no Claude key for this account — set one in the settings panel";
        return with_cookies(error(StatusCode::NOT_FOUND, message), &signed);
    };
    let model = field("model").unwrap_or_else(|| DEFAULT_AI_MODEL.into());
    let base = state.settings.anthropic_base_url.clone();
    let turn = match parse_turn(&body) {
        Ok(t) => t,
        Err(m) => return with_cookies(error(StatusCode::BAD_REQUEST, &m), &signed),
    };
    with_cookies(answer(&base, key, model, turn, HEARTBEAT), &signed)
}

/// The NDJSON stream of one turn; a closed tab drops the request to Claude.
pub fn answer(base: &str, key: String, model: String, turn: Turn, heartbeat: Duration) -> Response {
    let (tx, mut rx) = unbounded_channel::<Bytes>();
    tokio::spawn(run(Claude::new(base), key, model, turn, tx, heartbeat));
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

/// What the browser is told, one JSON line at a time; the last status is kept for the heartbeat.
#[derive(Clone)]
struct Say {
    tx: UnboundedSender<Bytes>,
    last: Arc<Mutex<Option<Bytes>>>,
}

impl Say {
    fn line(&self, v: Value) -> Bytes {
        let mut text = v.to_string();
        text.push('\n');
        let b = Bytes::from(text);
        let _ = self.tx.send(b.clone());
        b
    }
    fn status(&self, text: &str) {
        let b = self.line(json!({ "type": "status", "text": text }));
        if let Ok(mut last) = self.last.lock() {
            *last = Some(b);
        }
    }
    fn error(&self, message: &str) {
        self.line(json!({ "type": "error", "error": message }));
    }
    /// The status again, or a ping while there is none.
    fn beat(&self) {
        let last = self.last.lock().ok().and_then(|l| l.clone());
        match last {
            Some(b) => {
                let _ = self.tx.send(b);
            }
            None => {
                self.line(json!({ "type": "ping" }));
            }
        }
    }
}

/// The turn with the heartbeat beside it.
async fn run(
    claude: Claude,
    key: String,
    model: String,
    turn: Turn,
    tx: UnboundedSender<Bytes>,
    heartbeat: Duration,
) {
    let say = Say {
        tx: tx.clone(),
        last: Arc::default(),
    };
    let work = one_turn(&claude, &key, &model, &turn, &say);
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

/// Ask once; the answer's text and content blocks, or why there is no text.
async fn one_turn(claude: &Claude, key: &str, model: &str, turn: &Turn, say: &Say) {
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
    let msg = match claude.stream(key, body, &mut on_text).await {
        Ok(m) => m,
        Err(f) => return say.error(&f.message()),
    };
    match answer_text(&msg) {
        Ok(text) => {
            let content = msg.get("content").cloned().unwrap_or(Value::Null);
            say.line(json!({ "type": "turn", "text": text, "content": content }));
        }
        Err(f) => say.error(&f.message()),
    }
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

    fn good() -> Value {
        json!({
            "system": "You design panels.",
            "schema": { "type": "object" },
            "messages": [
                { "role": "user", "content": "go" },
                { "role": "assistant", "content": [{ "type": "text", "text": "{}" }] },
                { "role": "user", "content": "again" },
            ],
        })
    }

    #[test]
    fn a_turn_is_a_prompt_a_schema_and_a_conversation_ending_with_the_user() {
        assert_eq!(parse_turn(&good()).unwrap().messages.len(), 3);
        let bad = |f: &dyn Fn(&mut Value)| {
            let mut v = good();
            f(&mut v);
            parse_turn(&v).unwrap_err()
        };
        assert!(bad(&|v| v["system"] = json!("")).starts_with("system"));
        assert!(bad(&|v| v["system"] = json!(5)).starts_with("system"));
        assert!(bad(&|v| v["system"] = json!("x".repeat(MAX_SYSTEM + 1))).starts_with("system"));
        assert!(bad(&|v| v["schema"] = json!("x")).starts_with("schema"));
        for kind in ["document", "tool_result", "tool_use", "server_tool_use"] {
            let set = |v: &mut Value| v["messages"][0]["content"] = json!([{ "type": kind }]);
            assert!(bad(&set).starts_with("messages[0]"), "{kind}");
        }
        let mut v = good();
        v["messages"][0]["content"] = json!([{ "type": "text", "text": "a" }, { "type": "image" }]);
        assert!(parse_turn(&v).is_ok());
        assert!(bad(&|v| v["messages"] = json!([])).starts_with("messages"));
        assert!(bad(&|v| v["messages"] = json!("x")).starts_with("messages"));
        assert!(
            bad(&|v| v["messages"] = json!(vec![json!({"role":"user","content":"a"}); 6]))
                .starts_with("messages")
        );
        assert!(bad(&|v| v["messages"][0]["role"] = json!("assistant")).contains("messages[0]"));
        assert!(bad(&|v| v["messages"][1]["role"] = json!("user")).contains("messages[1]"));
        assert!(bad(&|v| v["messages"][2]["content"] = json!(7)).contains("messages[2]"));
        assert!(bad(&|v| v["messages"][1]["content"] = json!([1])).contains("messages[1]"));
        assert!(bad(&|v| v["messages"][1]["content"] = json!([])).contains("messages[1]"));
        assert!(
            bad(&|v| {
                v["messages"].as_array_mut().unwrap().pop();
            })
            .contains("the last one")
        );
        assert!(parse_turn(&json!([])).is_err());
        assert!(parse_turn(&Value::Null).is_err());
    }

    #[test]
    fn only_the_keys_the_api_reads_go_up() {
        let mut v = good();
        v["messages"][0]["cache_control"] = json!({ "type": "ephemeral" });
        v["model"] = json!("claude-evil");
        let t = parse_turn(&v).unwrap();
        assert_eq!(t.messages[0], json!({ "role": "user", "content": "go" }));
        assert_eq!(request_body("claude-opus-5", &t)["model"], "claude-opus-5");
    }
}
