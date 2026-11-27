//! bed.rs — what the hub's suite stands on: the testbed process (the front with the engine core and its hooks), a TCP
//! relay a test can cut, WebSocket clients, a fake engine, a bare supervisor speaking frames by hand, and the real agent.
//! What a reader gets wrong: a hook reaches the core directly and stands for nothing a page can do.

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use k7_engine::tunnel::frames::encode_binary;
use k7_supervisor::tunnel::{Options, TunnelAgent};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::process::{Child, Command};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{accept_async, client_async};

pub const SECRET: &str = "engine-testbed-secret";
pub const TUNNEL_PATH: &str = "/api/engine-tunnel";

/// A scratch folder of this run, removed with the value.
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn new(what: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "k7-hub-{what}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        // The real path: a root named through a link would be judged by where it leads.
        Dir(p.canonicalize().unwrap())
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// The folder as the server's text names it.
    pub fn s(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Waits for `ok`, or fails the test after `ms`.
pub async fn until(ok: impl Fn() -> bool, ms: u64) {
    until_async(|| async { ok() }, ms).await
}

/// Waits for `ok`, which asks the hub asynchronously, or fails the test after `ms`.
pub async fn until_async<F, Fut>(mut ok: F, ms: u64)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let end = Instant::now() + Duration::from_millis(ms);
    while !ok().await {
        assert!(Instant::now() < end, "timed out");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// One exchange with the testbed over a plain socket: the status and the body.
async fn post(port: u16, path: &str, body: &Value) -> (u16, String) {
    let body = body.to_string();
    let raw = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\ncontent-type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = String::new();
    // A hung testbed fails here, naming the hook, instead of eating the caller's whole test.
    tokio::time::timeout(Duration::from_secs(20), s.read_to_string(&mut out))
        .await
        .unwrap_or_else(|_| panic!("{path} did not answer"))
        .unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (head[9..12].parse().unwrap(), body.to_string())
}

/// A running testbed: its port, what it logged (prefix aside), and its hooks.
pub struct Testbed {
    pub port: u16,
    pub lines: Arc<Mutex<Vec<String>>>,
    child: Child,
    _auth: Dir,
}

impl Testbed {
    /// The testbed with `env` over its defaults: sessions known, the bindings in memory, every interface.
    pub async fn start(env: &[(&str, String)]) -> Testbed {
        let auth = Dir::new("auth");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_k7-engine-testbed"));
        cmd.env("K7_ENGINE_TESTBED", "1")
            .env("K7_FRONT_SECRET", SECRET)
            .env("K7_FRONT_LISTEN", "0.0.0.0:0")
            .env("K7_SESSION_TOKEN", SECRET)
            .env("STUDIO_AUTH_DIR", auth.s())
            .env("K7_ENGINE_TEST_MEMORY", "1");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("the testbed starts");
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let (port_tx, mut port_rx) = mpsc::unbounded_channel::<u16>();
        for (reader, is_out) in [
            (
                Box::new(child.stdout.take().unwrap())
                    as Box<dyn tokio::io::AsyncRead + Unpin + Send>,
                true,
            ),
            (Box::new(child.stderr.take().unwrap()), false),
        ] {
            let (lines, tx) = (lines.clone(), port_tx.clone());
            tokio::spawn(async move {
                let mut r = BufReader::new(reader).lines();
                while let Ok(Some(l)) = r.next_line().await {
                    if is_out
                        && let Some(p) = l
                            .strip_prefix("k7-front on http://")
                            .and_then(|a| a.rsplit(':').next())
                            .and_then(|p| p.parse().ok())
                    {
                        let _ = tx.send(p);
                    }
                    let l = l
                        .strip_prefix("[registry] ")
                        .or_else(|| l.strip_prefix("tunnel | "))
                        .unwrap_or(&l)
                        .to_string();
                    lines.lock().unwrap().push(l);
                }
            });
        }
        let port = tokio::time::timeout(Duration::from_secs(20), port_rx.recv())
            .await
            .unwrap_or_else(|_| panic!("the testbed did not start: {:?}", lines.lock().unwrap()))
            .expect("the testbed's port");
        Testbed {
            port,
            lines,
            child,
            _auth: auth,
        }
    }

    pub async fn hook(&self, op: &str, body: Value) -> Value {
        let (_, text) = post(self.port, &format!("/__engine-test/{op}"), &body).await;
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }

    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
    }
}

/// A TCP relay whose sockets `cut` drops, for a hub in another process.
pub struct Relay {
    pub port: u16,
    pairs: Arc<Mutex<Vec<JoinHandle<()>>>>,
    accept: JoinHandle<()>,
}

impl Relay {
    async fn start(to: u16) -> Relay {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let pairs: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::default();
        let held = pairs.clone();
        let accept = tokio::spawn(async move {
            while let Ok((mut a, _)) = listener.accept().await {
                let pair = tokio::spawn(async move {
                    if let Ok(mut b) = TcpStream::connect(("127.0.0.1", to)).await {
                        let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
                    }
                });
                held.lock().unwrap().push(pair);
            }
        });
        Relay {
            port,
            pairs,
            accept,
        }
    }

    /// Drops every tunnel's socket as a network would, no close frame.
    pub fn cut(&self) {
        for p in self.pairs.lock().unwrap().drain(..) {
            p.abort();
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.cut();
        self.accept.abort();
    }
}

/// What one tunnel may upload.
#[derive(Clone, Copy)]
pub struct UploadLimits {
    pub max_open: u64,
    pub max_bytes: u64,
    pub bytes_per_hour: u64,
    pub idle_ms: u64,
}

impl Default for UploadLimits {
    fn default() -> Self {
        UploadLimits {
            max_open: 16,
            max_bytes: 1 << 31,
            bytes_per_hour: 20 << 30,
            idle_ms: 60_000,
        }
    }
}

/// Who may hold a tunnel, and the limits the hub is given.
#[derive(Default)]
pub struct HubOptions {
    /// None: anyone; else the one id with its token (and secret when given).
    pub verify: Option<(&'static str, &'static str, Option<&'static str>)>,
    pub min_protocol: Option<u32>,
    pub uploads: Option<UploadLimits>,
    pub ping_ms: Option<u64>,
}

/// The hub under test, started.
pub struct Hub {
    pub bed: Testbed,
    pub relay: Relay,
}

impl Hub {
    pub async fn start(o: HubOptions) -> Hub {
        let mut env: Vec<(&str, String)> = vec![];
        if let Some(m) = o.min_protocol {
            env.push(("K7_ENGINE_TEST_MIN_PROTOCOL", m.to_string()));
        }
        if let Some(p) = o.ping_ms {
            env.push(("K7_ENGINE_TEST_PING_MS", p.to_string()));
        }
        if let Some(u) = o.uploads {
            env.push(("K7_ENGINE_TEST_UPLOAD_OPEN", u.max_open.to_string()));
            env.push(("K7_ENGINE_TEST_UPLOAD_BYTES", u.max_bytes.to_string()));
            env.push(("K7_ENGINE_TEST_UPLOAD_HOUR", u.bytes_per_hour.to_string()));
            env.push(("K7_ENGINE_TEST_UPLOAD_IDLE_MS", u.idle_ms.to_string()));
        }
        let bed = Testbed::start(&env).await;
        let verify = match o.verify {
            None => json!({ "mode": "all" }),
            Some((id, token, secret)) => {
                json!({ "mode": "fixed", "id": id, "token": token, "secret": secret })
            }
        };
        bed.hook("verify", verify).await;
        let relay = Relay::start(bed.port).await;
        Hub { bed, relay }
    }

    pub fn port(&self) -> u16 {
        self.bed.port
    }

    /// What an agent dials: the hub through its cut-able relay.
    pub fn agent_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.relay.port)
    }

    pub async fn has(&self, id: &str) -> bool {
        self.bed.hook("hubHas", json!({ "id": id })).await == json!(true)
    }

    pub async fn until_has(&self, id: &str) {
        until_async(|| self.has(id), 10_000).await;
    }

    pub async fn bind_local(&self, id: &str, roots: Value, kind: &str, synced: bool) -> String {
        let path = self
            .bed
            .hook(
                "hubBind",
                json!({ "engineId": id, "roots": roots, "kind": kind, "synced": synced }),
            )
            .await;
        path.as_str().expect("a bound path").to_string()
    }

    pub async fn unbind_local(&self, path: &str) {
        self.bed.hook("hubUnbind", json!({ "path": path })).await;
    }

    /// A channel as the seam's callers open one: a one-shot binding, dialled on loopback; None when refused.
    pub async fn open_ready(&self, id: &str, roots: Value, kind: &str) -> Option<Client> {
        let path = self.bind_local(id, roots, kind, false).await;
        let c = Client::dial(self.port(), &path, None).await.ok();
        self.unbind_local(&path).await;
        c
    }

    pub fn lines(&self) -> Vec<String> {
        self.bed.lines.lock().unwrap().clone()
    }

    pub async fn stop(self) {
        drop(self.relay);
        self.bed.stop().await;
    }
}

/// A non-loopback IPv4 of this machine, to dial the hub as a peer that is not this machine's loopback.
pub fn lan_address() -> Option<Ipv4Addr> {
    // A UDP socket aimed past loopback names the interface a packet would leave by; nothing is sent.
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("203.0.113.1:9").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(a) if !a.is_loopback() => Some(a),
        _ => None,
    }
}

/// A WebSocket client of the hub on a bound path or a tunnel path: it keeps what it hears.
pub struct Client {
    tx: mpsc::UnboundedSender<Message>,
    pub texts: Arc<Mutex<Vec<String>>>,
    pub closed: Arc<AtomicBool>,
    done: Arc<Notify>,
    task: JoinHandle<()>,
}

impl Client {
    /// Dials `path` of the hub at `port`, from `local` when given; the refusal's status on Err (0: no socket).
    pub async fn dial(port: u16, path: &str, local: Option<Ipv4Addr>) -> Result<Client, u16> {
        Client::dial_with(port, path, local, &[]).await
    }

    pub async fn dial_with(
        port: u16,
        path: &str,
        local: Option<Ipv4Addr>,
        headers: &[(&str, &str)],
    ) -> Result<Client, u16> {
        // A peer that is not loopback dials the machine's own address from it.
        let host = local.unwrap_or(Ipv4Addr::LOCALHOST);
        let sock = TcpSocket::new_v4().map_err(|_| 0u16)?;
        if let Some(l) = local {
            sock.bind(SocketAddr::new(IpAddr::V4(l), 0))
                .map_err(|_| 0u16)?;
        }
        let stream = sock
            .connect(SocketAddr::new(IpAddr::V4(host), port))
            .await
            .map_err(|_| 0u16)?;
        let mut req = format!("ws://127.0.0.1:{port}{path}")
            .into_client_request()
            .map_err(|_| 0u16)?;
        for (k, v) in headers {
            req.headers_mut().insert(
                tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        let ws = match client_async(req, stream).await {
            Ok((ws, _)) => ws,
            Err(tokio_tungstenite::tungstenite::Error::Http(r)) => return Err(r.status().as_u16()),
            Err(_) => return Err(0),
        };
        Ok(Client::run(ws))
    }

    fn run<S>(ws: tokio_tungstenite::WebSocketStream<S>) -> Client
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let texts: Arc<Mutex<Vec<String>>> = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        let done = Arc::new(Notify::new());
        let (t, c, d) = (texts.clone(), closed.clone(), done.clone());
        let task = tokio::spawn(async move {
            let (mut sink, mut stream) = ws.split();
            loop {
                tokio::select! {
                    out = rx.recv() => match out {
                        Some(m) => if sink.send(m).await.is_err() { break },
                        None => break,
                    },
                    got = stream.next() => match got {
                        Some(Ok(Message::Text(x))) => t.lock().unwrap().push(x.as_str().to_string()),
                        Some(Ok(Message::Binary(b))) => t.lock().unwrap().push(String::from_utf8_lossy(&b).into_owned()),
                        Some(Ok(_)) => {}
                        _ => break,
                    },
                }
            }
            c.store(true, Ordering::SeqCst);
            d.notify_waiters();
        });
        Client {
            tx,
            texts,
            closed,
            done,
            task,
        }
    }

    pub fn send(&self, text: &str) {
        let _ = self.tx.send(Message::text(text));
    }

    /// A close frame, as a client that is done says it.
    pub fn close(&self) {
        let _ = self.tx.send(Message::Close(None));
    }

    pub fn heard(&self) -> Vec<String> {
        self.texts.lock().unwrap().clone()
    }

    pub async fn until_closed(&self, ms: u64) {
        until(|| self.closed.load(Ordering::SeqCst), ms).await;
        let _ = &self.done;
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The fake engine's sockets: each with the way to say something to it, and whether it ended.
pub type Peers = Arc<Mutex<Vec<(mpsc::UnboundedSender<Message>, Arc<AtomicBool>)>>>;

/// An engine: a WebSocket server that records the text it gets and can say text to its newest peer.
pub struct FakeEngine {
    pub port: u16,
    pub got: Arc<Mutex<Vec<String>>>,
    pub peers: Peers,
    /// How many frames the engine had heard when its first socket ended.
    pub closed_with: Arc<Mutex<Option<usize>>>,
    accept: JoinHandle<()>,
}

impl FakeEngine {
    pub async fn start() -> FakeEngine {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let got: Arc<Mutex<Vec<String>>> = Arc::default();
        let peers: Peers = Arc::default();
        let closed_with: Arc<Mutex<Option<usize>>> = Arc::default();
        let (g, p, cw) = (got.clone(), peers.clone(), closed_with.clone());
        let accept = tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let Ok(ws) = accept_async(sock).await else {
                    continue;
                };
                let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
                let closed = Arc::new(AtomicBool::new(false));
                p.lock().unwrap().push((tx, closed.clone()));
                let (g, cw) = (g.clone(), cw.clone());
                tokio::spawn(async move {
                    let (mut sink, mut stream) = ws.split();
                    loop {
                        tokio::select! {
                            out = rx.recv() => match out {
                                Some(m) => if sink.send(m).await.is_err() { break },
                                None => break,
                            },
                            m = stream.next() => match m {
                                Some(Ok(Message::Text(t))) => g.lock().unwrap().push(t.as_str().to_string()),
                                Some(Ok(_)) => {}
                                _ => break,
                            },
                        }
                    }
                    cw.lock().unwrap().get_or_insert(g.lock().unwrap().len());
                    closed.store(true, Ordering::SeqCst);
                });
            }
        });
        FakeEngine {
            port,
            got,
            peers,
            closed_with,
            accept,
        }
    }

    pub fn got(&self) -> Vec<String> {
        self.got.lock().unwrap().clone()
    }

    /// Says `text` to the newest peer.
    pub fn say(&self, text: &str) {
        let peers = self.peers.lock().unwrap();
        let _ = peers.last().expect("a peer").0.send(Message::text(text));
    }

    /// Closes the newest peer's socket from the engine's side.
    pub fn hang_up(&self) {
        let peers = self.peers.lock().unwrap();
        let _ = peers.last().expect("a peer").0.send(Message::Close(None));
    }

    /// Whether the `n`th peer's socket ended.
    pub fn peer_closed(&self, n: usize) -> bool {
        self.peers
            .lock()
            .unwrap()
            .get(n)
            .is_some_and(|p| p.1.load(Ordering::SeqCst))
    }
}

impl Drop for FakeEngine {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

/// An engine that reads every frame and never answers a close: the agent has to let go by itself.
pub struct StuckEngine {
    pub port: u16,
    pub texts: Arc<Mutex<Vec<String>>>,
    pub close_at: Arc<Mutex<Option<Instant>>>,
    pub end_at: Arc<Mutex<Option<Instant>>>,
    accept: JoinHandle<()>,
}

impl StuckEngine {
    pub async fn start() -> StuckEngine {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let texts: Arc<Mutex<Vec<String>>> = Arc::default();
        let close_at: Arc<Mutex<Option<Instant>>> = Arc::default();
        let end_at: Arc<Mutex<Option<Instant>>> = Arc::default();
        let (t, c, e) = (texts.clone(), close_at.clone(), end_at.clone());
        let accept = tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let (t, c, e) = (t.clone(), c.clone(), e.clone());
                tokio::spawn(async move {
                    let mut buf: Vec<u8> = vec![];
                    let mut chunk = [0u8; 8192];
                    let mut upgraded = false;
                    while let Ok(n) = sock.read(&mut chunk).await {
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if !upgraded {
                            let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                                continue;
                            };
                            let raw = String::from_utf8_lossy(&buf[..end]).into_owned();
                            let key = raw
                                .lines()
                                .find(|l| l.to_lowercase().starts_with("sec-websocket-key:"))
                                .map(|l| l[l.find(':').unwrap() + 1..].trim().to_string())
                                .unwrap_or_default();
                            let accept =
                                tokio_tungstenite::tungstenite::handshake::derive_accept_key(
                                    key.as_bytes(),
                                );
                            let reply = format!(
                                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
                            );
                            if sock.write_all(reply.as_bytes()).await.is_err() {
                                break;
                            }
                            upgraded = true;
                            buf.drain(..end + 4);
                        }
                        // Frames of a masked client: read them, answer none.
                        loop {
                            if buf.len() < 2 {
                                break;
                            }
                            let op = buf[0] & 0x0f;
                            let mut len = (buf[1] & 0x7f) as usize;
                            let mut at = 2;
                            if len == 126 {
                                if buf.len() < 4 {
                                    break;
                                }
                                len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
                                at = 4;
                            } else if len == 127 {
                                if buf.len() < 10 {
                                    break;
                                }
                                len = u64::from_be_bytes(buf[2..10].try_into().unwrap()) as usize;
                                at = 10;
                            }
                            let masked = buf[1] & 0x80 != 0;
                            let mask: [u8; 4] = if masked && buf.len() >= at + 4 {
                                buf[at..at + 4].try_into().unwrap()
                            } else {
                                [0; 4]
                            };
                            if masked {
                                at += 4;
                            }
                            if buf.len() < at + len {
                                break;
                            }
                            let payload: Vec<u8> = buf[at..at + len]
                                .iter()
                                .enumerate()
                                .map(|(i, b)| if masked { b ^ mask[i % 4] } else { *b })
                                .collect();
                            buf.drain(..at + len);
                            if op == 1 {
                                t.lock()
                                    .unwrap()
                                    .push(String::from_utf8_lossy(&payload).into_owned());
                            }
                            if op == 8 {
                                c.lock().unwrap().get_or_insert(Instant::now());
                            }
                        }
                    }
                    *e.lock().unwrap() = Some(Instant::now());
                });
            }
        });
        StuckEngine {
            port,
            texts,
            close_at,
            end_at,
            accept,
        }
    }

    pub fn types(&self) -> Vec<String> {
        self.texts
            .lock()
            .unwrap()
            .iter()
            .map(|g| {
                serde_json::from_str::<Value>(g).unwrap()["type"]
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    }
}

impl Drop for StuckEngine {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

/// What the real agent is given to dial a hub.
pub struct AgentSpec<'a> {
    pub url: &'a str,
    pub id: &'a str,
    pub token: &'a str,
    pub secret: &'a str,
    pub engine_port: u16,
    pub cache: PathBuf,
    pub protocol: Option<i64>,
}

/// The machine's tunnel agent, with the two test jobs the old reference agent answered (`where`, `upper`).
pub fn agent(spec: AgentSpec) -> (TunnelAgent, Arc<Mutex<Vec<String>>>) {
    let mut o = Options::new(spec.url, spec.id, spec.token, spec.cache);
    o.tunnel_secret = spec.secret.into();
    let port = spec.engine_port;
    o.engine_port = Arc::new(move || port);
    if let Some(p) = spec.protocol {
        o.protocol = p;
    }
    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = log.clone();
    o.log = Arc::new(move |m| sink.lock().unwrap().push(m.to_string()));
    // Answers one path twice: once mapped back to the server's, once under `raw` as the machine's.
    o.jobs.add("where", |req, emit, _| async move {
        let source = req["source"].as_str().unwrap_or("").to_string();
        emit.send(json!({"type": "where", "path": source, "raw": {"path": source}}));
        Ok(())
    });
    // Reads its source, writes one "stem" beside it, reporting progress first.
    o.jobs.add("upper", |req, emit, _| async move {
        let source = req["source"].as_str().unwrap_or("").to_string();
        let out_dir = req["outDir"].as_str().unwrap_or("").to_string();
        let upper = std::fs::read_to_string(&source)
            .map_err(|e| e.to_string())?
            .to_uppercase();
        emit.send(json!({"type": "progress", "percent": 50}));
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let name = std::path::Path::new(&source)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let out = std::path::Path::new(&out_dir).join(format!("{name}.up"));
        std::fs::write(&out, upper).map_err(|e| e.to_string())?;
        emit.send(json!({"type": "done", "files": [out.to_string_lossy()]}));
        Ok(())
    });
    (TunnelAgent::start(o), log)
}

/// One frame a bare agent heard: its header and, for a binary frame, the payload.
#[derive(Clone, Debug)]
pub struct Heard {
    pub head: Value,
    pub payload: Vec<u8>,
}

/// A bare supervisor speaking the tunnel's frames by hand, so a suite can send what no agent would.
pub struct Bare {
    tx: mpsc::UnboundedSender<Message>,
    heard: Arc<Mutex<Vec<Heard>>>,
    seen_opens: usize,
    req: i64,
    task: JoinHandle<()>,
}

impl Bare {
    /// A bare supervisor for engine `id`, dialled into `url`'s tunnel path.
    pub async fn dial(url_port: u16, id: &str, token: &str) -> Bare {
        let stream = TcpStream::connect(("127.0.0.1", url_port)).await.unwrap();
        let mut req = format!("ws://127.0.0.1:{url_port}{TUNNEL_PATH}?id={id}")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("x-studio-engine-token", token.parse().unwrap());
        let (ws, _) = client_async(req, stream)
            .await
            .unwrap_or_else(|e| panic!("refused: {e}"));
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let heard: Arc<Mutex<Vec<Heard>>> = Arc::default();
        let h = heard.clone();
        let task = tokio::spawn(async move {
            let (mut sink, mut stream) = ws.split();
            loop {
                tokio::select! {
                    out = rx.recv() => match out {
                        Some(m) => if sink.send(m).await.is_err() { break },
                        None => break,
                    },
                    got = stream.next() => match got {
                        Some(Ok(Message::Text(t))) => h.lock().unwrap().push(Heard {
                            head: serde_json::from_str(t.as_str()).unwrap_or(Value::Null),
                            payload: vec![],
                        }),
                        Some(Ok(Message::Binary(b))) if b.len() >= 4 => {
                            let len = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
                            let head = serde_json::from_slice(&b[4..4 + len]).unwrap_or(Value::Null);
                            h.lock().unwrap().push(Heard { head, payload: b[4 + len..].to_vec() });
                        }
                        Some(Ok(_)) => {}
                        _ => break,
                    },
                }
            }
        });
        Bare {
            tx,
            heard,
            seen_opens: 0,
            req: 1000,
            task,
        }
    }

    async fn next<T>(&self, pick: impl Fn(&[Heard]) -> Option<T>) -> T {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(v) = pick(&self.heard.lock().unwrap()) {
                return v;
            }
            assert!(Instant::now() < end, "the hub did not answer");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// The next channel the hub opens, answered `opened`.
    pub async fn opened(&mut self) -> i64 {
        let n = self.seen_opens;
        let m = self
            .next(|h| h.iter().filter(|x| x.head["t"] == "open").nth(n).cloned())
            .await;
        self.seen_opens += 1;
        let ch = m.head["ch"].as_i64().unwrap();
        let _ = self
            .tx
            .send(Message::text(json!({"t": "opened", "ch": ch}).to_string()));
        ch
    }

    pub async fn stat(&mut self, ch: i64, paths: &[String]) -> Vec<Value> {
        let r = self.req;
        self.req += 1;
        let _ = self.tx.send(Message::text(
            json!({"t": "stat", "req": r, "ch": ch, "paths": paths}).to_string(),
        ));
        let m = self
            .next(|h| {
                h.iter()
                    .find(|x| x.head["t"] == "stat-reply" && x.head["req"] == r)
                    .cloned()
            })
            .await;
        m.head["entries"].as_array().cloned().unwrap_or_default()
    }

    /// A file's bytes, or the hub's refusal.
    pub async fn get(&mut self, ch: i64, path: &str) -> Result<Vec<u8>, String> {
        let r = self.req;
        self.req += 1;
        let _ = self.tx.send(Message::text(
            json!({"t": "get", "req": r, "ch": ch, "path": path}).to_string(),
        ));
        self.next(|h| {
            if let Some(e) = h
                .iter()
                .find(|x| x.head["t"] == "get-error" && x.head["req"] == r)
            {
                return Some(Err(e.head["error"].to_string()));
            }
            let chunks: Vec<&Heard> = h
                .iter()
                .filter(|x| x.head["t"] == "chunk" && x.head["req"] == r)
                .collect();
            if !chunks.iter().any(|c| c.head["last"] == true) {
                return None;
            }
            Some(Ok(chunks.iter().flat_map(|c| c.payload.clone()).collect()))
        })
        .await
    }

    /// One raw upload chunk, its reply not awaited.
    pub fn put_chunk(&self, ch: i64, req: i64, path: &str, chunk: &[u8], last: bool) {
        let head = json!({"t": "put", "req": req, "ch": ch, "path": path, "last": last});
        let _ = self.tx.send(Message::binary(encode_binary(&head, chunk)));
    }

    /// Sends `chunks` as one upload, the last one marked; the hub's reply (its error, its stat).
    pub async fn put(&mut self, ch: i64, path: &str, chunks: &[&[u8]]) -> (Option<String>, Value) {
        let r = self.req;
        self.req += 1;
        for (i, c) in chunks.iter().enumerate() {
            self.put_chunk(ch, r, path, c, i == chunks.len() - 1);
        }
        let m = self
            .next(|h| {
                h.iter()
                    .find(|x| x.head["t"] == "put-reply" && x.head["req"] == r)
                    .cloned()
            })
            .await;
        (
            m.head["error"].as_str().map(str::to_string),
            m.head["stat"].clone(),
        )
    }

    pub fn replies(&self, t: &str) -> Vec<Value> {
        self.heard
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.head["t"] == t)
            .map(|h| h.head.clone())
            .collect()
    }

    /// The socket dropped with no close frame.
    pub fn close(&self) {
        self.task.abort();
    }
}

impl Drop for Bare {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A handshake by hand on `path`: the stream, and the head of the answer (its status line and headers).
async fn handshake(port: u16, path: &str, headers: &[(&str, &str)]) -> (TcpStream, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let extra: String = headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}\r\n"))
        .collect();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n{extra}\r\n"
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut head = vec![];
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if s.read(&mut b).await.unwrap() == 0 {
            break;
        }
        head.push(b[0]);
    }
    (s, String::from_utf8_lossy(&head).into_owned())
}

/// The tunnel upgrade's answer to a bare client sending `headers`: its status and its body.
pub async fn upgrade(port: u16, headers: &[(&str, &str)]) -> (u16, String) {
    let mut all = vec![("x-studio-engine-token", "t")];
    all.extend_from_slice(headers);
    let (mut s, head) = handshake(port, &format!("{TUNNEL_PATH}?id=bare"), &all).await;
    let status: u16 = head[9..12].parse().unwrap();
    let mut body = String::new();
    if status != 101 {
        let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_string(&mut body)).await;
    }
    (status, body)
}

/// A handshake on `path`, then one frame with RSV1 set, which no extension negotiated makes a protocol fault:
/// the answer's status line once the server closed the socket.
pub async fn send_bad_frame(port: u16, path: &str, headers: &[(&str, &str)]) -> String {
    let (mut s, head) = handshake(port, path, headers).await;
    // FIN + RSV1 + text, masked, an empty payload.
    s.write_all(&[0xc1, 0x80, 1, 2, 3, 4]).await.unwrap();
    let mut rest = vec![];
    tokio::time::timeout(Duration::from_secs(3), s.read_to_end(&mut rest))
        .await
        .expect("the server kept the socket open")
        .ok();
    head.lines().next().unwrap_or("").to_string()
}
