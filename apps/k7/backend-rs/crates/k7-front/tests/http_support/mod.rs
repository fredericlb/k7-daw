//! What the front's tests share: a backend whose file half answers the seam and echoes every other request,
//! a front with a data folder of its own, and one raw HTTP/1.1 exchange with it.

#![allow(dead_code, reason = "each test binary uses its own share of these")]

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The bearer the stub backend says is `service:k7`'s.
pub const SERVICE_TOKEN: &str = "tok-svc";

/// A backend whose file half answers the seam and echoes every other request.
pub async fn backend() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = l.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = s.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    let Some((head, body)) = text.split_once("\r\n\r\n") else {
                        continue;
                    };
                    let len = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if body.len() < len {
                        continue;
                    }
                    let line = head.lines().next().unwrap_or("").to_string();
                    let gated = head.to_ascii_lowercase().contains("x-k7-internal: s\r\n");
                    let json = |b: String| (200, "application/json", b);
                    let (status, kind, b) = match line.split(' ').take(2).collect::<Vec<_>>()[..] {
                        ["POST", "/api/internal/files/verify-token"] if gated => json(
                            if body.contains(&format!("\"{SERVICE_TOKEN}\"")) {
                                r#"{"sub":"service:k7"}"#
                            } else {
                                r#"{"sub":null}"#
                            }
                            .to_string(),
                        ),
                        [method, path] => (200, "text/plain", format!("ts {method} {path}")),
                        _ => (400, "text/plain", String::new()),
                    };
                    let reply = format!(
                        "HTTP/1.1 {status} OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                        b.len()
                    );
                    let _ = s.write_all(reply.as_bytes()).await;
                    return;
                }
            });
        }
    });
    addr
}

/// A front over a data folder of its own; its address and that folder.
pub async fn front(upstream: SocketAddr, extra: &[(&str, &str)]) -> (SocketAddr, PathBuf) {
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("k7-front-http-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let library = root.join("library");
    let mut env: std::collections::BTreeMap<String, String> = [
        ("SESSION_SECRET", "test-secret".to_string()),
        (
            "STUDIO_AUTH_DIR",
            root.join("auth").to_string_lossy().into_owned(),
        ),
        ("STUDIO_MEDIATEK", library.to_string_lossy().into_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    for (k, v) in extra {
        env.insert(k.to_string(), v.to_string());
    }
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream.to_string(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    (rx.await.unwrap(), library)
}

/// One exchange: the status, the headers (lowercased names) and the body's bytes.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.text()))
    }
}

pub async fn send(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)]) -> Reply {
    send_with(addr, method, path, headers, "").await
}

/// The same, with a JSON body — the routes that take one are asked what they refuse.
pub async fn send_with(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    let at = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..at]).into_owned();
    let mut lines = head.lines();
    let status = lines.next().unwrap()[9..12].parse().unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: out[at + 4..].to_vec(),
    }
}

/// The `Cookie` value that signs `user` in.
pub fn cookie(user: serde_json::Value) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    k7_auth::cookie_header(
        user.as_object().unwrap().clone(),
        "studio.sid",
        "test-secret",
        now,
    )
}
