//! `POST /api/ai/organize/turn` through a whole front: the guards in their order, the shape of a turn, and what
//! reaches Claude (the account's key and model, nothing the browser chose). The replies are played by a fake.

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

/// A Claude that answers every request with `status` and `body`, remembering what it was sent.
async fn claude(status: u16, body: &'static str) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let log = log.clone();
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
                log.lock().unwrap().push((key, body_in));
                let out = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(out.as_bytes()).await;
            });
        }
    });
    (url, seen)
}

async fn started(name: &str, base: &str) -> (Front, String) {
    let prefs = std::env::temp_dir().join(format!("k7-ai-turn-{name}-{}", std::process::id()));
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
        "/api/ai/organize/turn",
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

fn turn() -> String {
    json!({
        "system": "You design panels.",
        "schema": { "type": "object" },
        "messages": [{ "role": "user", "content": "go" }],
    })
    .to_string()
}

#[tokio::test]
async fn the_guards_come_in_their_order() {
    let (url, seen) = claude(
        404,
        r#"{"type":"error","error":{"type":"not_found_error","message":"model: x"}}"#,
    )
    .await;
    let (f, c) = started("guards", &url).await;
    // No session: refused, and the body is not even read as a turn.
    let r = send(
        f.addr,
        "POST",
        "/api/ai/organize/turn",
        &[("Content-Type", "application/json")],
        &turn(),
    )
    .await;
    assert_eq!(
        (r.status, r.body.as_str()),
        (401, r#"{"error":"unauthenticated"}"#)
    );
    // A session without a key for the account.
    let r = post(&f, &c, &turn()).await;
    assert_eq!(r.status, 404);
    assert!(
        r.body.contains("no Claude key for this account"),
        "{}",
        r.body
    );
    keyed(&f, &c, Some("claude-sonnet-4-5")).await;
    // A body past the limit is refused before anything else.
    let big =
        json!({ "system": "x".repeat(2 * 1024 * 1024), "schema": {}, "messages": [] }).to_string();
    assert_eq!(post(&f, &c, &big).await.status, 413);
    // A body that is no turn.
    for bad in ["{}", "[]", r#"{"system":"s","schema":{},"messages":[]}"#] {
        let r = post(&f, &c, bad).await;
        assert_eq!(r.status, 400, "{bad}: {}", r.body);
    }
    assert!(
        seen.lock().unwrap().is_empty(),
        "nothing reached Claude before a good turn"
    );
}

#[tokio::test]
async fn claude_hears_the_accounts_key_and_model_and_the_page_never_sees_the_key() {
    let (url, seen) = claude(404, r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-sonnet-4-5"}}"#).await;
    let (f, c) = started("model", &url).await;
    keyed(&f, &c, Some("claude-sonnet-4-5")).await;
    // The page names its own model and key; neither is believed.
    let body = json!({
        "system": "S", "schema": { "type": "object" }, "model": "claude-evil", "apiKey": "sk-evil",
        "messages": [{ "role": "user", "content": "go", "extra": 1 }],
    })
    .to_string();
    let r = post(&f, &c, &body).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("content-type"), Some("application/x-ndjson"));
    // A refused model is the API's own sentence, as one error line.
    let body = dechunk(&r.body);
    let lines: Vec<Value> = body
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["type"], "error");
    assert!(
        lines[0]["error"]
            .as_str()
            .unwrap()
            .starts_with("Claude API: 404"),
        "{}",
        lines[0]
    );
    assert!(!body.contains(SECRET_KEY));
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "a 404 is not retried");
    assert_eq!(seen[0].0, SECRET_KEY);
    assert_eq!(seen[0].1["model"], "claude-sonnet-4-5");
    assert_eq!(
        seen[0].1["messages"],
        json!([{ "role": "user", "content": "go" }])
    );
    assert_eq!(seen[0].1["max_tokens"], 64000);
}
