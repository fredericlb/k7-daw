//! `POST /api/ai/organize` against `fixtures/organize-vectors.json`, the reference the TypeScript route and loop wrote: the
//! same model replies must draw the same requests to Claude, the same progress lines and the same end.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{front, send};

/// What the replay saw of each request: the key header and the body.
type Seen = Arc<Mutex<Vec<(String, Value)>>>;

/// A Claude that plays the vector's replies in turn, each in the pieces and the ending the vector names.
async fn replay(replies: Vec<Value>) -> (String, Seen) {
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
                let body: Value = serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
                let nth = {
                    let mut log = log.lock().unwrap();
                    log.push((key, body));
                    log.len() - 1
                };
                let r = &replies[nth.min(replies.len() - 1)];
                let status = r["status"].as_u64().unwrap_or(200);
                let mut out = format!("HTTP/1.1 {status} X\r\n");
                for (k, v) in r["headers"].as_object().into_iter().flatten() {
                    out.push_str(&format!("{k}: {}\r\n", v.as_str().unwrap_or_default()));
                }
                out.push_str("transfer-encoding: chunked\r\nconnection: close\r\n\r\n");
                let _ = sock.write_all(out.as_bytes()).await;
                let bytes = r["body"].as_str().unwrap_or_default().as_bytes();
                let split = r["split"]
                    .as_u64()
                    .map_or(bytes.len().max(1), |s| s as usize);
                for piece in bytes.chunks(split) {
                    let _ = sock
                        .write_all(format!("{:x}\r\n", piece.len()).as_bytes())
                        .await;
                    let _ = sock.write_all(piece).await;
                    let _ = sock.write_all(b"\r\n").await;
                    let _ = sock.flush().await;
                }
                // `close` ends the body as it should; `cut` leaves it unfinished, as a dropped connection does.
                if r["end"] != "cut" {
                    let _ = sock.write_all(b"0\r\n\r\n").await;
                }
                let _ = sock.shutdown().await;
            });
        }
    });
    (url, seen)
}

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

fn lines(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn the_same_model_replies_draw_the_same_requests_progress_and_end() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/organize-vectors.json"
    );
    let vectors: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let key = vectors["apiKey"].as_str().unwrap();
    let cases = vectors["cases"].as_array().unwrap();
    assert!(cases.len() >= 16);
    let cookie = crate::http_support::cookie();
    for (n, c) in cases.iter().enumerate() {
        let name = c["name"].as_str().unwrap();
        let replies = c["replies"].as_array().cloned().unwrap_or_default();
        let (url, seen) = replay(if replies.is_empty() {
            vec![Value::Null]
        } else {
            replies
        })
        .await;
        let prefs = std::env::temp_dir().join(format!("k7-ai-vectors-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&prefs);
        let f = front(
            &format!("ai-vectors-{n}"),
            &[
                ("STUDIO_PREFS_DIR", prefs.to_str().unwrap()),
                ("ANTHROPIC_BASE_URL", &url),
            ],
        )
        .await;
        let settings = serde_json::json!({ "apiKey": key, "model": c["model"] }).to_string();
        let put = send(
            f.addr,
            "PUT",
            "/api/ai/settings",
            &[("Content-Type", "application/json"), ("Cookie", &cookie)],
            &settings,
        )
        .await;
        assert_eq!(put.status, 200, "{name}: {}", put.body);
        let r = send(
            f.addr,
            "POST",
            "/api/ai/organize",
            &[("Content-Type", "application/json"), ("Cookie", &cookie)],
            &c["request"].to_string(),
        )
        .await;
        assert_eq!(
            r.status as u64,
            c["status"].as_u64().unwrap(),
            "{name}: {}",
            r.body
        );
        let want = lines(c["answer"].as_str().unwrap());
        if r.status != 200 {
            // The old route said zod's JSON; the sentence is ours, the refusal is the same.
            assert!(
                want[0]["error"].is_string() && seen.lock().unwrap().is_empty(),
                "{name}"
            );
            continue;
        }
        let got = lines(&dechunk(&r.body));
        let progress = |l: &[Value]| {
            l.iter()
                .filter(|e| e["type"] == "status")
                .map(|e| e["text"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(progress(&got), progress(&want), "{name}: progress");
        let (last, last_want) = (got.last().unwrap(), want.last().unwrap());
        if last_want["type"] == "done" {
            assert_eq!(last["type"], "done", "{name}: {last}");
            assert_eq!(last["result"], last_want["result"], "{name}: the panel");
        } else {
            let mut said = vec![last_want["error"].as_str().unwrap().to_owned()];
            said.extend(
                last_want["issues"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| i.as_str().unwrap().to_owned()),
            );
            assert_eq!(last["type"], "error", "{name}: {last}");
            assert_eq!(last["error"], said.join(" — "), "{name}: the error");
        }
        let upstream = c["upstream"].as_array().unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), upstream.len(), "{name}: requests to Claude");
        for (i, (got, want)) in seen.iter().zip(upstream).enumerate() {
            assert_eq!(
                got.0, want["headers"]["x-api-key"],
                "{name}: request {i}'s key"
            );
            let sent: Value = serde_json::from_str(want["body"].as_str().unwrap()).unwrap();
            assert_eq!(got.1, sent, "{name}: request {i}'s body");
        }
    }
}
