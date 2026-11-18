//! The scripted engine and jobs agent the link's tests dial: real sockets on loopback, frames recorded per connection,
//! answers on a script. What a reader gets wrong: a fake engine holds a session LOADED only `loaded_after` past its project.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use super::held;

/// A frame the fake engine read.
#[derive(Clone, Debug)]
pub struct Got {
    /// Which connection read it, for a test that tells a re-dial apart.
    #[allow(dead_code)]
    pub conn: u64,
    pub at: Instant,
    pub frame: Value,
}

#[derive(Default)]
struct Script {
    loaded_after: Duration,
    /// Answers to nothing at all.
    silent: bool,
    /// Frames sent in answer to a `plugin-scan` question.
    scan: Vec<Value>,
    plugins: Vec<Value>,
    devices: Vec<Value>,
    /// A rescan says its list at once and that no scan runs this long after.
    rescan_after: Duration,
    /// An arm is refused with this sentence instead of answered.
    arm_error: Option<String>,
    /// An arm is read and never answered.
    arm_mute: bool,
}

#[derive(Default)]
struct Shared {
    got: Mutex<Vec<Got>>,
    conns: Mutex<HashMap<u64, mpsc::UnboundedSender<Message>>>,
    project_at: Mutex<HashMap<String, Instant>>,
    script: Mutex<Script>,
    queries: Mutex<super::fake_plugins::Script>,
    accepted: AtomicU64,
    dials: AtomicU64,
    refusing: AtomicBool,
    slamming: AtomicBool,
    /// Accepted and never read from: a stalled engine.
    stalling: AtomicBool,
}

pub struct FakeEngine {
    pub url: String,
    pub port: u16,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for FakeEngine {
    fn drop(&mut self) {
        self.task.abort();
        self.close_all();
    }
}

fn session_of(frame: &Value) -> String {
    frame
        .get("session")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

impl FakeEngine {
    pub async fn start() -> FakeEngine {
        FakeEngine::on(0).await
    }

    /// On `port` (0 for any), so a killed engine can come back where it was.
    pub async fn on(port: u16) -> FakeEngine {
        let listener = {
            let mut tries = 0;
            loop {
                match TcpListener::bind(("127.0.0.1", port)).await {
                    Ok(l) => break l,
                    Err(e) if tries < 50 => {
                        tries += 1;
                        let _ = e;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => panic!("the fake engine's port: {e}"),
                }
            }
        };
        let port = listener.local_addr().unwrap().port();
        let shared = Arc::new(Shared::default());
        let s = shared.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                s.dials.fetch_add(1, Ordering::Relaxed);
                if s.refusing.load(Ordering::Relaxed) {
                    continue;
                }
                let s = s.clone();
                tokio::spawn(async move { serve(tcp, s).await });
            }
        });
        FakeEngine {
            url: format!("ws://127.0.0.1:{port}"),
            port,
            shared,
            task,
        }
    }

    /// Stop listening and close what is open, as a killed engine does; the port is free again for `on`.
    pub fn stop(&self) {
        self.task.abort();
        self.close_all();
    }

    pub fn loaded_after(&self, d: Duration) {
        held(&self.shared.script).loaded_after = d;
    }

    pub fn silent(&self, on: bool) {
        held(&self.shared.script).silent = on;
    }

    /// What the fake says to the three plugin queries.
    pub fn queries(&self, f: impl FnOnce(&mut super::fake_plugins::Script)) {
        f(&mut held(&self.shared.queries));
    }

    pub fn answer_scan_with(&self, frames: Vec<Value>) {
        held(&self.shared.script).scan = frames;
    }

    pub fn devices(&self, list: Vec<Value>) {
        held(&self.shared.script).devices = list;
    }

    pub fn rescan_after(&self, d: Duration) {
        held(&self.shared.script).rescan_after = d;
    }

    pub fn mute_arm(&self) {
        held(&self.shared.script).arm_mute = true;
    }

    pub fn refuse_arm(&self, message: &str) {
        held(&self.shared.script).arm_error = Some(message.to_owned());
    }

    pub fn plugins(&self, list: Vec<Value>) {
        held(&self.shared.script).plugins = list;
    }

    /// Every frame read so far, in arrival order.
    pub fn got(&self) -> Vec<Got> {
        held(&self.shared.got).clone()
    }

    /// The frames of one session, as `[type, ...]` values.
    pub fn of(&self, session: &str) -> Vec<Value> {
        self.got()
            .into_iter()
            .filter(|g| session_of(&g.frame) == session)
            .map(|g| g.frame)
            .collect()
    }

    pub fn kinds(&self, session: &str) -> Vec<String> {
        self.of(session)
            .iter()
            .map(|f| {
                let t = f["type"].as_str().unwrap_or("").to_string();
                match f.get("action").and_then(Value::as_str) {
                    Some(a) => format!("{t} {a}"),
                    None => t,
                }
            })
            .collect()
    }

    /// Every TCP connection tried, handshaken or not.
    pub fn dials(&self) -> u64 {
        self.shared.dials.load(Ordering::Relaxed)
    }

    pub fn accepted(&self) -> u64 {
        self.shared.accepted.load(Ordering::Relaxed)
    }

    /// Close every connection from this side, as a killed engine does.
    pub fn close_all(&self) {
        for (_, tx) in held(&self.shared.conns).drain() {
            let _ = tx.send(Message::Close(None));
        }
    }

    pub fn send_all(&self, frame: Value) {
        for tx in held(&self.shared.conns).values() {
            let _ = tx.send(Message::text(frame.to_string()));
        }
    }

    /// Accept connections and drop them at once, as an engine that closes right after opening.
    pub fn slam(&self, on: bool) {
        self.shared.slamming.store(on, Ordering::Relaxed);
    }

    /// Complete the upgrade and then read nothing: the writer's socket buffers fill.
    pub fn stall(&self, on: bool) {
        self.shared.stalling.store(on, Ordering::Relaxed);
    }

    pub fn refuse(&self, on: bool) {
        self.shared.refusing.store(on, Ordering::Relaxed);
    }
}

async fn serve(tcp: tokio::net::TcpStream, s: Arc<Shared>) {
    let Ok(ws) = tokio_tungstenite::accept_async(tcp).await else {
        return;
    };
    let conn = s.accepted.fetch_add(1, Ordering::Relaxed) + 1;
    if s.stalling.load(Ordering::Relaxed) {
        std::future::pending::<()>().await;
    }
    let (mut write, mut read) = ws.split();
    if s.slamming.load(Ordering::Relaxed) {
        let _ = write.send(Message::Close(None)).await;
        return;
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    held(&s.conns).insert(conn, tx.clone());
    tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            let end = matches!(m, Message::Close(_));
            if write.send(m).await.is_err() || end {
                break;
            }
        }
    });
    while let Some(Ok(m)) = read.next().await {
        let Message::Text(t) = m else { continue };
        let Ok(frame) = serde_json::from_str::<Value>(t.as_str()) else {
            continue;
        };
        held(&s.got).push(Got {
            conn,
            at: Instant::now(),
            frame: frame.clone(),
        });
        answer(&s, &frame, &tx);
    }
    held(&s.conns).remove(&conn);
}

fn answer(s: &Shared, frame: &Value, tx: &mpsc::UnboundedSender<Message>) {
    let say = |v: Value| {
        let _ = tx.send(Message::text(v.to_string()));
    };
    let session = session_of(frame);
    match frame["type"].as_str().unwrap_or("") {
        "project" => {
            held(&s.project_at).insert(session, Instant::now());
        }
        "drop-session" => {
            let who = frame["session"].as_str().unwrap_or("").to_string();
            held(&s.project_at).remove(&who);
        }
        "list-sessions" => {
            let script = held(&s.script);
            if script.silent {
                return;
            }
            let now = Instant::now();
            let rows: Vec<Value> = held(&s.project_at)
                .iter()
                .map(|(id, at)| json!({ "id": id, "loaded": now.duration_since(*at) >= script.loaded_after }))
                .collect();
            say(json!({ "type": "sessions", "sessions": rows }));
        }
        "plugin-scan" => {
            let script = held(&s.script);
            if script.silent {
                return;
            }
            let frames = if script.scan.is_empty() {
                vec![
                    json!({ "type": "plugin-scan", "scanning": false, "done": 0, "total": 0, "version": 1 }),
                ]
            } else {
                script.scan.clone()
            };
            drop(script);
            frames.into_iter().for_each(say);
        }
        "arm" if held(&s.script).arm_mute => {}
        "arm" => match held(&s.script).arm_error.clone() {
            Some(message) => say(json!({ "type": "error", "message": message })),
            None => say(json!({
                "type": "armed", "track": frame["track"], "armed": frame.get("armed").and_then(Value::as_bool).unwrap_or(true),
            })),
        },
        "list-plugin-params" | "show-native-plugin-ui-of" | "snapshot-plugin-base" => {
            if held(&s.script).silent {
                return;
            }
            for f in super::fake_plugins::answers(&held(&s.queries), frame) {
                say(f);
            }
        }
        "list-audio-devices" => {
            let devices = held(&s.script).devices.clone();
            say(json!({ "type": "audio-devices", "devices": devices }));
        }
        "rescan-plugins" => {
            let (plugins, after) = {
                let script = held(&s.script);
                (script.plugins.clone(), script.rescan_after)
            };
            say(json!({ "type": "plugins", "plugins": plugins }));
            let end = json!({ "type": "plugin-scan", "scanning": false, "done": 0, "total": 0, "version": 1 });
            if after.is_zero() {
                say(end);
            } else {
                let tx = tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(after).await;
                    let _ = tx.send(Message::text(end.to_string()));
                });
            }
        }
        "list-plugins" => {
            let plugins = held(&s.script).plugins.clone();
            say(json!({ "type": "plugins", "plugins": plugins }));
        }
        _ => {}
    }
}

/// What a `sync` job ends in.
#[derive(Clone)]
pub enum JobEnd {
    Synced {
        failed: u64,
        failures: Vec<String>,
    },
    Error(String),
    /// A `render` request is answered with a mix and these stems, written under its `outDir`.
    // Built by tests that run only on Unix.
    #[cfg_attr(not(unix), allow(dead_code))]
    Rendered {
        stems: Vec<String>,
    },
    /// A freeze's render writes the file it was asked for, then fails: the half file a machine leaves behind.
    #[cfg_attr(not(unix), allow(dead_code))]
    WroteThenError(String),
    /// A `separate` request is answered with one running record, then this one as the finished job.
    #[cfg_attr(not(unix), allow(dead_code))]
    Separated {
        job: Value,
    },
    /// Never answers.
    Never,
}

/// The answer to a `render` request on the script: the files are really written, as a machine writes under its root.
fn rendered(req: &Value, end: &JobEnd) -> Option<Value> {
    let id = req["id"].clone();
    match end {
        JobEnd::Rendered { .. } if req["out"].is_string() => {
            // A freeze's render: the machine writes the one file it was asked for.
            std::fs::write(req["out"].as_str()?, b"RIFFfake").ok()?;
            Some(json!({ "type": "rendered", "id": id }))
        }
        JobEnd::Rendered { stems } => {
            let dir = std::path::PathBuf::from(req["outDir"].as_str()?);
            std::fs::create_dir_all(&dir).ok()?;
            let mix = dir.join("mix.wav");
            std::fs::write(&mix, b"RIFFfake").ok()?;
            let mut list = Vec::new();
            for name in stems.iter().filter(|_| req["stems"] == true) {
                let file = dir.join(format!("{name}.wav"));
                std::fs::write(&file, b"RIFFstem").ok()?;
                list.push(json!({ "name": name, "file": file.display().to_string() }));
            }
            Some(
                json!({ "type": "rendered", "id": id, "mix": mix.display().to_string(), "stems": list }),
            )
        }
        JobEnd::WroteThenError(m) => {
            std::fs::write(req["out"].as_str()?, b"RIFFhalf").ok()?;
            Some(json!({ "type": "error", "id": id, "error": m }))
        }
        JobEnd::Error(m) => Some(json!({ "type": "error", "id": id, "error": m })),
        JobEnd::Separated { .. } | JobEnd::Never => None,
        JobEnd::Synced { .. } => {
            Some(json!({ "type": "error", "id": id, "error": "no render scripted" }))
        }
    }
}

/// The frames a `separate` request gets: a record in progress and the last one, or what the script says.
fn separated(req: &Value, end: &JobEnd) -> Vec<Value> {
    let id = req["id"].clone();
    match end {
        JobEnd::Separated { job } => {
            let mut running = job.clone();
            running["done"] = json!(false);
            running["phase"] = json!("separating");
            running["percent"] = json!(40);
            vec![
                json!({ "type": "separating", "id": id, "job": running }),
                json!({ "type": "separated", "id": id, "job": job }),
            ]
        }
        JobEnd::Error(m) => vec![json!({ "type": "error", "id": id, "error": m })],
        _ => vec![],
    }
}

/// A machine's jobs path: answers each request on a script and records what it was asked.
pub struct FakeJobs {
    pub url: String,
    asked: Arc<Mutex<Vec<Value>>>,
    end: Arc<Mutex<JobEnd>>,
    delay: Arc<Mutex<Duration>>,
    task: JoinHandle<()>,
}

impl Drop for FakeJobs {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeJobs {
    pub async fn start() -> FakeJobs {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let asked: Arc<Mutex<Vec<Value>>> = Arc::default();
        let end = Arc::new(Mutex::new(JobEnd::Synced {
            failed: 0,
            failures: vec![],
        }));
        let delay: Arc<Mutex<Duration>> = Arc::default();
        let (a, e, d) = (asked.clone(), end.clone(), delay.clone());
        let task = tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                let (a, e, d) = (a.clone(), e.clone(), d.clone());
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(tcp).await else {
                        return;
                    };
                    let (mut write, mut read) = ws.split();
                    while let Some(Ok(Message::Text(t))) = read.next().await {
                        let Ok(req) = serde_json::from_str::<Value>(t.as_str()) else {
                            continue;
                        };
                        held(&a).push(req.clone());
                        let id = req["id"].clone();
                        let files = req["files"].as_array().map_or(0, Vec::len);
                        let wait = *held(&d);
                        let end = held(&e).clone();
                        if req["type"] == "separate" {
                            tokio::time::sleep(wait).await;
                            for reply in separated(&req, &end) {
                                let _ = write.send(Message::text(reply.to_string())).await;
                            }
                            continue;
                        }
                        if req["type"] == "render" {
                            tokio::time::sleep(wait).await;
                            let reply = rendered(&req, &end);
                            if let Some(r) = reply {
                                let _ = write.send(Message::text(r.to_string())).await;
                            }
                            continue;
                        }
                        let _ = write
                            .send(Message::text(
                                json!({ "type": "syncing", "id": id, "files": files, "filesDone": 0, "bytes": 10, "bytesDone": 0 })
                                    .to_string(),
                            ))
                            .await;
                        tokio::time::sleep(wait).await;
                        let reply = match end {
                            JobEnd::Synced { failed, failures } => {
                                json!({ "type": "synced", "id": id, "failed": failed, "failures": failures })
                            }
                            JobEnd::Error(m) | JobEnd::WroteThenError(m) => {
                                json!({ "type": "error", "id": id, "error": m })
                            }
                            JobEnd::Rendered { .. } | JobEnd::Separated { .. } => {
                                json!({ "type": "error", "id": id, "error": "not a sync" })
                            }
                            JobEnd::Never => continue,
                        };
                        let _ = write.send(Message::text(reply.to_string())).await;
                    }
                });
            }
        });
        FakeJobs {
            url,
            asked,
            end,
            delay,
            task,
        }
    }

    pub fn ends(&self, end: JobEnd) {
        *held(&self.end) = end;
    }

    pub fn takes(&self, d: Duration) {
        *held(&self.delay) = d;
    }

    pub fn asked(&self) -> Vec<Value> {
        held(&self.asked).clone()
    }
}

pub use super::testkit::*;
