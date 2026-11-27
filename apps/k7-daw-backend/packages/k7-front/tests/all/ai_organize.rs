//! `POST /api/ai/organize` through a whole front: the guards in their order, the conversation (a proposal, a repair, a
//! refusal), and what reaches Claude (the account's key and model, nothing the browser chose). Claude is played by a fake.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{Front, Reply, front, send};

const SECRET_KEY: &str = "sk-ant-api03-secret-wxyz";

fn cookie() -> String {
    format!("Cookie: {}", crate::http_support::cookie())
}

/// What the fake Claude saw of each request: the key header and the body.
type Seen = Arc<Mutex<Vec<(String, Value)>>>;

/// One streamed Messages answer saying `text`, as the API frames it.
fn stream_of(text: &str) -> String {
    let events = [
        (
            "message_start",
            json!({ "type": "message_start", "message": { "id": "m", "type": "message", "role": "assistant", "model": "x", "content": [], "stop_reason": null, "usage": { "input_tokens": 1, "output_tokens": 1 } } }),
        ),
        (
            "content_block_start",
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
        ),
        (
            "content_block_delta",
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } }),
        ),
        (
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 }),
        ),
        (
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 2 } }),
        ),
        ("message_stop", json!({ "type": "message_stop" })),
    ];
    events
        .iter()
        .map(|(name, data)| format!("event: {name}\ndata: {data}\n\n"))
        .collect()
}

/// A Claude that answers its requests in turn with `(status, body)`, the last one for every request after, remembering what it was sent.
async fn claude(replies: Vec<(u16, String)>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let log = log.clone();
            let replies = replies.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let (head_end, len) = loop {
                    let n = sock.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        break (i + 4, len);
                    }
                };
                while buf.len() < head_end + len {
                    let n = sock.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
                let key = head
                    .lines()
                    .find_map(|l| l.strip_prefix("x-api-key: "))
                    .unwrap_or_default()
                    .to_string();
                let body_in: Value =
                    serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
                let nth = {
                    let mut log = log.lock().unwrap();
                    log.push((key, body_in));
                    log.len() - 1
                };
                let (status, body) = replies[nth.min(replies.len() - 1)].clone();
                let kind = if status == 200 {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let out = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(out.as_bytes()).await;
            });
        }
    });
    (url, seen)
}

async fn started(name: &str, base: &str) -> (Front, String) {
    let prefs = std::env::temp_dir().join(format!("k7-ai-organize-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prefs);
    let f = front(
        name,
        &[
            ("STUDIO_PREFS_DIR", prefs.to_str().unwrap()),
            ("ANTHROPIC_BASE_URL", base),
        ],
    )
    .await;
    (f, cookie())
}

async fn post(f: &Front, cookie: &str, body: &str) -> Reply {
    send(
        f.addr,
        "POST",
        "/api/ai/organize",
        &[
            ("Content-Type", "application/json"),
            ("Cookie", cookie.strip_prefix("Cookie: ").unwrap()),
        ],
        body,
    )
    .await
}

async fn keyed(f: &Front, cookie: &str, model: Option<&str>) {
    let body = json!({ "apiKey": SECRET_KEY, "model": model }).to_string();
    let r = send(
        f.addr,
        "PUT",
        "/api/ai/settings",
        &[
            ("Content-Type", "application/json"),
            ("Cookie", cookie.strip_prefix("Cookie: ").unwrap()),
        ],
        &body,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
}

/// A chunked body as its bytes, for a stream the front sends in pieces.
fn dechunk(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some((size, tail)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        if n == 0 {
            break;
        }
        out.push_str(&tail[..n]);
        rest = tail[n..].strip_prefix("\r\n").unwrap_or(&tail[n..]);
    }
    out
}

/// The events of a reply's stream, in order.
fn events(r: &Reply) -> Vec<Value> {
    dechunk(&r.body)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// What the page sends: a module with one param.
fn request() -> Value {
    json!({
        "module": "filter",
        "moduleText": "plugin({ type: \"internal\", name: \"x\" })",
        "params": [{ "name": "Cutoff", "meta": { "min": 0, "max": 1 }, "value": "0.5" }],
        "uiText": "",
        "widgetsJs": "",
        "mode": "reset",
        "direction": "",
    })
}

/// The model's answer: one knob on `bind`.
fn answer(bind: &str) -> String {
    json!({
        "header": "filter — a test",
        "panel": { "cols": 4, "rows": 2, "widgets": [
            { "id": "cut", "type": "knob", "col": 0, "row": 0, "colSpan": 2, "rowSpan": 2, "label": "Cut", "bind": bind, "min": 0, "max": 1 }
        ] },
        "widgets_js": "",
        "notes": "one knob",
    })
    .to_string()
}

#[tokio::test]
async fn the_guards_come_in_their_order() {
    let (url, seen) = claude(vec![(
        404,
        r#"{"type":"error","error":{"type":"not_found_error","message":"model: x"}}"#.into(),
    )])
    .await;
    let (f, c) = started("guards", &url).await;
    // No session: refused, and the body is not even read as a request.
    let r = send(
        f.addr,
        "POST",
        "/api/ai/organize",
        &[("Content-Type", "application/json")],
        &request().to_string(),
    )
    .await;
    assert_eq!(
        (r.status, r.body.as_str()),
        (401, r#"{"error":"unauthenticated"}"#)
    );
    // A session without a key for the account.
    let r = post(&f, &c, &request().to_string()).await;
    assert_eq!(r.status, 404);
    assert!(
        r.body.contains("no Claude key for this account"),
        "{}",
        r.body
    );
    keyed(&f, &c, Some("claude-sonnet-4-5")).await;
    // A body past the limit is refused before anything else.
    let big = json!({ "module": "x".repeat(2 * 1024 * 1024) }).to_string();
    assert_eq!(post(&f, &c, &big).await.status, 413);
    // Bodies that are no request: missing parts, an update with nothing to change, a module with no name.
    let mut update = request();
    update["mode"] = json!("update");
    update["direction"] = json!("  ");
    let mut nameless = request();
    nameless["module"] = json!("");
    for bad in [json!({}), json!([]), update, nameless] {
        let r = post(&f, &c, &bad.to_string()).await;
        assert_eq!(r.status, 400, "{bad}: {}", r.body);
    }
    assert!(
        seen.lock().unwrap().is_empty(),
        "nothing reached Claude before a good request"
    );
}

#[tokio::test]
async fn a_good_first_answer_is_the_result_and_claude_hears_only_the_accounts_key_and_model() {
    let (url, seen) = claude(vec![(200, stream_of(&answer("Cutoff")))]).await;
    let (f, c) = started("good", &url).await;
    keyed(&f, &c, Some("claude-sonnet-4-5")).await;
    // The page names its own model and key; neither is believed.
    let mut body = request();
    body["model"] = json!("claude-evil");
    body["apiKey"] = json!("sk-evil");
    let r = post(&f, &c, &body.to_string()).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("content-type"), Some("application/x-ndjson"));
    let lines = events(&r);
    let last = lines.last().unwrap();
    assert_eq!(last["type"], "done", "{lines:?}");
    let result = &last["result"];
    assert!(
        result["uiText"]
            .as_str()
            .unwrap()
            .starts_with("// filter — a test\ncols(4)"),
        "{last}"
    );
    assert_eq!(result["notes"], "one knob");
    assert_eq!(
        lines[0],
        json!({ "type": "status", "text": "Claude is designing the panel…" })
    );
    assert!(!dechunk(&r.body).contains(SECRET_KEY));
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, SECRET_KEY);
    assert_eq!(seen[0].1["model"], "claude-sonnet-4-5");
    assert_eq!(seen[0].1["max_tokens"], 64000);
    let prompt = seen[0].1["messages"][0]["content"].as_str().unwrap();
    assert!(prompt.starts_with("Module: filter\n"), "{prompt}");
    assert!(
        prompt.contains("- \"Cutoff\": range 0..1, value 0.5"),
        "{prompt}"
    );
    assert!(
        seen[0].1["system"][0]["text"]
            .as_str()
            .unwrap()
            .contains("# What to return")
    );
    assert_eq!(
        seen[0].1["output_config"]["format"]["schema"]["required"][1],
        "panel"
    );
}

#[tokio::test]
async fn a_mistake_is_handed_back_once_and_a_corrected_answer_lands() {
    let (url, seen) = claude(vec![
        (200, stream_of(&answer("Nope"))),
        (200, stream_of(&answer("Cutoff"))),
    ])
    .await;
    let (f, c) = started("repair", &url).await;
    keyed(&f, &c, None).await;
    let r = post(&f, &c, &request().to_string()).await;
    let lines = events(&r);
    assert_eq!(lines.last().unwrap()["type"], "done", "{lines:?}");
    assert!(
        lines
            .iter()
            .any(|l| l["text"] == "Claude is fixing the panel…"),
        "{lines:?}"
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    let messages = &seen[1].1["messages"];
    assert_eq!(messages.as_array().unwrap().len(), 3);
    assert_eq!(messages[1]["role"], "assistant");
    let repair = messages[2]["content"].as_str().unwrap();
    assert_eq!(
        repair,
        "The panel was refused:\n- `cut` binds `Nope`, which is not a parameter\n\nReturn the whole corrected answer."
    );
}

#[tokio::test]
async fn two_wrong_answers_end_in_an_error_naming_the_mistakes() {
    let (url, seen) = claude(vec![(200, stream_of(&answer("Nope")))]).await;
    let (f, c) = started("refused", &url).await;
    keyed(&f, &c, None).await;
    let lines = events(&post(&f, &c, &request().to_string()).await);
    let last = lines.last().unwrap();
    assert_eq!(last["type"], "error");
    assert_eq!(
        last["error"],
        "Claude's panel did not pass the checks — `cut` binds `Nope`, which is not a parameter"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        2,
        "a proposal and one repair, no more"
    );
    // A model's junk is a refused answer, never a crash.
    let (url, _) = claude(vec![(200, stream_of("not json"))]).await;
    let (f, c) = started("junk", &url).await;
    keyed(&f, &c, None).await;
    let lines = events(&post(&f, &c, &request().to_string()).await);
    assert_eq!(
        lines.last().unwrap()["error"],
        "Claude's panel did not pass the checks — the answer was not the JSON object asked for"
    );
}

#[tokio::test]
async fn a_refused_model_is_the_apis_own_sentence_as_one_error_line() {
    let (url, seen) = claude(vec![(404, r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-sonnet-4-5"}}"#.into())]).await;
    let (f, c) = started("api-error", &url).await;
    keyed(&f, &c, Some("claude-sonnet-4-5")).await;
    let lines = events(&post(&f, &c, &request().to_string()).await);
    let last = lines.last().unwrap();
    assert_eq!(last["type"], "error");
    assert!(
        last["error"]
            .as_str()
            .unwrap()
            .starts_with("Claude API: 404"),
        "{last}"
    );
    assert_eq!(seen.lock().unwrap().len(), 1, "a 404 is not retried");
}
