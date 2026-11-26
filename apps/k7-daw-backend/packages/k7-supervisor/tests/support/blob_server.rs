//! blob_server.rs — a one-file HTTP/1.1 server on 127.0.0.1 that serves BYTES (the instruments' archives), reads a
//! `Range`, redirects, answers any status, and can hold an answer back until the test lets it go.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Notify;

/// A request as it arrived.
#[derive(Debug, Clone)]
pub struct Asked {
    pub path: String,
    pub range: Option<String>,
}

/// An answer: a status, extra headers, a body, and a gate the answer waits at until it is notified.
#[derive(Default)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub gate: Option<Arc<Notify>>,
}

impl Answer {
    pub fn ok(body: Vec<u8>) -> Answer {
        Answer {
            status: 200,
            body,
            ..Answer::default()
        }
    }
    pub fn status(status: u16) -> Answer {
        Answer {
            status,
            ..Answer::default()
        }
    }
    pub fn header(mut self, k: &str, v: &str) -> Answer {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn redirect(to: &str) -> Answer {
        Answer::status(302).header("Location", to)
    }
    /// The tail of `body` from `from`, as a 206 does.
    pub fn partial(body: &[u8], from: usize) -> Answer {
        Answer {
            status: 206,
            body: body[from..].to_vec(),
            ..Answer::default()
        }
        .header(
            "Content-Range",
            &format!("bytes {from}-{}/{}", body.len() - 1, body.len()),
        )
    }
}

pub struct Blobs {
    pub origin: String,
    asked: Arc<Mutex<Vec<Asked>>>,
}

impl Blobs {
    /// The requests that came for `path`, oldest first.
    pub fn to(&self, path: &str) -> Vec<Asked> {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|a| a.path == path)
            .cloned()
            .collect()
    }
}

async fn read_head(sock: &mut tokio::net::TcpStream) -> Option<Asked> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    let mut lines = head.split("\r\n");
    let path = lines.next()?.split(' ').nth(1)?.to_string();
    let range = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("range"))
        .map(|(_, v)| v.trim().to_string());
    Some(Asked { path, range })
}

pub async fn serve_blobs(handler: impl Fn(&Asked) -> Answer + Send + Sync + 'static) -> Blobs {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let (log, handler) = (asked.clone(), Arc::new(handler));
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (log, handler) = (log.clone(), handler.clone());
            tokio::spawn(async move {
                let Some(req) = read_head(&mut sock).await else {
                    return;
                };
                let answer = handler(&req);
                log.lock().unwrap().push(req);
                if let Some(gate) = &answer.gate {
                    gate.notified().await;
                }
                let mut head = format!("HTTP/1.1 {} Answer\r\n", answer.status);
                for (k, v) in &answer.headers {
                    head += &format!("{k}: {v}\r\n");
                }
                head += &format!(
                    "Content-Length: {}\r\nConnection: close\r\n\r\n",
                    answer.body.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(&answer.body).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    Blobs {
        origin: format!("http://127.0.0.1:{port}"),
        asked,
    }
}
