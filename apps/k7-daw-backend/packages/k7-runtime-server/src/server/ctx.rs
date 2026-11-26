//! ctx.rs — what every handler of the server shares: the engine, who this engine says it is, the connected
//! clients and who listens to, drives or subscribes to what, and the broadcast verbs over them.
//! What a reader gets wrong: a client is named by a counter that is never reused, not by a pointer, so a late
//! frame can never reach the next connection that took the old one's place.

use super::held_notes::HeldNotes;
use super::machine::MachineState;
use super::msg::Msg;
use super::outbox::Outbox;
use super::pending::PendingQueries;
use super::plugin_state::PluginState;
use super::supersede::Supersede;
use crate::engine::Engine;
use crate::pairing::gate::{Gate, Out};
use crate::wire::json;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Connections past this are dropped at accept.
pub const MAX_CONNECTIONS: usize = 32;
/// The most sessions an engine holds (`EngineHost::kMaxSessions`).
pub const MAX_SESSIONS: usize = 64;
/// A ceiling on a session id, not a shape: naming one creates it.
pub const MAX_SESSION_ID_BYTES: usize = 256;

/// One connected socket.
pub struct Client {
    pub id: u64,
    pub ip: String,
    pub outbox: Outbox,
}

impl Client {
    /// ANY THREAD. A reply to this socket, printed as nlohmann prints it.
    pub fn send(&self, j: &Value) -> bool {
        self.outbox.post(&Arc::from(json::dump(j)), false)
    }

    /// A frame already printed, shared by whoever sends it to several sockets.
    pub fn send_text(&self, payload: &Arc<str>, droppable: bool) -> bool {
        self.outbox.post(payload, droppable)
    }

    pub fn close(&self, code: u16, reason: &'static str) {
        self.outbox.close(code, reason);
    }
}

/// Who listens to, drives and is told about what, by client id, under one lock.
#[derive(Default)]
pub struct Bindings {
    /// Who asked for audio, and for which session.
    pub listeners: HashMap<u64, String>,
    /// Which sockets DRIVE a session, declared on hello with `{"role":"driver"}`, at most one session each.
    pub drivers: HashMap<u64, String>,
    /// WHICH project each socket is about, bound ONCE from its first real message, so a state frame reaches only those.
    pub subscribers: HashMap<u64, String>,
    /// The codec each listener asked for; absent is float32.
    pub codecs: HashMap<u64, u8>,
    /// The listeners that hear the mix by microphone, which the interface must not play back into.
    pub silent: HashSet<u64>,
}

/// What a socket was bound to when it left: the sessions the engine must be told it no longer serves.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Departed {
    pub listener: Option<String>,
    pub driver: Option<String>,
}

/// Who this engine says it is.
pub struct Identity {
    pub id: String,
    pub token: String,
    /// The start time in ms names this process: a relaunch after a crash is a new one.
    pub boot_id: String,
}

pub struct Ctx {
    pub engine: Engine,
    pub identity: Identity,
    /// Nobody hears this machine's device, so a client should play the broadcast instead.
    pub audio_to_browser: bool,
    /// The quick crashes that made the supervisor start this engine in safe mode, 0 when it is not in it.
    pub safe_mode_crashes: u32,
    pub gate: Arc<Gate>,
    /// The read-only queries waiting on the message thread, which an identical one joins.
    pub pending: PendingQueries,
    /// The plugin verbs' shared state: the one scan, the probe slots, the live parameter tickets.
    pub plugins: PluginState,
    /// Which queued project, tracks and parameter jobs a newer message made pointless.
    pub supersede: Supersede,
    /// The notes each socket's `midi-live` holds down, released when it closes.
    pub held_notes: HeldNotes,
    /// The machine verbs' shared state: the device list last sent, the arrivals, the one render.
    pub machine: MachineState,
    clients: Mutex<HashMap<u64, Arc<Client>>>,
    bindings: Mutex<Bindings>,
    next_id: AtomicU64,
}

impl Ctx {
    pub fn new(engine: Engine, identity: Identity, gate: Arc<Gate>) -> Ctx {
        Ctx {
            engine,
            identity,
            audio_to_browser: false,
            safe_mode_crashes: 0,
            gate,
            pending: PendingQueries::default(),
            plugins: PluginState::default(),
            supersede: Supersede::default(),
            held_notes: HeldNotes::default(),
            machine: MachineState::default(),
            clients: Mutex::default(),
            bindings: Mutex::default(),
            next_id: AtomicU64::new(1),
        }
    }

    fn clients(&self) -> MutexGuard<'_, HashMap<u64, Arc<Client>>> {
        self.clients.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The binding maps, locked: a post made under the lock cannot land after a later socket's.
    pub fn bindings(&self) -> MutexGuard<'_, Bindings> {
        self.bindings.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A socket accepted, or `None` at the connection limit.
    pub fn register(&self, ip: String) -> Option<Arc<Client>> {
        let mut clients = self.clients();
        if clients.len() >= MAX_CONNECTIONS {
            return None;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let client = Arc::new(Client {
            id,
            ip,
            outbox: Outbox::default(),
        });
        clients.insert(id, client.clone());
        Some(client)
    }

    /// A socket closed: its bindings and its trust go, and it is no one's target any more.
    pub fn forget(&self, client: &Client) -> Departed {
        let departed = {
            let mut b = self.bindings();
            b.subscribers.remove(&client.id);
            super::broadcast::leave(&self.engine, &mut b, client.id)
        };
        self.perform(client, self.gate.forget_socket(client.id));
        self.clients().remove(&client.id);
        client.outbox.shutdown();
        departed
    }

    pub fn client_count(&self) -> usize {
        self.clients().len()
    }

    pub fn client(&self, id: u64) -> Option<Arc<Client>> {
        self.clients().get(&id).cloned()
    }

    fn snapshot(&self) -> Vec<Arc<Client>> {
        self.clients().values().cloned().collect()
    }

    /// A machine-wide reply: trusted sockets only, an unpaired LAN peer hears nothing but its own refusals.
    pub fn broadcast(&self, j: &Value) {
        let payload: Arc<str> = Arc::from(json::dump(j));
        for c in self.snapshot() {
            if self.gate.is_trusted(c.id) {
                c.send_text(&payload, false);
            }
        }
    }

    /// A reply about one project, stamped with its session: a verb of its own, so a new reply must CHOOSE.
    pub fn broadcast_to(&self, session: &str, mut j: Value) {
        if let Value::Object(map) = &mut j {
            map.insert("session".into(), Value::String(session.to_string()));
        }
        self.broadcast(&j);
    }

    /// The machine's own clients only: what a pairing request carries is for the musician's eyes.
    pub fn broadcast_local(&self, j: &Value) {
        let payload: Arc<str> = Arc::from(json::dump(j));
        for id in self.gate.locals() {
            if let Some(c) = self.client(id) {
                c.send_text(&payload, false);
            }
        }
    }

    /// What the gate asks done, in order: `from` is the socket a reply goes back to.
    pub fn perform(&self, from: &Client, outs: Vec<Out>) {
        for out in outs {
            match out {
                Out::Reply(v) => {
                    from.send(&v);
                }
                Out::To(id, v) => {
                    self.send_to(id, &v);
                }
                Out::ToLocals(v) => self.broadcast_local(&v),
                Out::Close(id) => self.close_client(id),
            }
        }
    }

    /// One socket by id, if it is still connected.
    pub fn send_to(&self, id: u64, j: &Value) -> bool {
        self.client(id).is_some_and(|c| c.send(j))
    }

    pub fn close_client(&self, id: u64) {
        if let Some(c) = self.client(id) {
            c.close(1000, "closed");
        }
    }

    /// The sockets that asked about `session`, for a state frame addressed to them alone.
    pub fn state_targets(&self, session: &str) -> Vec<Arc<Client>> {
        let ids: Vec<u64> = {
            let b = self.bindings();
            b.subscribers
                .iter()
                .filter(|(_, s)| s.as_str() == session)
                .map(|(id, _)| *id)
                .collect()
        };
        ids.into_iter().filter_map(|id| self.client(id)).collect()
    }
}

/// One request a handler answers: the socket, the parsed message, its session and its type.
pub struct Request {
    pub ctx: Arc<Ctx>,
    pub client: Arc<Client>,
    pub msg: Msg,
    pub session: String,
    pub ty: String,
}
