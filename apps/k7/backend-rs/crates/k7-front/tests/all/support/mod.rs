//! A front against no backend, and a raw HTTP/1.1 exchange with it, for the sign-in's tests.

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct Front {
    pub addr: SocketAddr,
    pub auth: PathBuf,
}

pub async fn front(name: &str, extra: &[(&str, &str)]) -> Front {
    let root = std::env::temp_dir().join(format!("k7-front-auth-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut env: std::collections::BTreeMap<String, String> = [
        ("K7_SESSION_TOKEN", crate::http_support::TOKEN),
        ("STUDIO_AUTH_DIR", root.join("auth").to_str().unwrap()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
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
    Front {
        addr: rx.await.unwrap(),
        auth: root.join("auth"),
    }
}

/// A raw HTTP/1.1 exchange: the status, the headers (lowercased names) and the body.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    /// The `name=value` pairs of every Set-Cookie, as a browser would send them back.
    pub fn cookies(&self) -> String {
        self.headers
            .iter()
            .filter(|(k, _)| k == "set-cookie")
            .map(|(_, v)| v.split(';').next().unwrap().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub async fn send(
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
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    let mut lines = head.lines();
    let status = lines.next().unwrap()[9..12].parse().unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: body.to_string(),
    }
}
