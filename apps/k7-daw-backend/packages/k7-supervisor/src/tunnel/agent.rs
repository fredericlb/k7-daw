//! agent.rs — the ENGINE machine's end of the tunnel: one outbound socket to the server that lists the engine,
//! redialled with backoff until stopped, and for every channel the server opens either a real socket to the local
//! engine or a jobs channel this agent answers itself. The reader never waits on a download: queues and tasks do.
//! What a reader gets wrong: any frame from the hub, not only a pong, proves the tunnel alive.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{WebSocketStream, client_async};

use super::channel::{Channel, Payload};
use super::files::{ChannelFiles, FileRequests};
use super::flag::raised;
use super::jobs::Jobs;
use super::ledger::SyncLedger;
use super::link::{Link, Out};
use super::path_map::{Log, MappedRoot, PathMap};
use super::wire::{
    BinaryHeader, HubMessage, TUNNEL_PROTOCOL_HEADER, TUNNEL_SECRET_HEADER, TUNNEL_TOKEN_HEADER,
    binary_frame_type, decode_binary, parse_hub_message, text_frame_type, tunnel_url,
};

const MAX_BACKOFF_MS: u64 = 30_000;
const PING: Duration = Duration::from_secs(20);
/// Dialing the hub, handshake included.
const DIAL: Duration = Duration::from_secs(15);

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
type Socket = WebSocketStream<Box<dyn Io>>;

pub struct Options {
    /// The registry's base URL; the tunnel is `/api/engine-tunnel` under it.
    pub registry_url: String,
    pub id: String,
    pub token: String,
    /// The machine's tunnel secret, sent in a header; empty, the hub takes this as a legacy tunnel.
    pub tunnel_secret: String,
    /// The engine's port, asked at each channel: a relaunch may move it.
    pub engine_port: Arc<dyn Fn() -> u16 + Send + Sync>,
    /// Where this server's files are kept on this machine.
    pub cache_dir: PathBuf,
    pub jobs: Jobs,
    pub log: Log,
    /// The TLS configuration of an `https` registry; the shared one when None.
    pub tls: Option<Arc<rustls::ClientConfig>>,
    /// The engine wire's version stated on the upgrade; the supervisor's own unless a test plays another machine.
    pub protocol: i64,
    /// How often a ping goes; a tunnel that missed one is dropped.
    pub ping: Duration,
}

impl Options {
    pub fn new(registry_url: &str, id: &str, token: &str, cache_dir: PathBuf) -> Options {
        Options {
            registry_url: registry_url.to_string(),
            id: id.to_string(),
            token: token.to_string(),
            tunnel_secret: String::new(),
            engine_port: Arc::new(|| 8765),
            cache_dir,
            jobs: Jobs::default(),
            log: Arc::new(|_| {}),
            tls: None,
            protocol: crate::wire::ENGINE_PROTOCOL,
            ping: PING,
        }
    }
}

/// What the agent's tasks share: the link, the files asked of the hub, and the channels open.
pub struct Shared {
    pub(super) opts: Options,
    pub(super) link: Arc<Link>,
    files: Arc<FileRequests>,
    ledger: Arc<SyncLedger>,
    channels: Mutex<HashMap<i64, Arc<Channel>>>,
    ignored: Mutex<HashSet<String>>,
    up: AtomicBool,
}

pub struct TunnelAgent {
    stop: watch::Sender<bool>,
    shared: Arc<Shared>,
}

/// Wait before redialling: full jitter over an exponential step, so a restarted server is not hit by every engine at once.
pub fn backoff(attempt: u32, random: u32) -> Duration {
    let step = MAX_BACKOFF_MS.min(1000u64 << attempt.min(5));
    Duration::from_millis(step / 2 + u64::from(random) % (step / 2 + 1))
}

fn random_u32() -> u32 {
    let mut b = [0u8; 4];
    let _ = getrandom::fill(&mut b);
    u32::from_le_bytes(b)
}

fn safe_name(name: &str) -> bool {
    let mut cs = name.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && cs.all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

impl TunnelAgent {
    /// Dial, and keep redialling with backoff until `stop`; inside a tokio runtime.
    pub fn start(opts: Options) -> TunnelAgent {
        let link = Arc::new(Link::default());
        let shared = Arc::new(Shared {
            files: Arc::new(FileRequests::new(link.clone())),
            link,
            opts,
            ledger: Arc::default(),
            channels: Mutex::default(),
            ignored: Mutex::default(),
            up: AtomicBool::new(false),
        });
        let (stop, rx) = watch::channel(false);
        tokio::spawn(shared.clone().life(rx));
        TunnelAgent { stop, shared }
    }

    /// Let go: returns at once, the tasks finishing on their own.
    pub fn stop(&self) {
        self.stop.send_replace(true);
    }

    /// Whether the hub accepted us last time.
    pub fn connected(&self) -> bool {
        self.shared.up.load(Ordering::SeqCst)
    }
}

impl Drop for TunnelAgent {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Shared {
    pub(super) fn channel(&self, ch: i64) -> Option<Arc<Channel>> {
        self.channels().get(&ch).cloned()
    }

    fn channels(&self) -> MutexGuard<'_, HashMap<i64, Arc<Channel>>> {
        self.channels.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Takes `c` out of the map if it is still the hub's channel; false when the hub already let go.
    pub(super) fn take_channel(&self, c: &Arc<Channel>) -> bool {
        let mut map = self.channels();
        if map.get(&c.ch).is_some_and(|m| Arc::ptr_eq(m, c)) {
            map.remove(&c.ch);
            return true;
        }
        false
    }

    pub(super) fn tell(&self, m: &Value) {
        self.link.send_text(m.to_string());
    }

    async fn life(self: Arc<Self>, mut stop: watch::Receiver<bool>) {
        // The handshake carries the token and the tunnel secret, so a plain ws past loopback is never dialled.
        let why = crate::urls::plaintext_refusal(&self.opts.registry_url);
        if !why.is_empty() {
            (self.opts.log)(&format!("tunnel not dialled: {why}"));
            return;
        }
        let Some(url) = tunnel_url(&self.opts.registry_url, &self.opts.id) else {
            (self.opts.log)(&format!(
                "tunnel not dialled: {} is not a web address",
                self.opts.registry_url
            ));
            return;
        };
        let (mut attempt, mut last_fail) = (0u32, String::new());
        let mut inner = stop.clone();
        while !*stop.borrow() {
            let why = tokio::select! {
                _ = raised(&mut stop) => break,
                why = self.connection(&url, &mut attempt, &mut inner) => why,
            };
            self.teardown();
            // WHY, once per reason: a server down for an hour redials a hundred times and says it once.
            if let Some(why) = why.filter(|w| *w != last_fail) {
                (self.opts.log)(&format!("tunnel to {}: {why}", self.opts.registry_url));
                last_fail = why;
            }
            let wait = backoff(attempt, random_u32());
            attempt += 1;
            tokio::select! {
                _ = raised(&mut stop) => break,
                _ = sleep(wait) => {}
            }
        }
        self.teardown();
    }

    fn teardown(&self) {
        self.link.detach();
        if self.up.swap(false, Ordering::SeqCst) {
            (self.opts.log)(&format!("down from {}", self.opts.registry_url));
        }
        self.files.fail_all();
        let gone: Vec<Arc<Channel>> = self.channels().drain().map(|(_, c)| c).collect();
        gone.iter().for_each(|c| c.shut());
    }

    async fn dial(&self, url: &str) -> Result<Socket, String> {
        let mut req = url.into_client_request().map_err(|e| e.to_string())?;
        let mut put = |name: &str, value: &str| -> Result<(), String> {
            let v = HeaderValue::from_str(value).map_err(|e| e.to_string())?;
            req.headers_mut().insert(
                HeaderName::from_bytes(name.as_bytes()).map_err(|e| e.to_string())?,
                v,
            );
            Ok(())
        };
        put(TUNNEL_TOKEN_HEADER, &self.opts.token)?;
        put(TUNNEL_PROTOCOL_HEADER, &self.opts.protocol.to_string())?;
        if !self.opts.tunnel_secret.is_empty() {
            put(TUNNEL_SECRET_HEADER, &self.opts.tunnel_secret)?;
        }
        let uri = req.uri().clone();
        let host = uri.host().unwrap_or_default().to_string();
        let secure = uri.scheme_str() == Some("wss");
        let port = uri.port_u16().unwrap_or(if secure { 443 } else { 80 });
        let shake = async {
            let tcp = TcpStream::connect((host.trim_matches(['[', ']']), port))
                .await
                .map_err(|e| e.to_string())?;
            let io: Box<dyn Io> = if secure {
                let cfg = self
                    .opts
                    .tls
                    .clone()
                    .unwrap_or_else(crate::trust::client_config);
                let name = rustls::pki_types::ServerName::try_from(
                    host.trim_matches(['[', ']']).to_string(),
                )
                .map_err(|e| e.to_string())?;
                Box::new(
                    tokio_rustls::TlsConnector::from(cfg)
                        .connect(name, tcp)
                        .await
                        .map_err(|e| e.to_string())?,
                )
            } else {
                Box::new(tcp)
            };
            client_async(req, io)
                .await
                .map(|(ws, _)| ws)
                .map_err(|e| match e {
                    WsError::Http(r) => {
                        let body = r.body().as_deref().map(|b| {
                            String::from_utf8_lossy(b)
                                .chars()
                                .take(200)
                                .collect::<String>()
                        });
                        format!(
                            "refused with HTTP {} {}",
                            r.status().as_u16(),
                            body.unwrap_or_default()
                        )
                        .trim_end()
                        .to_string()
                    }
                    other => other.to_string(),
                })
        };
        timeout(DIAL, shake)
            .await
            .unwrap_or_else(|_| Err("no answer".into()))
    }

    /// One connection, from its dial to its end; the reason it ended, if one is worth saying.
    async fn connection(
        self: &Arc<Self>,
        url: &str,
        attempt: &mut u32,
        stop: &mut watch::Receiver<bool>,
    ) -> Option<String> {
        let ws = match self.dial(url).await {
            Ok(ws) => ws,
            Err(why) => return Some(why),
        };
        *attempt = 0;
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Out>();
        self.link.attach(tx);
        self.up.store(true, Ordering::SeqCst);
        (self.opts.log)(&format!("up to {}", self.opts.registry_url));
        let link = self.link.clone();
        let mut writer = tokio::spawn(async move {
            while let Some(out) = rx.recv().await {
                let (msg, n) = match out {
                    Out::Text(t) => {
                        let n = t.len();
                        (Message::text(t), n)
                    }
                    Out::Binary(b) => {
                        let n = b.len();
                        (Message::binary(b), n)
                    }
                    Out::Ping => (Message::Ping(Vec::new().into()), 0),
                };
                sink.send(msg).await.map_err(|e| e.to_string())?;
                link.written(n);
            }
            Ok::<(), String>(())
        });
        let mut tick = tokio::time::interval(self.opts.ping);
        tick.tick().await;
        let mut alive = true;
        let why = loop {
            tokio::select! {
                _ = raised(stop) => break None,
                done = &mut writer => break Some(done.ok().and_then(Result::err).unwrap_or_else(|| "the writer ended".into())),
                _ = tick.tick() => {
                    if !alive {
                        break Some("no answer to a ping".into());
                    }
                    alive = false;
                    self.link.send_ping();
                }
                got = stream.next() => {
                    alive = true;
                    match got {
                        Some(Ok(Message::Text(t))) => self.on_text(t.as_str()),
                        Some(Ok(Message::Binary(b))) => self.on_binary(&b).await,
                        Some(Ok(Message::Close(f))) => {
                            break Some(match f {
                                Some(f) => format!("closed {} {}", u16::from(f.code), f.reason).trim_end().to_string(),
                                None => "closed".into(),
                            });
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => break Some(e.to_string()),
                        None => break Some("connection lost".into()),
                    }
                }
            }
        };
        writer.abort();
        why
    }

    /// Said once per type that a frame was dropped, so a hub newer than this agent shows in the log.
    fn unknown_frame(&self, kind: &str, t: &str) {
        let fresh = self
            .ignored
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(format!("{kind}:{t}"));
        if fresh {
            (self.opts.log)(&format!(
                "ignored a {kind} frame of type '{t}' from {} — this supervisor does not read it",
                self.opts.registry_url
            ));
        }
    }

    fn on_text(self: &Arc<Self>, text: &str) {
        let m = match parse_hub_message(text) {
            Ok(m) => m,
            Err(t) => return self.unknown_frame("text", &t),
        };
        match m {
            HubMessage::Open { ch, roots, jobs, synced } => self.open_channel(ch, &roots, jobs, synced),
            HubMessage::Close { ch } => {
                let gone = self.channels().remove(&ch);
                if let Some(c) = gone {
                    self.close_engine(&c);
                }
            }
            HubMessage::Data { ch, text } => self.to_engine(ch, Payload::Text(text)),
            HubMessage::StatReply { req, entries } => self.files.on_stat_reply(req, &entries),
            HubMessage::PutReply { req, stat, error } => self.files.on_put_reply(req, &stat, &error),
            HubMessage::GetError { req, error } => self.files.on_get_error(req, &error),
            // WebRTC is not carried: the browser stays on the tunnel.
            HubMessage::RtcOffer { req } => self.tell(&json!({"t": "rtc-answer", "req": req, "error": "WebRTC is not available on this machine"})),
        }
    }

    async fn on_binary(self: &Arc<Self>, frame: &[u8]) {
        let Some((header, start)) = decode_binary(frame) else {
            return self.unknown_frame("binary", &binary_frame_type(frame));
        };
        match header {
            BinaryHeader::Chunk { req, last } => {
                self.files.on_chunk(req, last, &frame[start..]).await
            }
            BinaryHeader::Data { ch } => self.to_engine(ch, Payload::Bin(frame[start..].to_vec())),
            BinaryHeader::Put { .. } => self.unknown_frame(
                "binary",
                &text_frame_type(&String::from_utf8_lossy(&frame[4..start])),
            ),
        }
    }

    fn open_channel(
        self: &Arc<Self>,
        ch: i64,
        roots: &[super::wire::OpenRoot],
        jobs: bool,
        synced: bool,
    ) {
        if roots.iter().any(|r| !safe_name(&r.name)) {
            return self.tell(&json!({"t": "close", "ch": ch, "reason": "refused a root name"}));
        }
        let mapped: Vec<MappedRoot> = roots
            .iter()
            .map(|r| MappedRoot {
                remote: r.remote.clone(),
                local: self
                    .opts
                    .cache_dir
                    .join(&r.name)
                    .to_string_lossy()
                    .into_owned(),
                writable: r.writable,
            })
            .collect();
        let map = (!mapped.is_empty()).then(|| {
            let files = Box::new(ChannelFiles {
                reqs: self.files.clone(),
                ch,
            });
            Arc::new(PathMap::new(
                mapped,
                files,
                self.ledger.clone(),
                self.opts.log.clone(),
            ))
        });
        let (c, engine_rx) = Channel::new(ch, jobs, synced && !jobs, map);
        // A reused id replaces its channel, whose engine socket would otherwise stay open with nothing to reap it.
        let previous = self.channels().insert(ch, c.clone());
        if let Some(p) = previous {
            p.shut();
        }
        if jobs {
            return self.tell(&json!({"t": "opened", "ch": ch}));
        }
        // Which of the two copy rules this channel follows is the first thing a lag report needs.
        let rule = if synced {
            "synced, never fetches"
        } else {
            "fetches before each frame"
        };
        (self.opts.log)(&format!("ch{ch} to the engine: {rule}"));
        tokio::spawn(self.clone().engine_socket(c, engine_rx));
    }
}
