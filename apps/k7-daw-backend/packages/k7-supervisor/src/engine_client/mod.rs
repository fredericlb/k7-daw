//! engine_client — request/reply over ONE long-lived connection to the engine. Replies are typed by frame,
//! not by request, so exactly one call is in flight: every call queues on the connection's lock, waits for
//! its typed reply or its deadline, and a deadline forces a redial rather than trusting a half-dead socket.
//! `close` makes the call in flight give up at once and the next one dial. The why:
//! `apps/k7-daw-engine/k7-supervisor/docs/supervisorPolicy.md`, "EngineQueries".

mod conn;
pub mod replies;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Mutex, watch};
use tokio::time::timeout;

use crate::protocol::{AudioDevice, EditPlugin, PairRequest, PairedDevice, Session};
use conn::Conn;
pub use replies::Catalogue;

/// The deadlines are generous on purpose: a plugin list can be thousands of entries, a device switch opens hardware.
pub const WHOAMI: Duration = Duration::from_millis(4000);
pub const DEVICES: Duration = Duration::from_millis(6000);
pub const PLUGINS: Duration = Duration::from_millis(8000);
pub const SET_DEVICE: Duration = Duration::from_millis(8000);
pub const EDIT_PLUGINS: Duration = Duration::from_millis(6000);
pub const SESSIONS: Duration = Duration::from_millis(6000);
pub const PAIRING: Duration = Duration::from_millis(4000);

struct Inner {
    port: AtomicU16,
    /// Bumped by `close`; a call that started in an older one returns nothing.
    generation: watch::Sender<u64>,
    conn: Mutex<Option<Conn>>,
}

#[derive(Clone)]
pub struct EngineClient(Arc<Inner>);

impl EngineClient {
    pub fn new(port: u16) -> EngineClient {
        EngineClient(Arc::new(Inner {
            port: AtomicU16::new(port),
            generation: watch::channel(0).0,
            conn: Mutex::new(None),
        }))
    }

    /// The engine now serves on `port`: the next dial goes there.
    pub fn set_port(&self, port: u16) {
        self.0.port.store(port, Ordering::Relaxed);
        self.close();
    }

    /// Drop the connection (the engine stopped); the call in flight returns nothing, the next one redials.
    pub fn close(&self) {
        self.0.generation.send_modify(|g| *g += 1);
        let inner = self.0.clone();
        tokio::spawn(async move {
            let mut conn = inner.conn.lock().await;
            if conn
                .as_ref()
                .is_some_and(|c| c.generation != *inner.generation.borrow())
            {
                *conn = None;
            }
        });
    }

    /// The generation now: a result is current when it is still this one after the call.
    pub fn generation(&self) -> u64 {
        *self.0.generation.borrow()
    }

    /// The connection, dialed when there is none or it went stale. `None` when the engine does not answer.
    async fn connected<'a>(
        &self,
        slot: &'a mut Option<Conn>,
        generation: u64,
    ) -> Option<&'a mut Conn> {
        if slot
            .as_ref()
            .is_some_and(|c| !c.open() || c.generation != generation)
        {
            *slot = None;
        }
        if slot.is_none() {
            *slot = Conn::dial(self.0.port.load(Ordering::Relaxed), generation).await;
        }
        slot.as_mut()
    }

    /// `frame` sent, then the next frame of type `reply` waited for; None on no connection, a failed send,
    /// the deadline or a `close` meanwhile.
    pub async fn request(&self, frame: Value, reply: &str, deadline: Duration) -> Option<Value> {
        let mut changed = self.0.generation.subscribe();
        let generation = *changed.borrow_and_update();
        let call = async {
            let mut slot = self.0.conn.lock().await;
            if *self.0.generation.borrow() != generation {
                return None;
            }
            let conn = self.connected(&mut slot, generation).await?;
            let waiter = conn.ask(&frame, reply).await;
            let answer = match waiter {
                Some(rx) => timeout(deadline, rx).await.ok().and_then(Result::ok),
                None => None,
            };
            if answer.is_none() {
                // A timeout usually means the engine went away mid-call: force a redial.
                conn.forget();
                *slot = None;
            }
            answer
        };
        tokio::select! {
            out = call => out,
            _ = changed.changed() => None,
        }
    }

    /// Fire-and-forget; false when no connection could be had or the frame did not go.
    pub async fn send(&self, frame: Value) -> bool {
        let mut changed = self.0.generation.subscribe();
        let generation = *changed.borrow_and_update();
        let call = async {
            let mut slot = self.0.conn.lock().await;
            if *self.0.generation.borrow() != generation {
                return false;
            }
            let Some(conn) = self.connected(&mut slot, generation).await else {
                return false;
            };
            let sent = conn.send(&frame).await;
            if !sent {
                *slot = None;
            }
            sent
        };
        tokio::select! {
            out = call => out,
            _ = changed.changed() => false,
        }
    }

    /// The engine's `whoami` reply as it came: its protocol, build and safe mode.
    pub async fn whoami(&self) -> Option<Value> {
        self.request(json!({"type": "whoami"}), "whoami", WHOAMI)
            .await
    }

    pub async fn list_audio_devices(&self) -> Option<Vec<AudioDevice>> {
        let msg = self
            .request(
                json!({"type": "list-audio-devices"}),
                "audio-devices",
                DEVICES,
            )
            .await?;
        Some(replies::devices(&msg))
    }

    /// Switch interface; the engine answers with the same device list.
    pub async fn set_audio_device(
        &self,
        name: &str,
        device_type: &str,
    ) -> Option<Vec<AudioDevice>> {
        let mut frame = json!({"type": "set-audio-device", "name": name});
        if !device_type.is_empty() {
            frame["deviceType"] = json!(device_type);
        }
        let msg = self.request(frame, "audio-devices", SET_DEVICE).await?;
        Some(replies::devices(&msg))
    }

    pub async fn list_plugins(&self) -> Option<Catalogue> {
        let msg = self
            .request(json!({"type": "list-plugins"}), "plugins", PLUGINS)
            .await?;
        Some(replies::catalogue(&msg))
    }

    /// The plugin INSTANCES across every session, each naming its own.
    pub async fn list_edit_plugins(&self) -> Option<Vec<EditPlugin>> {
        let msg = self
            .request(
                json!({"type": "list-edit-plugins"}),
                "edit-plugins",
                EDIT_PLUGINS,
            )
            .await?;
        Some(replies::edit_plugins(&msg))
    }

    pub async fn list_sessions(&self) -> Option<Vec<Session>> {
        let msg = self
            .request(json!({"type": "list-sessions"}), "sessions", SESSIONS)
            .await?;
        Some(replies::sessions(&msg))
    }

    /// The LAN devices waiting for the musician's approval.
    pub async fn list_pair_requests(&self) -> Option<Vec<PairRequest>> {
        let msg = self
            .request(
                json!({"type": "list-pair-requests"}),
                "pair-requests",
                PAIRING,
            )
            .await?;
        Some(replies::pair_requests(&msg))
    }

    /// The LAN devices this engine has paired.
    pub async fn list_paired_devices(&self) -> Option<Vec<PairedDevice>> {
        let msg = self
            .request(
                json!({"type": "list-paired-devices"}),
                "paired-devices",
                PAIRING,
            )
            .await?;
        Some(replies::paired_devices(&msg))
    }

    /// `pair-approve` and `pair-deny` (a request id) or `unpair-device` (a device id); the pairing poll shows the result.
    pub async fn pair_verb(&self, verb: &str, id: &str) -> bool {
        let key = if verb == "unpair-device" {
            "deviceId"
        } else {
            "requestId"
        };
        self.send(json!({"type": verb, key: id})).await
    }

    /// Whether a device on this network drives unpaired; closing drops the ones it let in.
    pub async fn set_lan_open(&self, open: bool) -> bool {
        self.send(json!({"type": "lan-open", "open": open})).await
    }

    pub async fn drop_session(&self, session: &str) -> bool {
        self.send(json!({"type": "drop-session", "session": session}))
            .await
    }

    /// A compiled document into `session`: the slot's, the one project the supervisor ever sends.
    pub async fn apply_project(&self, session: &str, doc: &Value) -> bool {
        self.send(json!({"type": "project", "session": session, "doc": doc}))
            .await
    }

    /// The engine acknowledges by drawing the window.
    pub async fn show_native_plugin_ui(&self, id: &str, session: Option<&str>) -> bool {
        let mut frame = json!({"type": "show-native-plugin-ui", "id": id});
        if let Some(s) = session {
            frame["session"] = json!(s);
        }
        self.send(frame).await
    }

    /// Asks for a rescan; the scan cache is deleted by the caller first, or the rescan returns the stale list.
    pub async fn rescan_plugins(&self) -> bool {
        self.send(json!({"type": "rescan-plugins", "force": true}))
            .await
    }
}
