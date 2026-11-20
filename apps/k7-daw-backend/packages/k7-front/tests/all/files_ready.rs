//! `/api/ready`, answered by the front under `K7_FILES=front` once the hub is launched: the cases of
//! the conformance spec `ready`, against a front on a files root of its own.

use std::net::SocketAddr;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Front {
    addr: SocketAddr,
}

async fn front(extra: &[(&str, &str)]) -> Front {
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("k7-front-ready-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = |p: &str| root.join(p).to_string_lossy().into_owned();
    let mut env: std::collections::BTreeMap<String, String> = [
        ("K7_SESSION_TOKEN", crate::http_support::TOKEN.to_string()),
        ("STUDIO_AUTH_DIR", path("auth")),
        ("STUDIO_FILES_ROOT", path("files")),
        ("STUDIO_PREFS_DIR", path("prefs")),
        ("STUDIO_MEDIATEK", path("mediatek")),
        ("K7_FILES", "front".into()),
        ("K7_ENGINE_CORE", "front".into()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    for (k, v) in extra {
        env.insert(k.to_string(), v.to_string());
    }
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    let f = Front {
        addr: rx.await.unwrap(),
    };
    for _ in 0..100 {
        let host = f.addr.to_string();
        if send(&f, "GET", "/api/ready", &[("host", &host)], "")
            .await
            .body
            .starts_with('{')
        {
            return f;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the front never took the file hub");
}

struct Reply {
    status: u16,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }
}

async fn send(f: &Front, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Reply {
    let host = headers
        .iter()
        .find(|(k, _)| *k == "host")
        .map_or(f.addr.to_string(), |(_, v)| v.to_string());
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers.iter().filter(|(k, _)| *k != "host") {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() && !headers.iter().any(|(k, _)| *k == "content-type") {
        raw.push_str("content-type: application/json\r\n");
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = tokio::net::TcpStream::connect(f.addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    let read =
        tokio::time::timeout(std::time::Duration::from_secs(30), s.read_to_end(&mut out)).await;
    read.unwrap_or_else(|_| panic!("{method} {path}: no answer within 30 s"))
        .unwrap();
    let at = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..at]).into_owned();
    let mut lines = head.lines();
    let status = lines.next().unwrap()[9..12].parse().unwrap();
    Reply {
        status,
        body: String::from_utf8_lossy(&out[at + 4..]).into_owned(),
    }
}

#[tokio::test]
async fn ready_answers_this_machine_only() {
    let f = front(&[]).await;
    let host = f.addr.to_string();
    let r = send(&f, "GET", "/api/ready", &[("host", &host)], "").await;
    assert_eq!(r.status, 200, "{}", r.body);
    let v = r.json();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["ok", "problems", "volumes", "files"]);
    assert!(
        v["volumes"]
            .as_object()
            .unwrap()
            .values()
            .all(|x| x == "ok")
    );
    let files: Vec<&String> = v["files"].as_object().unwrap().keys().collect();
    assert_eq!(files, ["running"]);
    let r = send(&f, "GET", "/api/ready", &[("host", "example.com")], "").await;
    assert_eq!(
        (r.status, r.body.as_str()),
        (403, "This server answers its own machine only.")
    );
}
