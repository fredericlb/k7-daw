//! turn_vectors.rs — the turn route against `fixtures/organize-vectors.json`, the reference as committed (the SDK's
//! requests and the NDJSON of the TypeScript route that ran the whole conversation): the same replies played by a
//! fake Claude must draw the same requests, byte for byte, and the NDJSON each turn is the server's part of.
//! The fake takes one connection per request.

use std::sync::{Arc, Mutex};

use http_body_util::BodyExt;
use serde_json::{Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::routes::ai::turn::{HEARTBEAT, Turn, answer};

/// The request headers compared with the reference's, byte for byte.
const COMPARED: [&str; 7] = [
    "accept",
    "anthropic-version",
    "content-type",
    "x-api-key",
    "x-stainless-helper-method",
    "x-stainless-retry-count",
    "x-stainless-timeout",
];

#[derive(Default)]
struct Script {
    replies: Vec<Value>,
    seen: Vec<Value>,
}

/// One request read off the socket: its line, its headers lowercased, its body.
async fn read_request(sock: &mut TcpStream) -> Option<(String, Vec<(String, String)>, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let line = lines.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let len: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some((line, headers, body))
}

/// Record the request, then play the next reply in chunks, ended or cut.
async fn serve(mut sock: TcpStream, script: Arc<Mutex<Script>>) {
    let Some((line, headers, body)) = read_request(&mut sock).await else {
        return;
    };
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut compared = Map::new();
    for h in COMPARED {
        if let Some((_, v)) = headers.iter().find(|(k, _)| k == h) {
            compared.insert(h.into(), Value::String(v.clone()));
        }
    }
    let reply = {
        let mut s = script.lock().unwrap();
        s.seen.push(serde_json::json!({
            "method": method,
            "path": path,
            "headers": compared,
            "body": String::from_utf8_lossy(&body),
        }));
        (!s.replies.is_empty()).then(|| s.replies.remove(0))
    };
    let Some(r) = reply else {
        let _ = sock
            .write_all(b"HTTP/1.1 500 Nope\r\ncontent-length: 18\r\nconnection: close\r\n\r\nno reply scripted")
            .await;
        return;
    };
    let mut head = format!("HTTP/1.1 {} X\r\n", r["status"]);
    for (k, v) in r["headers"].as_object().unwrap() {
        head.push_str(&format!("{k}: {}\r\n", v.as_str().unwrap()));
    }
    head.push_str("transfer-encoding: chunked\r\nconnection: close\r\n\r\n");
    if sock.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let bytes = r["body"].as_str().unwrap().as_bytes();
    let split = r["split"].as_u64().unwrap() as usize;
    for piece in bytes.chunks(split) {
        let mut c = format!("{:x}\r\n", piece.len()).into_bytes();
        c.extend_from_slice(piece);
        c.extend_from_slice(b"\r\n");
        if sock.write_all(&c).await.is_err() {
            return;
        }
        let _ = sock.flush().await;
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    // A cut leaves the chunked body without its last chunk, which the client reads as a body cut short.
    if r["end"] == "close" {
        let _ = sock.write_all(b"0\r\n\r\n").await;
        let _ = sock.flush().await;
    }
}

async fn fake() -> (String, Arc<Mutex<Script>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let script: Arc<Mutex<Script>> = Arc::default();
    let s = script.clone();
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            tokio::spawn(serve(sock, s.clone()));
        }
    });
    (url, script)
}

/// The turns a case asked for, read off the requests the TypeScript route made (a retry of one counted once), each with its first request's place.
fn turns(c: &Value) -> Vec<(Turn, usize)> {
    let mut out: Vec<(Turn, usize)> = Vec::new();
    let mut last_len = 0;
    for (at, u) in c["upstream"].as_array().unwrap().iter().enumerate() {
        let body: Value = serde_json::from_str(u["body"].as_str().unwrap()).unwrap();
        let messages = body["messages"].as_array().unwrap().clone();
        if out.is_empty() || messages.len() != last_len {
            last_len = messages.len();
            out.push((
                Turn {
                    system: body["system"][0]["text"].as_str().unwrap().into(),
                    schema: body["output_config"]["format"]["schema"].clone(),
                    messages,
                },
                at,
            ));
        }
    }
    out
}

/// What the route said per round: its progress lines, and the error that ended the round if the server's.
fn rounds(c: &Value) -> Vec<(Vec<Value>, Option<Value>)> {
    let mut out: Vec<(Vec<Value>, Option<Value>)> = Vec::new();
    for line in c["answer"].as_str().unwrap().lines() {
        let l: Value = serde_json::from_str(line).unwrap();
        let t = l["type"].as_str().unwrap_or_default();
        if t == "status" && l["text"].as_str().unwrap().starts_with("Claude is ") {
            out.push((Vec::new(), None));
        } else if t == "status" {
            out.last_mut().unwrap().0.push(l);
        } else if t == "error" && l.get("issues").is_none() {
            out.last_mut().unwrap().1 = Some(l);
        }
    }
    out
}

/// The text deltas of a reply's blocks, joined.
fn text_of(reply: &Value) -> String {
    reply["body"]
        .as_str()
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .filter(|e| e["delta"]["type"] == "text_delta")
        .map(|e| e["delta"]["text"].as_str().unwrap().to_string())
        .collect()
}

/// The NDJSON of one turn, line by line.
async fn ndjson(res: axum::response::Response) -> Vec<Value> {
    let body = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&body)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_and_streams_what_typescript_does() {
    let v: Value =
        serde_json::from_str(include_str!("../../../fixtures/organize-vectors.json")).unwrap();
    let key = v["apiKey"].as_str().unwrap();
    let (url, script) = fake().await;
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 16);
    let mut played = 0;
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let asked = turns(c);
        if asked.is_empty() {
            continue;
        }
        {
            let mut s = script.lock().unwrap();
            s.replies = c["replies"].as_array().unwrap().clone();
            s.seen.clear();
        }
        let model = c["model"].as_str().unwrap().to_string();
        let said = rounds(c);
        assert_eq!(said.len(), asked.len(), "{name}: rounds");
        let mut next_reply = 0;
        let firsts: Vec<usize> = asked.iter().map(|(_, at)| *at).collect();
        for (j, (turn, _)) in asked.into_iter().enumerate() {
            let res = answer(&url, key.into(), model.clone(), turn, HEARTBEAT);
            assert_eq!(res.status().as_u16(), 200, "{name}: status");
            let got = ndjson(res).await;
            let (progress, failed) = &said[j];
            let (end, written) = got.split_last().unwrap();
            assert_eq!(written, progress.as_slice(), "{name}: round {j} progress");
            if let Some(e) = failed {
                assert_eq!(end, e, "{name}: round {j} error");
                continue;
            }
            assert_eq!(
                end["type"], "turn",
                "{name}: round {j} ends in a turn: {end}"
            );
            // The reply this round used is the last 200 one it was given, retries before it passed over.
            let replies = c["replies"].as_array().unwrap();
            while replies[next_reply]["status"] != 200 {
                next_reply += 1;
            }
            assert_eq!(
                end["text"],
                text_of(&replies[next_reply]),
                "{name}: round {j} text"
            );
            next_reply += 1;
            // The content is what the next request hands back as the assistant's turn.
            if let Some(next) = firsts.get(j + 1).map(|at| &c["upstream"][*at]) {
                let body: Value = serde_json::from_str(next["body"].as_str().unwrap()).unwrap();
                assert_eq!(
                    end["content"], body["messages"][1]["content"],
                    "{name}: content"
                );
            }
        }
        let seen = std::mem::take(&mut script.lock().unwrap().seen);
        let expected = c["upstream"].as_array().unwrap();
        assert_eq!(seen.len(), expected.len(), "{name}: requests");
        for (i, (got, want)) in seen.iter().zip(expected).enumerate() {
            assert_eq!(got["method"], want["method"], "{name}: request {i}");
            assert_eq!(got["path"], want["path"], "{name}: request {i}");
            assert_eq!(
                got["headers"], want["headers"],
                "{name}: request {i} headers"
            );
            assert!(
                got["body"] == want["body"],
                "{name}: request {i} body differs"
            );
        }
        played += 1;
    }
    assert!(played >= 14, "{played} cases played");
}

fn one_turn() -> Turn {
    Turn {
        system: "s".into(),
        schema: serde_json::json!({ "type": "object" }),
        messages: vec![serde_json::json!({ "role": "user", "content": "go" })],
    }
}

fn sse(text: &str) -> String {
    let ev = |name: &str, data: Value| format!("event: {name}\ndata: {data}\n\n");
    let message = serde_json::json!({ "id": "m", "type": "message", "role": "assistant", "model": "x", "content": [],
        "stop_reason": null, "stop_sequence": null, "usage": { "input_tokens": 1, "output_tokens": 1 } });
    [
        ev("message_start", serde_json::json!({ "type": "message_start", "message": message })),
        ev("content_block_start", serde_json::json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })),
        ev("content_block_delta", serde_json::json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } })),
        ev("content_block_stop", serde_json::json!({ "type": "content_block_stop", "index": 0 })),
        ev("message_delta", serde_json::json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn", "stop_sequence": null }, "usage": { "output_tokens": 1 } })),
        ev("message_stop", serde_json::json!({ "type": "message_stop" })),
    ]
    .concat()
}

fn reply(body: String, split: usize) -> Value {
    serde_json::json!({ "status": 200, "headers": { "content-type": "text/event-stream" }, "body": body, "split": split, "end": "close" })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_turn_keeps_the_line_warm() {
    let (url, script) = fake().await;
    script.lock().unwrap().replies = vec![reply(sse("{}"), 6)];
    let res = answer(
        &url,
        "k".into(),
        "m".into(),
        one_turn(),
        std::time::Duration::from_millis(15),
    );
    let got = ndjson(res).await;
    let pings = got.iter().filter(|l| l["type"] == "ping").count();
    assert!(pings >= 1, "no heartbeat in {got:?}");
    assert_eq!(got.last().unwrap()["type"], "turn");
    assert_eq!(got.last().unwrap()["text"], "{}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_of_ten_megabytes_is_cut() {
    let (url, script) = fake().await;
    let junk = "event: ping\ndata: {\"type\":\"ping\"}\n\n".repeat(10 * 1024 * 1024 / 34);
    script.lock().unwrap().replies = vec![reply(junk, 1 << 20)];
    let res = answer(&url, "k".into(), "m".into(), one_turn(), HEARTBEAT);
    let got = ndjson(res).await;
    let end = got.last().unwrap();
    assert_eq!(end["type"], "error", "{end}");
    assert!(
        end["error"].as_str().unwrap().contains("ran past the size"),
        "{end}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_model_is_the_apis_sentence() {
    let (url, script) = fake().await;
    let body =
        r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-nope"}}"#;
    script.lock().unwrap().replies = vec![serde_json::json!({
        "status": 404, "headers": { "content-type": "application/json" }, "body": body, "split": 1000, "end": "close" })];
    let res = answer(
        &url,
        "k".into(),
        "claude-nope".into(),
        one_turn(),
        HEARTBEAT,
    );
    let got = ndjson(res).await;
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0]["type"], "error");
    let said = got[0]["error"].as_str().unwrap();
    assert!(
        said.starts_with("Claude API: 404") && said.contains("model: claude-nope"),
        "{said}"
    );
}
