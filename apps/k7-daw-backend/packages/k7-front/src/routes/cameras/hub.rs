//! hub.rs — the accounts' remote cameras in memory: who is on, which viewer negotiates with which camera, and
//! the frames carried between them. A camera is listed
//! from its admission until its socket ends; the picture itself never passes here (`docs/cameras.md`).
//! The thing a reader would get wrong: the state is behind one lock and never awaited under it, so each
//! socket's task only reads and writes its socket, and every other socket is reached through its queue.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, watch};

use super::names::{self, Listed};
use super::ticket::{now_ms, random_hex};

/// Signalling is SDP and candidates, a few kilobytes: anything far past it is not a frame.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Cameras on at once, per account.
    pub cameras: usize,
    /// Viewers at once, per account: each holds a peer connection the camera encodes for.
    pub viewers: usize,
    /// How often a socket is pinged; one that did not answer the previous ping is dropped.
    pub ping: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            cameras: 8,
            viewers: 32,
            ping: Duration::from_secs(30),
        }
    }
}

/// What a socket's task is asked to write.
enum Out {
    Frame(String),
    Close(u16, &'static str),
}

/// Frames waiting for one socket; past them the socket is too slow to keep and is closed 1013.
const OUTBOX: usize = 256;
/// How long one write to a socket may take before the socket is dropped.
const SEND_WAIT: Duration = Duration::from_secs(10);

/// A socket's outbox: bounded, and a full one says so to the socket's task instead of growing.
#[derive(Clone)]
struct Queue {
    tx: mpsc::Sender<Out>,
    over: Arc<tokio::sync::Notify>,
}

impl Queue {
    fn push(&self, out: Out) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.tx.try_send(out) {
            self.over.notify_one();
        }
    }
}

struct Camera {
    id: String,
    account: String,
    name: String,
    since: f64,
    queue: Queue,
}

struct Viewer {
    account: String,
    queue: Queue,
    /// The camera this viewer negotiates with, once its `want` was matched.
    camera: Option<String>,
    /// Why it got that camera rather than the one it named, carried on the offer.
    note: Option<String>,
}

#[derive(Default)]
struct State {
    /// In the order they came on, as the TypeScript `Map` keeps them.
    cameras: Vec<Camera>,
    viewers: HashMap<String, Viewer>,
}

pub enum Role {
    Camera(String),
    Viewer,
}

/// A socket admitted to the hub, waiting for its upgrade to complete.
pub struct Admitted {
    pub id: String,
    queue: mpsc::Receiver<Out>,
    over: Arc<tokio::sync::Notify>,
}

pub struct Hub {
    limits: Limits,
    state: Mutex<State>,
    stop: watch::Sender<bool>,
}

fn send(queue: &Queue, frame: Value) {
    queue.push(Out::Frame(
        crate::prefs_schema::js_numbers(&frame).to_string(),
    ));
}

impl State {
    fn at_cap(&self, account: &str, role: &Role, limits: &Limits) -> bool {
        match role {
            Role::Camera(_) => {
                self.cameras.iter().filter(|c| c.account == account).count() >= limits.cameras
            }
            Role::Viewer => {
                self.viewers
                    .values()
                    .filter(|v| v.account == account)
                    .count()
                    >= limits.viewers
            }
        }
    }

    fn camera(&self, id: &str) -> Option<&Camera> {
        self.cameras.iter().find(|c| c.id == id)
    }

    fn names(&self, account: &str) -> Vec<&str> {
        self.cameras
            .iter()
            .filter(|c| c.account == account)
            .map(|c| c.name.as_str())
            .collect()
    }

    /// Tell a camera how many are watching, which its screen shows.
    fn count_viewers(&self, camera: &Camera) {
        let count = self
            .viewers
            .values()
            .filter(|v| v.camera.as_deref() == Some(camera.id.as_str()))
            .count();
        send(&camera.queue, json!({ "type": "viewers", "count": count }));
    }
}

impl Hub {
    pub fn new(limits: Limits) -> Self {
        Hub {
            limits,
            state: Mutex::new(State::default()),
            stop: watch::channel(false).0,
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The account's cameras, oldest first, as `GET /api/cameras` lists them.
    pub fn list(&self, account: &str) -> Value {
        let state = self.state();
        let listed: Vec<Value> = state
            .cameras
            .iter()
            .filter(|c| c.account == account)
            .map(|c| json!({ "name": c.name, "since": c.since }))
            .collect();
        crate::prefs_schema::js_numbers(&Value::Array(listed))
    }

    /// Whether `account` is at its cap for this role, checked before the handshake and again on admission.
    pub fn full(&self, account: &str, role: &Role) -> bool {
        self.state().at_cap(account, role, &self.limits)
    }

    /// Take a socket in: a camera is listed under a unique name and greeted; None when the account is full.
    pub fn admit(&self, account: &str, role: Role) -> Option<Admitted> {
        let mut state = self.state();
        if state.at_cap(account, &role, &self.limits) {
            return None;
        }
        let (tx, rx) = mpsc::channel(OUTBOX);
        let over = Arc::new(tokio::sync::Notify::new());
        let queue = Queue {
            tx,
            over: over.clone(),
        };
        let id = random_hex(8);
        match role {
            Role::Camera(raw) => {
                let name = names::unique_camera_name(
                    &names::clean_camera_name(&raw, "camera"),
                    &state.names(account),
                );
                send(&queue, json!({ "type": "hello", "name": name }));
                state.cameras.push(Camera {
                    id: id.clone(),
                    account: account.into(),
                    name,
                    since: now_ms(),
                    queue,
                });
            }
            Role::Viewer => {
                state.viewers.insert(
                    id.clone(),
                    Viewer {
                        account: account.into(),
                        queue,
                        camera: None,
                        note: None,
                    },
                );
            }
        }
        Some(Admitted {
            id,
            queue: rx,
            over,
        })
    }

    /// A socket that ended, or whose upgrade never completed: unlisted, and its counterparts told.
    pub fn leave(&self, id: &str) {
        let mut state = self.state();
        if let Some(at) = state.cameras.iter().position(|c| c.id == id) {
            state.cameras.remove(at);
            // A viewer's picture is peer to peer and dies with the camera; its socket is closed so it hears why.
            for v in state
                .viewers
                .values()
                .filter(|v| v.camera.as_deref() == Some(id))
            {
                v.queue.push(Out::Close(1001, "the camera went off"));
            }
            return;
        }
        let Some(v) = state.viewers.remove(id) else {
            return;
        };
        if let Some(cam) = v.camera.as_deref().and_then(|c| state.camera(c)) {
            send(&cam.queue, json!({ "type": "gone", "from": id }));
            state.count_viewers(cam);
        }
    }

    /// A frame a socket sent, carried where the hub's rules let it go.
    fn receive(&self, id: &str, text: &str) {
        let Some(frame) = names::read_camera_frame(text) else {
            return;
        };
        let mut state = self.state();
        if state.camera(id).is_some() {
            from_camera(&state, id, frame);
        } else {
            from_viewer(&mut state, id, frame);
        }
    }

    /// Close every socket, as a stopping server says goodbye.
    pub fn stop(&self) {
        self.stop.send_replace(true);
    }

    /// Serve an admitted socket until it closes, misses a ping, or the hub stops.
    pub async fn serve(self: Arc<Self>, admitted: Admitted, mut socket: WebSocket) {
        let Admitted {
            id,
            mut queue,
            over,
        } = admitted;
        let mut stop = self.stop.subscribe();
        let start = tokio::time::Instant::now() + self.limits.ping;
        let mut beat = tokio::time::interval_at(start, self.limits.ping);
        let (mut alive, mut closing) = (true, false);
        loop {
            tokio::select! {
                got = socket.recv() => match got {
                    Some(Ok(Message::Text(t))) => self.receive(&id, t.as_str()),
                    Some(Ok(Message::Binary(b))) => self.receive(&id, &String::from_utf8_lossy(&b)),
                    Some(Ok(Message::Pong(_))) => alive = true,
                    Some(Ok(Message::Ping(_) | Message::Close(_))) => {}
                    Some(Err(_)) | None => break,
                },
                out = queue.recv(), if !closing => match out {
                    Some(Out::Frame(text)) => {
                        if !sent(socket.send(Message::Text(text.into()))).await {
                            break;
                        }
                    }
                    Some(Out::Close(code, reason)) => {
                        closing = true;
                        let close = CloseFrame { code, reason: reason.into() };
                        if !sent(socket.send(Message::Close(Some(close)))).await {
                            break;
                        }
                    }
                    None => break,
                },
                _ = beat.tick() => {
                    if !alive || closing {
                        break;
                    }
                    alive = false;
                    if !sent(socket.send(Message::Ping(Bytes::new()))).await {
                        break;
                    }
                }
                _ = over.notified(), if !closing => {
                    let close = CloseFrame { code: 1013, reason: "this connection is too slow to keep up".into() };
                    let _ = sent(socket.send(Message::Close(Some(close)))).await;
                    break;
                }
                _ = async { stop.wait_for(|s| *s).await.is_ok() }, if !closing => {
                    closing = true;
                    let close = CloseFrame { code: 1001, reason: "the server is stopping".into() };
                    if !sent(socket.send(Message::Close(Some(close)))).await {
                        break;
                    }
                }
            }
        }
        self.leave(&id);
    }
}

/// Whether a write to a socket completed within `SEND_WAIT`.
async fn sent<E>(write: impl std::future::Future<Output = Result<(), E>>) -> bool {
    matches!(tokio::time::timeout(SEND_WAIT, write).await, Ok(Ok(())))
}

/// A camera's frame reaches a viewer only if that viewer asked for this camera.
fn from_camera(state: &State, id: &str, mut frame: Map<String, Value>) {
    let Some(Value::String(to)) = frame.get("to").cloned() else {
        return;
    };
    let Some(v) = state.viewers.get(&to) else {
        return;
    };
    if v.camera.as_deref() != Some(id) {
        return;
    }
    frame.shift_remove("to");
    let kind = frame
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if kind == "offer" {
        frame.insert(
            "problem".into(),
            v.note.clone().map_or(Value::Null, Value::String),
        );
    }
    if matches!(kind.as_str(), "offer" | "ice" | "problem") {
        send(&v.queue, Value::Object(frame));
    }
}

/// A viewer's `want` picks its camera once; its answer and candidates reach only that camera.
fn from_viewer(state: &mut State, id: &str, mut frame: Map<String, Value>) {
    let Some(v) = state.viewers.get(id) else {
        return;
    };
    let kind = frame.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "want"
        && let Some(Value::String(label)) = frame.get("label")
    {
        // One camera per socket: a lane is one picture.
        if v.camera.is_some() {
            return;
        }
        let account = v.account.clone();
        let listed: Vec<Listed> = state
            .cameras
            .iter()
            .filter(|c| c.account == account)
            .map(|c| Listed {
                name: &c.name,
                since: c.since,
            })
            .collect();
        let (found, problem) = names::match_remote_camera(label, &listed);
        let cam = found.and_then(|n| {
            state
                .cameras
                .iter()
                .find(|c| c.account == account && c.name == n)
        });
        let Some(cam) = cam else {
            let text = problem.unwrap_or_else(|| "no remote camera is on".into());
            send(&v.queue, json!({ "type": "problem", "text": text }));
            return;
        };
        send(
            &cam.queue,
            json!({ "type": "want", "label": label, "from": id }),
        );
        let cam_id = cam.id.clone();
        if let Some(v) = state.viewers.get_mut(id) {
            v.camera = Some(cam_id.clone());
            v.note = problem;
        }
        if let Some(cam) = state.camera(&cam_id) {
            state.count_viewers(cam);
        }
        return;
    }
    if !matches!(kind, "answer" | "ice") {
        return;
    }
    let Some(cam) = v.camera.as_deref().and_then(|c| state.camera(c)) else {
        return;
    };
    frame.insert("from".into(), Value::String(id.into()));
    send(&cam.queue, Value::Object(frame));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An outbox nobody drains stops at its bound and says it is over, instead of growing with every frame.
    #[tokio::test]
    async fn a_full_outbox_is_bounded_and_told() {
        let (tx, mut rx) = mpsc::channel(OUTBOX);
        let over = Arc::new(tokio::sync::Notify::new());
        let queue = Queue {
            tx,
            over: over.clone(),
        };
        for n in 0..OUTBOX * 4 {
            send(&queue, json!({ "n": n }));
        }
        let told = tokio::time::timeout(Duration::from_secs(1), over.notified()).await;
        assert!(told.is_ok(), "a full outbox was not told");
        let mut kept = 0;
        while rx.try_recv().is_ok() {
            kept += 1;
        }
        assert_eq!(kept, OUTBOX);
    }
}
