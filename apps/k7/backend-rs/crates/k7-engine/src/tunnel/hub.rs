//! hub.rs — the SERVER end of the engine tunnels: supervisors dial in on TUNNEL_PATH,
//! one tunnel per engine, the newest verified one winning; a process on this host gets a LOOPBACK path bound
//! to an engine (`bind_local`), whose key is its only credential. HTTP is the front's; this decides.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use indexmap::IndexSet;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::WebSocketStream;

use super::channel::Channel;
use super::frames::{Kind, Root, TUNNEL_PATH};
use super::hub_tunnel::Tunnel;
use super::local;
use super::uploads::UploadLimits;
use crate::clock::Clock;
use crate::log::Sink;
use crate::ws::{self, Event};

/// Who may hold a tunnel: an engine this server lists, its token, and its secret once bound.
pub type Verify = Arc<dyn Fn(&str, &str, Option<&str>) -> bool + Send + Sync>;

pub struct HubOptions {
    pub verify: Verify,
    /// The key a tunnel presents to be the server's render machine; None, no tunnel is one.
    pub render_key: Option<String>,
    /// Keepalive period; a tunnel silent for two is dropped.
    pub ping_ms: u64,
    pub uploads: UploadLimits,
    /// The oldest engine wire served.
    pub min_protocol: u32,
    pub log: Sink,
}

struct LocalBinding {
    /// None is the render machine, whichever tunnel holds the render key when a client connects.
    engine_id: Option<String>,
    roots: Vec<Root>,
    kind: Kind,
    synced: bool,
}

#[derive(Default)]
struct State {
    tunnels: HashMap<String, Arc<Tunnel>>,
    locals: HashMap<String, LocalBinding>,
    /// The engines whose tunnel presented the render key, most recent last.
    renderers: IndexSet<String>,
    /// The same, kept while their tunnel is down: the server role outlives a drop.
    rendered: HashSet<String>,
    /// Engines already said to be refused for their protocol.
    outdated: HashSet<String>,
    pinging: bool,
}

pub struct Hub {
    opts: HubOptions,
    state: Mutex<State>,
    clock: Clock,
    /// Drawn per hub: a caller that sees it change knows every binding it held is gone.
    epoch: String,
}

/// Why an upgrade is refused before the 101.
#[derive(Debug, PartialEq)]
pub enum Deny {
    /// A bare status, as `deny` writes it.
    Status(u16),
    /// A status with a reason in a text body, which a supervisor's log shows.
    Reason(u16, String),
}

/// What a tunnel upgrade presented, read from its query and headers.
pub struct TunnelAsk<'a> {
    pub id: &'a str,
    pub token: &'a str,
    pub secret: Option<&'a str>,
    /// `x-studio-engine-protocol` as sent.
    pub protocol: Option<&'a str>,
    pub render_key: Option<&'a str>,
}

/// `engineProtocolOf`: anything but a positive integer, absent included, reads as 1.
pub fn protocol_of(v: Option<&str>) -> u64 {
    v.map(str::trim)
        .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|t| t.parse::<u64>().ok())
        .filter(|n| *n >= 1 && *n <= 9_007_199_254_740_991)
        .unwrap_or(1)
}

fn random_key() -> String {
    use base64::Engine as _;
    let mut b = [0u8; 24];
    getrandom::fill(&mut b).expect("the system's random source");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

impl Hub {
    pub fn new(opts: HubOptions, clock: Clock) -> Arc<Self> {
        Arc::new(Hub {
            opts,
            state: Mutex::default(),
            clock,
            epoch: random_key(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn has(&self, id: &str) -> bool {
        self.lock().tunnels.contains_key(id)
    }

    fn tunnel(&self, id: &str) -> Option<Arc<Tunnel>> {
        self.lock().tunnels.get(id).cloned()
    }

    /// A channel to engine `id`, or None when it has no tunnel; it reads `Open` once the engine answered.
    pub fn open(&self, id: &str, roots: Vec<Root>, kind: Kind, synced: bool) -> Option<Channel> {
        Some(self.tunnel(id)?.open(roots, kind, synced))
    }

    /// …the same, once the engine answered, or None after `timeout`.
    pub async fn open_ready(
        &self,
        id: &str,
        roots: Vec<Root>,
        timeout: Duration,
        kind: Kind,
    ) -> Option<Channel> {
        let mut ch = self.open(id, roots, kind, false)?;
        let answered = tokio::time::timeout(timeout, async {
            matches!(ch.rx.recv().await, Some(super::channel::ChannelEvent::Open))
        })
        .await;
        if answered == Ok(true) {
            Some(ch)
        } else {
            ch.tx.close();
            None
        }
    }

    /// The engine's answer to a browser's WebRTC offer.
    pub async fn signal(&self, id: &str, sdp: &str, timeout: Duration) -> Result<String, String> {
        let t = self.tunnel(id).ok_or("this engine has no tunnel")?;
        t.signal(sdp, timeout).await
    }

    /// A loopback path reaching engine `id` (None: the render machine) with `roots` shared.
    pub fn bind_local(
        &self,
        engine_id: Option<&str>,
        roots: Vec<Root>,
        kind: Kind,
        synced: bool,
    ) -> String {
        self.bind_local_as(random_key(), engine_id, roots, kind, synced)
    }

    /// …under `key`, one its caller held before this hub's process restarted.
    pub fn bind_local_as(
        &self,
        key: String,
        engine_id: Option<&str>,
        roots: Vec<Root>,
        kind: Kind,
        synced: bool,
    ) -> String {
        let binding = LocalBinding {
            engine_id: engine_id.map(str::to_string),
            roots,
            kind,
            synced: synced && engine_id.is_some(),
        };
        self.lock().locals.insert(key.clone(), binding);
        format!("{TUNNEL_PATH}/local/{key}")
    }

    /// Forgets a bound path's key; a socket already upgraded through it keeps working.
    pub fn unbind_local(&self, path: &str) {
        if let Some(key) = local_key(path) {
            self.lock().locals.remove(key);
        }
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    pub fn renders_enabled(&self) -> bool {
        self.opts.render_key.is_some()
    }

    /// The render machine's id while its tunnel is up, the most recent if several hold the key.
    pub fn renderer(&self) -> Option<String> {
        let s = self.lock();
        s.renderers
            .iter()
            .rfind(|id| s.tunnels.contains_key(*id))
            .cloned()
    }

    /// Whether engine `id`'s last tunnel presented the render key, up or not.
    pub fn is_renderer(&self, id: &str) -> bool {
        self.lock().rendered.contains(id)
    }

    /// Marks an engine as the render machine without a tunnel, for the testbed's stubs.
    #[cfg(feature = "test-hooks")]
    pub fn mark_rendered(&self, id: &str) {
        self.lock().rendered.insert(id.to_string());
    }

    /// The checks before a tunnel's 101, in `acceptTunnel`'s order; Ok says whether it is the render machine.
    pub fn check_tunnel(&self, ask: &TunnelAsk) -> Result<bool, Deny> {
        // Checked BEFORE the incumbent is touched: a refused newcomer leaves the live tunnel where it is.
        if ask.id.is_empty()
            || ask.token.is_empty()
            || !(self.opts.verify)(ask.id, ask.token, ask.secret)
        {
            return Err(Deny::Status(403));
        }
        let protocol = protocol_of(ask.protocol);
        let min = u64::from(self.opts.min_protocol);
        if protocol < min {
            let reason = format!(
                "this engine speaks protocol {protocol} and this server needs {min} at least — update k7 on this machine"
            );
            if self.lock().outdated.insert(ask.id.to_string()) {
                (self.opts.log)(&format!("{} refused: {reason}", ask.id));
            }
            return Err(Deny::Reason(426, reason));
        }
        self.lock().outdated.remove(ask.id);
        let offered = ask.render_key.unwrap_or("");
        let renders = !offered.is_empty() && self.opts.render_key.is_some();
        // A wrong key is refused, not ignored: a render machine that is not one must say so in its log.
        if !offered.is_empty() {
            let key = self.opts.render_key.as_deref().unwrap_or("");
            if !renders || !bool::from(offered.as_bytes().ct_eq(key.as_bytes())) {
                return Err(Deny::Status(403));
            }
        }
        Ok(renders)
    }

    /// Serves a tunnel whose upgrade passed `check_tunnel`, until its socket ends.
    pub async fn serve_tunnel<S>(self: Arc<Self>, id: String, renders: bool, ws: WebSocketStream<S>)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        self.start_pinging();
        let (peer, mut events) = ws::spawn(ws);
        let t = Tunnel::new(
            id.clone(),
            peer,
            self.opts.log.clone(),
            self.opts.uploads,
            self.clock.clone(),
        );
        let old = {
            let mut s = self.lock();
            let old = s.tunnels.insert(id.clone(), t.clone());
            // Re-added, so the newest render machine is the one a render reaches.
            s.renderers.shift_remove(&id);
            if renders {
                s.renderers.insert(id.clone());
                s.rendered.insert(id.clone());
            } else {
                s.rendered.remove(&id);
            }
            old
        };
        // One tunnel per engine: a reconnect, verified above, replaces the socket it outlived.
        if let Some(old) = old {
            old.peer.terminate();
        }
        (self.opts.log)(&format!(
            "{id} up{}",
            if renders { " (render machine)" } else { "" }
        ));
        while let Some(ev) = events.recv().await {
            match ev {
                Event::Frame(f) => t.on_frame(f),
                Event::Pong => t.alive.store(true, Ordering::Release),
                Event::Closed(_) => break,
            }
        }
        t.peer.terminate();
        t.shutdown();
        let mine = {
            let mut s = self.lock();
            let mine = s.tunnels.get(&id).is_some_and(|x| Arc::ptr_eq(x, &t));
            if mine {
                s.tunnels.remove(&id);
                s.renderers.shift_remove(&id);
            }
            mine
        };
        if mine {
            (self.opts.log)(&format!("{id} down"));
        }
    }

    /// The checks before a bound path's 101: 403 off loopback or for a key never bound, 503 with no tunnel.
    pub fn check_local(&self, key: &str, loopback: bool) -> Result<local::Target, Deny> {
        let s = self.lock();
        let Some(b) = s.locals.get(key).filter(|_| loopback) else {
            return Err(Deny::Status(403));
        };
        let engine = b.engine_id.clone().or_else(|| {
            s.renderers
                .iter()
                .rfind(|id| s.tunnels.contains_key(*id))
                .cloned()
        });
        match engine.and_then(|e| s.tunnels.get(&e).cloned()) {
            Some(tunnel) => Ok(local::Target {
                tunnel,
                roots: b.roots.clone(),
                kind: b.kind,
                synced: b.synced,
            }),
            None => Err(Deny::Status(503)),
        }
    }

    fn start_pinging(self: &Arc<Self>) {
        if std::mem::replace(&mut self.lock().pinging, true) {
            return;
        }
        let hub = Arc::downgrade(self);
        let every = Duration::from_millis(self.opts.ping_ms);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.tick().await;
            loop {
                tick.tick().await;
                let Some(hub) = hub.upgrade() else { return };
                let tunnels: Vec<_> = hub.lock().tunnels.values().cloned().collect();
                for t in tunnels {
                    if !t.alive.swap(false, Ordering::AcqRel) {
                        t.peer.terminate();
                    } else {
                        t.peer.ping();
                    }
                }
            }
        });
    }

    /// Cuts every tunnel, as a stopping server does.
    pub fn stop(&self) {
        let tunnels: Vec<_> = self.lock().tunnels.values().cloned().collect();
        for t in tunnels {
            t.peer.terminate();
        }
    }
}

/// The key of `<TUNNEL_PATH>/local/<key>`, when the path is one.
pub fn local_key(path: &str) -> Option<&str> {
    let key = path.strip_prefix(TUNNEL_PATH)?.strip_prefix("/local/")?;
    (!key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub() -> Arc<Hub> {
        Hub::new(
            HubOptions {
                verify: Arc::new(|_, _, _| true),
                render_key: None,
                ping_ms: 20_000,
                uploads: UploadLimits::default(),
                min_protocol: 1,
                log: Arc::new(|_| {}),
            },
            Clock::default(),
        )
    }

    fn status(h: &Hub, key: &str) -> u16 {
        match h.check_local(key, true) {
            Ok(_) => 101,
            Err(Deny::Status(s) | Deny::Reason(s, _)) => s,
        }
    }

    #[test]
    fn a_restarted_hub_has_another_epoch_and_takes_a_held_key_back() {
        let (a, b) = (hub(), hub());
        assert_ne!(a.epoch(), b.epoch());
        assert_eq!(a.epoch(), a.epoch());
        let path = a.bind_local(Some("e1"), vec![], Kind::Engine, true);
        let key = local_key(&path).unwrap().to_string();
        assert_eq!(key.len(), 32);
        assert_eq!(status(&a, &key), 503);
        assert_eq!(status(&b, &key), 403);
        assert_eq!(
            b.bind_local_as(key.clone(), Some("e1"), vec![], Kind::Engine, true),
            path
        );
        assert_eq!(status(&b, &key), 503);
        b.unbind_local(&path);
        assert_eq!(status(&b, &key), 403);
    }
}
