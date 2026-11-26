//! gate.rs — the gate every message passes first (ServePairing.cpp): who is trusted, and what an unpaired
//! LAN peer may say. Pure: it takes a socket's id, its peer address and the parsed message, and returns
//! what to send and whom to close; it owns no socket. The seam for the socket core is in the plan.
//! What a reader gets wrong: a loopback peer is the machine, relayed traffic included (`docs/pairing.md`).

use super::SocketId;
use super::net::{is_loopback, is_private, origin_allowed, same_key};
use super::store::{Approved, Pairing};
use crate::log::say;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// What only the machine itself may ask: a paired phone cannot approve a second one, nor list what pairs it.
const LOCAL_ONLY: &[&str] = crate::wire::catalogue::LOCAL_ONLY;

/// The largest frame a socket not yet trusted may send: a pairing request or a probe is well under a kilobyte.
pub const MAX_UNTRUSTED_BYTES: usize = 16 * 1024;

/// One thing for the socket core to do, in the order given.
#[derive(Debug, PartialEq)]
pub enum Out {
    /// Send to the socket the message came from.
    Reply(Value),
    /// Send to one socket, if it is still connected.
    To(SocketId, Value),
    /// Send to every loopback socket: what a request carries is for the musician's eyes.
    ToLocals(Value),
    /// Close that socket (its trust is already gone).
    Close(SocketId),
}

/// What the engine states about itself in a `whoami` an unpaired peer may still ask.
pub struct GateConfig {
    pub engine_id: String,
    pub engine_token: String,
    pub boot_id: String,
    /// `STUDIO_ENGINE_ALLOWED_ORIGINS`, parsed by `parse_origins`.
    pub allowed_origins: Vec<String>,
}

#[derive(Default)]
struct Trust {
    trusted: HashSet<SocketId>,
    local: HashSet<SocketId>,
    refused: HashSet<SocketId>,
    lan_trusted: HashSet<SocketId>,
}

pub struct Gate {
    pairing: Pairing,
    cfg: GateConfig,
    lan_open: AtomicBool,
    trust: Mutex<Trust>,
}

fn refuse(code: &str, message: &str) -> Out {
    Out::Reply(json!({"type": "error", "code": code, "message": message}))
}

impl Gate {
    /// The LAN starts closed: an engine nobody has told keeps it so, the side to fail on.
    pub fn new(pairing: Pairing, cfg: GateConfig) -> Self {
        Gate {
            pairing,
            cfg,
            lan_open: AtomicBool::new(false),
            trust: Mutex::default(),
        }
    }

    fn trust(&self) -> std::sync::MutexGuard<'_, Trust> {
        self.trust.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether the socket may be sent a machine-wide broadcast.
    pub fn is_trusted(&self, id: SocketId) -> bool {
        self.trust().trusted.contains(&id)
    }

    /// The loopback sockets, for `Out::ToLocals`.
    pub fn locals(&self) -> Vec<SocketId> {
        let mut v: Vec<SocketId> = self.trust().local.iter().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn lan_open(&self) -> bool {
        self.lan_open.load(Ordering::SeqCst)
    }

    /// The handshake's `Origin` (None: no header, a native client); true when the socket is refused and
    /// must be closed with code 1008, `origin not allowed`. Every later message of it is dropped.
    pub fn refuse_origin(&self, id: SocketId, origin: Option<&str>) -> bool {
        let Some(origin) = origin else { return false };
        if origin_allowed(origin, &self.cfg.allowed_origins) {
            return false;
        }
        self.trust().refused.insert(id);
        say!("[client] refused a page from '{origin}' (docs/pairing.md)");
        true
    }

    /// Whether the socket's handshake was refused: its messages are dropped, not answered.
    pub fn is_refused(&self, id: SocketId) -> bool {
        self.trust().refused.contains(&id)
    }

    /// A frame too large for a socket not yet trusted is answered `unpaired` before it is parsed.
    pub fn screen_frame_size(&self, id: SocketId, peer_ip: &str, bytes: usize) -> Option<Out> {
        if bytes <= MAX_UNTRUSTED_BYTES || is_loopback(peer_ip) || self.is_trusted(id) {
            return None;
        }
        Some(refuse(
            "unpaired",
            "this frame is too large for a device that is not paired",
        ))
    }

    fn trust_socket(&self, id: SocketId, is_local: bool) {
        let mut t = self.trust();
        t.trusted.insert(id);
        if is_local {
            t.local.insert(id);
        }
    }

    fn untrust(&self, id: SocketId) {
        let mut t = self.trust();
        t.trusted.remove(&id);
        t.local.remove(&id);
        t.refused.remove(&id);
        t.lan_trusted.remove(&id);
    }

    /// A peer of this network, the LAN being open: trusted unpaired, and remembered so a close of the LAN drops it.
    fn trust_lan(&self, id: SocketId) {
        let mut t = self.trust();
        if t.trusted.insert(id) {
            t.lan_trusted.insert(id);
        }
    }

    /// The gate for one parsed message of type `ty`: None lets it through to the verb table, Some is the
    /// whole answer and nothing else must run.
    pub fn screen_message(
        &self,
        id: SocketId,
        peer_ip: &str,
        msg: &Value,
        ty: &str,
    ) -> Option<Vec<Out>> {
        let is_local = is_loopback(peer_ip);
        let on_lan = self.lan_open() && is_private(peer_ip);
        if is_local {
            self.trust_socket(id, true);
        } else if on_lan {
            self.trust_lan(id);
        }
        let trusted = is_local || self.is_trusted(id);
        let text = |k: &str| msg.get(k).and_then(Value::as_str).unwrap_or_default();

        match ty {
            "pair-request" => {
                return Some(self.pair_request(id, peer_ip, text("name"), is_local, on_lan));
            }
            "pair" => return Some(self.pair(id, text("key"))),
            _ => {}
        }
        if !trusted {
            // A probe still learns that an engine is here and that it wants pairing, never its token.
            if ty == "whoami" || ty == "hello" {
                let c = &self.cfg;
                return Some(vec![Out::Reply(
                    json!({"type": "whoami", "id": c.engine_id, "boot": c.boot_id, "needsPairing": true}),
                )]);
            }
            return Some(vec![refuse(
                "unpaired",
                "this device is not paired with the engine — pair it first",
            )]);
        }
        if !LOCAL_ONLY.contains(&ty) {
            return None;
        }
        if !is_local {
            return Some(vec![refuse(
                "local-only",
                "only the machine running the engine can manage paired devices",
            )]);
        }
        Some(self.manage(ty, msg))
    }

    fn pair_request(
        &self,
        id: SocketId,
        ip: &str,
        name: &str,
        is_local: bool,
        on_lan: bool,
    ) -> Vec<Out> {
        if is_local {
            return vec![refuse("not-needed", "this machine needs no pairing")];
        }
        if on_lan {
            return vec![refuse(
                "not-needed",
                "this engine is open to its own network: no pairing is needed",
            )];
        }
        let Some(r) = self.pairing.request(name, ip, Some(id)) else {
            return vec![refuse(
                "busy",
                "too many devices are waiting to be paired — try again in a couple of minutes",
            )];
        };
        say!(
            "[pairing] '{}' ({ip}) asks to pair, code {}",
            r.name,
            r.code
        );
        vec![
            Out::Reply(json!({"type": "pair-pending", "requestId": r.id, "code": r.code})),
            Out::ToLocals(self.requests_json()),
        ]
    }

    fn pair(&self, id: SocketId, key: &str) -> Vec<Out> {
        // The engine's own token pairs too: it is what the server holds, for a relay dialling us over the LAN.
        if !key.is_empty() && same_key(key, &self.cfg.engine_token) {
            self.trust_socket(id, false);
            return vec![Out::Reply(json!({"type": "paired"}))];
        }
        if !key.is_empty()
            && let Some(d) = self.pairing.verify(key, Some(id))
        {
            self.trust_socket(id, false);
            return vec![Out::Reply(
                json!({"type": "paired", "deviceId": d.id, "name": d.name}),
            )];
        }
        vec![refuse(
            "unpaired",
            "this device is not paired with the engine (or was unpaired) — pair it again",
        )]
    }

    /// The machine's own verbs: list, approve, deny, unpair, and the supervisor's `lan-open`.
    fn manage(&self, ty: &str, msg: &Value) -> Vec<Out> {
        let text = |k: &str| msg.get(k).and_then(Value::as_str).unwrap_or_default();
        match ty {
            "list-pair-requests" => vec![Out::Reply(self.requests_json())],
            "list-paired-devices" => vec![Out::Reply(self.devices_json())],
            "pair-approve" => {
                let Some(Approved { device, key, from }) = self.pairing.approve(text("requestId"))
                else {
                    return vec![refuse(
                        "gone",
                        "that request expired or was already answered",
                    )];
                };
                let mut out = Vec::new();
                // The key goes to the device that asked, and nowhere else; it is never sent again.
                if let Some(from) = from {
                    self.trust_socket(from, false);
                    let paired = json!({"type": "paired", "deviceId": device.id, "name": device.name, "key": key});
                    out.push(Out::To(from, paired));
                }
                say!("[pairing] '{}' paired", device.name);
                out.push(Out::ToLocals(self.requests_json()));
                out.push(Out::ToLocals(self.devices_json()));
                out
            }
            "pair-deny" => {
                let request_id = text("requestId");
                let mut out = Vec::new();
                if let Some(from) = self.pairing.deny(request_id) {
                    out.push(Out::To(
                        from,
                        json!({"type": "pair-denied", "requestId": request_id}),
                    ));
                }
                out.push(Out::ToLocals(self.requests_json()));
                out
            }
            "unpair-device" => {
                // Its live connections lose their trust now, and are closed: an unpaired phone must not keep driving.
                let mut out = Vec::new();
                for live in self.pairing.revoke(text("deviceId")) {
                    self.untrust(live);
                    out.push(Out::Close(live));
                }
                out.push(Out::ToLocals(self.devices_json()));
                out
            }
            _ => self.set_lan_open(msg.get("open").and_then(Value::as_bool).unwrap_or(false)),
        }
    }

    fn set_lan_open(&self, open: bool) -> Vec<Out> {
        self.lan_open.store(open, Ordering::SeqCst);
        let mut dropped: Vec<SocketId> = Vec::new();
        if !open {
            dropped = self.trust().lan_trusted.iter().copied().collect();
            dropped.sort_unstable();
        }
        // Closed, the LAN devices let in by it go now: a phone must pair before it drives again.
        let out = dropped
            .into_iter()
            .map(|id| {
                self.untrust(id);
                Out::Close(id)
            })
            .collect();
        let state = if open {
            "open: a device on this network drives unpaired"
        } else {
            "closed: a device on this network pairs first"
        };
        say!("[pairing] the LAN is {state}");
        out
    }

    /// A socket closed: its trust, its request and its device binding go; the musician's list is refreshed.
    pub fn forget_socket(&self, id: SocketId) -> Vec<Out> {
        self.untrust(id);
        if self.pairing.forget(id) {
            vec![Out::ToLocals(self.requests_json())]
        } else {
            Vec::new()
        }
    }

    fn requests_json(&self) -> Value {
        let list: Vec<Value> = self
            .pairing
            .pending()
            .into_iter()
            .map(|r| json!({"requestId": r.id, "name": r.name, "ip": r.ip, "code": r.code, "since": r.created_ms}))
            .collect();
        json!({"type": "pair-requests", "requests": list})
    }

    fn devices_json(&self) -> Value {
        let list: Vec<Value> = self
            .pairing
            .devices()
            .into_iter()
            .map(|d| json!({"deviceId": d.id, "name": d.name, "created": d.created_ms, "lastSeen": d.last_seen_ms}))
            .collect();
        json!({"type": "paired-devices", "devices": list})
    }
}
