//! hub.rs — a tunnel hub and an engine on 127.0.0.1 for the agent's tests. The hub records each handshake,
//! answers `stat`, `get` and `put` from a tree of files it holds (what a server's file hub does), and hands every
//! other frame the agent sends to the test; the engine is a WebSocket server that records and says what it is told.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use k7_engine::tunnel::frames::{decode_binary, encode_binary};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex as AsyncMutex, mpsc};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::{accept_async, accept_hdr_async};

/// What a peer sent that the test is to see.
#[derive(Debug, Clone)]
pub enum Got {
    Text(Value),
    Binary(Value, Vec<u8>),
}

/// One handshake's headers, with `:path` for the request target.
pub type Handshake = Vec<(String, String)>;

pub type Tree = Arc<Mutex<BTreeMap<String, (Vec<u8>, f64)>>>;

/// One connection's way out, replaced at each accept.
type Outbox = Arc<Mutex<Option<mpsc::UnboundedSender<Message>>>>;

pub struct Hub {
    pub port: u16,
    /// Each handshake's request line and headers, oldest first.
    pub handshakes: Arc<Mutex<Vec<Handshake>>>,
    /// Files the hub serves and takes, by the server's path, with their mtime.
    pub tree: Tree,
    pub stats: Arc<AtomicUsize>,
    pub gets: Arc<AtomicUsize>,
    /// A status to refuse the next handshakes with.
    pub refuse: Arc<Mutex<Option<u16>>>,
    inbox: AsyncMutex<mpsc::UnboundedReceiver<Got>>,
    out: Outbox,
}

fn parse(m: &Message) -> Option<Got> {
    match m {
        Message::Text(t) => Some(Got::Text(
            serde_json::from_str(t.as_str()).unwrap_or(Value::Null),
        )),
        Message::Binary(b) => decode_binary(b).map(|(_, start)| {
            let head: Value = serde_json::from_slice(&b[4..start]).unwrap_or(Value::Null);
            Got::Binary(head, b[start..].to_vec())
        }),
        _ => None,
    }
}

impl Hub {
    pub async fn start() -> Hub {
        Hub::start_on(0).await
    }

    pub async fn start_on(port: u16) -> Hub {
        let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let (itx, irx) = mpsc::unbounded_channel();
        let hub = Hub {
            port: listener.local_addr().unwrap().port(),
            handshakes: Arc::default(),
            tree: Arc::default(),
            stats: Arc::default(),
            gets: Arc::default(),
            refuse: Arc::default(),
            inbox: AsyncMutex::new(irx),
            out: Arc::default(),
        };
        let (seen, refuse, out, tree) = (
            hub.handshakes.clone(),
            hub.refuse.clone(),
            hub.out.clone(),
            hub.tree.clone(),
        );
        let (stats, gets) = (hub.stats.clone(), hub.gets.clone());
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let (seen, refuse, out, tree, itx) = (
                    seen.clone(),
                    refuse.clone(),
                    out.clone(),
                    tree.clone(),
                    itx.clone(),
                );
                let (stats, gets) = (stats.clone(), gets.clone());
                tokio::spawn(async move {
                    #[allow(clippy::result_large_err)]
                    let check = |req: &Request,
                                 resp: Response|
                     -> Result<Response, ErrorResponse> {
                        let mut h: Vec<(String, String)> = req
                            .headers()
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                            .collect();
                        h.push((":path".into(), req.uri().to_string()));
                        seen.lock().unwrap().push(h);
                        if let Some(code) = *refuse.lock().unwrap() {
                            let mut r =
                                ErrorResponse::new(Some("update k7 on that machine".to_string()));
                            *r.status_mut() = code.try_into().unwrap();
                            return Err(r);
                        }
                        Ok(resp)
                    };
                    let Ok(ws) = accept_hdr_async(sock, check).await else {
                        return;
                    };
                    let (mut sink, mut stream) = ws.split();
                    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
                    *out.lock().unwrap() = Some(tx.clone());
                    let mut puts: BTreeMap<i64, Vec<u8>> = BTreeMap::new();
                    loop {
                        tokio::select! {
                            m = rx.recv() => match m {
                                Some(Message::Close(_)) | None => { let _ = sink.close().await; return; }
                                Some(m) => if sink.send(m).await.is_err() { return; },
                            },
                            got = stream.next() => {
                                let Some(Ok(m)) = got else { return };
                                let Some(g) = parse(&m) else { continue };
                                match &g {
                                    Got::Text(v) if v["t"] == "stat" => {
                                        stats.fetch_add(1, Ordering::SeqCst);
                                        let entries: Vec<Value> = v["paths"].as_array().unwrap().iter().map(|p| {
                                            let p = p.as_str().unwrap();
                                            match tree.lock().unwrap().get(p) {
                                                Some((b, m)) => json!({"path": p, "kind": "file", "size": b.len(), "mtimeMs": m}),
                                                None => json!({"path": p, "kind": "missing"}),
                                            }
                                        }).collect();
                                        let _ = tx.send(Message::text(json!({"t": "stat-reply", "req": v["req"], "entries": entries}).to_string()));
                                    }
                                    Got::Text(v) if v["t"] == "get" => {
                                        gets.fetch_add(1, Ordering::SeqCst);
                                        let file = tree.lock().unwrap().get(v["path"].as_str().unwrap()).cloned();
                                        match file {
                                            None => { let _ = tx.send(Message::text(json!({"t": "get-error", "req": v["req"], "error": "no such file"}).to_string())); }
                                            Some((bytes, _)) => {
                                                let mid = bytes.len() / 2;
                                                for (part, last) in [(&bytes[..mid], false), (&bytes[mid..], true)] {
                                                    let head = json!({"t": "chunk", "req": v["req"], "last": last});
                                                    let _ = tx.send(Message::binary(encode_binary(&head, part)));
                                                }
                                            }
                                        }
                                    }
                                    Got::Binary(h, payload) if h["t"] == "put" => {
                                        let req = h["req"].as_i64().unwrap();
                                        puts.entry(req).or_default().extend_from_slice(payload);
                                        if h["last"] == true {
                                            let bytes = puts.remove(&req).unwrap();
                                            let (size, mtime) = (bytes.len(), 1_700_000_000_000.0);
                                            tree.lock().unwrap().insert(h["path"].as_str().unwrap().to_string(), (bytes, mtime));
                                            let stat = json!({"path": h["path"], "kind": "file", "size": size, "mtimeMs": mtime});
                                            let _ = tx.send(Message::text(json!({"t": "put-reply", "req": req, "stat": stat}).to_string()));
                                        }
                                    }
                                    _ => { let _ = itx.send(g); }
                                }
                            }
                        }
                    }
                });
            }
        });
        hub
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Handshakes so far.
    pub fn accepted(&self) -> usize {
        self.handshakes.lock().unwrap().len()
    }

    /// A header of the handshake `n`, by lowercase name.
    pub fn header(&self, n: usize, name: &str) -> Option<String> {
        let all = self.handshakes.lock().unwrap();
        all[n]
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    pub fn send(&self, v: Value) {
        self.push(Message::text(v.to_string()));
    }

    pub fn send_binary(&self, header: Value, payload: &[u8]) {
        self.push(Message::binary(encode_binary(&header, payload)));
    }

    fn push(&self, m: Message) {
        let out = self.out.lock().unwrap();
        out.as_ref().expect("an agent connected").send(m).unwrap();
    }

    /// The hub hangs up on the agent.
    pub fn kick(&self) {
        self.push(Message::Close(None));
    }

    /// The next frame the agent sent that the hub does not answer itself.
    pub async fn next(&self) -> Got {
        timeout(Duration::from_secs(5), async {
            self.inbox.lock().await.recv().await.unwrap()
        })
        .await
        .expect("a frame from the agent")
    }

    /// The next text frame, which must be of type `t`.
    pub async fn next_of(&self, t: &str) -> Value {
        match self.next().await {
            Got::Text(v) if v["t"] == t => v,
            other => panic!("wanted a '{t}' frame, got {other:?}"),
        }
    }

    /// Wait for the agent to connect `n` times.
    pub async fn until_accepted(&self, n: usize) {
        until(|| self.accepted() >= n, 5000).await;
    }
}

/// Wait until `ok`, or fail the test after `ms`.
pub async fn until(ok: impl Fn() -> bool, ms: u64) {
    let end = std::time::Instant::now() + Duration::from_millis(ms);
    while !ok() {
        assert!(std::time::Instant::now() < end, "timed out");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// An engine: a WebSocket server that records what it receives and says what it is told.
pub struct Engine {
    pub port: u16,
    pub got: Arc<Mutex<Vec<Got>>>,
    pub connections: Arc<AtomicUsize>,
    out: Outbox,
}

impl Engine {
    pub async fn start() -> Engine {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let e = Engine {
            port: listener.local_addr().unwrap().port(),
            got: Arc::default(),
            connections: Arc::default(),
            out: Arc::default(),
        };
        let (got, connections, out) = (e.got.clone(), e.connections.clone(), e.out.clone());
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let Ok(ws) = accept_async(sock).await else {
                    continue;
                };
                connections.fetch_add(1, Ordering::SeqCst);
                let (mut sink, mut stream) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
                *out.lock().unwrap() = Some(tx);
                let got = got.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            m = rx.recv() => match m {
                                Some(Message::Close(_)) | None => { let _ = sink.close().await; return; }
                                Some(m) => if sink.send(m).await.is_err() { return; },
                            },
                            m = stream.next() => match m {
                                Some(Ok(Message::Text(t))) => got.lock().unwrap().push(Got::Text(serde_json::from_str(t.as_str()).unwrap_or(Value::Null))),
                                Some(Ok(Message::Binary(b))) => got.lock().unwrap().push(Got::Binary(Value::Null, b.to_vec())),
                                Some(Ok(_)) => {}
                                _ => return,
                            },
                        }
                    }
                });
            }
        });
        e
    }

    pub fn say(&self, v: Value) {
        self.push(Message::text(v.to_string()));
    }

    pub fn say_binary(&self, bytes: Vec<u8>) {
        self.push(Message::binary(bytes));
    }

    fn push(&self, m: Message) {
        self.out
            .lock()
            .unwrap()
            .as_ref()
            .expect("the agent connected")
            .send(m)
            .unwrap();
    }

    /// The engine goes away.
    pub fn hang_up(&self) {
        self.push(Message::Close(None));
    }

    pub fn texts(&self) -> Vec<Value> {
        self.got
            .lock()
            .unwrap()
            .iter()
            .filter_map(|g| {
                if let Got::Text(v) = g {
                    Some(v.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}
