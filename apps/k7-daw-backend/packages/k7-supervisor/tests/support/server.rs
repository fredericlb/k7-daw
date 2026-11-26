//! server.rs — a one-file HTTP/1.1 server on 127.0.0.1 for the registry tests: it records every request and
//! answers as a handler says (a status, a redirect, a hang, a closed connection), so a registry that refuses,
//! redirects, hangs and answers is each a line in a test.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One request as it arrived; header names lowercased.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|h| h.0 == name)
            .map(|h| h.1.as_str())
    }
}

/// What a handler decides for one request.
pub enum Reply {
    Status(u16, String),
    Redirect(String),
    Hang,
    Close,
}

pub struct Server {
    pub port: u16,
    pub seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The requests that arrived for `path`, oldest first.
    pub fn to(&self, path: &str) -> Vec<Seen> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.path == path)
            .cloned()
            .collect()
    }
}

async fn read_request(sock: &mut tokio::net::TcpStream) -> Option<Seen> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
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
    let mut first = lines.next()?.split(' ');
    let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    let want: usize = headers
        .iter()
        .find(|h| h.0 == "content-length")
        .and_then(|h| h.1.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < want {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some(Seen {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn answer(reply: &Reply) -> Option<String> {
    let (line, extra, body) = match reply {
        Reply::Status(code, body) => (format!("{code} Answer"), String::new(), body.clone()),
        Reply::Redirect(to) => (
            "302 Found".into(),
            format!("Location: {to}\r\n"),
            String::new(),
        ),
        Reply::Hang | Reply::Close => return None,
    };
    Some(format!(
        "HTTP/1.1 {line}\r\n{extra}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

/// Serve on an unused 127.0.0.1 port, or on `port` when given.
pub async fn serve_on(
    port: u16,
    handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static,
) -> Server {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (log, handler) = (log.clone(), handler.clone());
            tokio::spawn(async move {
                let Some(req) = read_request(&mut sock).await else {
                    return;
                };
                let reply = handler(&req);
                log.lock().unwrap().push(req);
                match answer(&reply) {
                    Some(text) => {
                        let _ = sock.write_all(text.as_bytes()).await;
                        let _ = sock.shutdown().await;
                    }
                    None if matches!(reply, Reply::Hang) => std::future::pending::<()>().await,
                    None => {}
                }
            });
        }
    });
    Server { port, seen }
}

pub async fn serve(handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static) -> Server {
    serve_on(0, handler).await
}
