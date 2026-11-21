//! lan — a device's surfaces opened from another device on the same network: the door this device opens while
//! "Available on the local network" is on, the relay that carries this device's page to another device's door,
//! and the presence of the screens a remote shows. Choices: `apps/k7-daw-backend/docs/lan.md`.
//! What a reader gets wrong: the page never dials the LAN, every byte goes through this device's own front.

mod confine;
mod door;
mod peers;
mod relay;
mod screens;
#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::routing::{any, get};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use k7_lan::{Advert, Discovery};
use tokio::sync::{oneshot, watch};

use crate::AppState;
use crate::settings::Settings;

/// The door's version: raised whenever a file route the surface uses changes shape, since peers must agree on it.
pub const API: u32 = 1;
/// How often the door's binding and the advertisement follow the switch and the engine.
const TICK: Duration = Duration::from_millis(500);
/// A failed bind is asked again after this long, not every tick.
const REBIND: Duration = Duration::from_secs(10);

/// Whether `path` is the family's on this device's own front; the door's `/lan/*` is another listener's.
pub fn is_lan_path(path: &str) -> bool {
    path == "/api/lan" || path.starts_with("/api/lan/")
}

struct Bound {
    addr: SocketAddr,
    stop: oneshot::Sender<()>,
}

pub struct Lan {
    listen: SocketAddr,
    /// `K7_LAN_OPEN=1`, or a test: the switch is on whatever the supervisor says.
    forced: AtomicBool,
    peers_env: Option<String>,
    discovery: Mutex<Option<Arc<dyn Discovery>>>,
    pub(crate) screens: screens::Screens,
    pub(crate) resolved: tokio::sync::Mutex<peers::Resolved>,
    pub(crate) http: Client<HttpConnector, Body>,
    door: Mutex<Option<Bound>>,
    door_addr: watch::Sender<Option<SocketAddr>>,
    served: Mutex<Option<(Instant, Option<door::ServedInfo>)>>,
    rebind_at: Mutex<Option<Instant>>,
}

impl Lan {
    /// The family's state on a device with the engine core; a server never opens a door.
    pub fn start(settings: &Settings) -> Option<Arc<Lan>> {
        if !settings.local_user || !settings.engine_core {
            return None;
        }
        Some(Arc::new(Lan {
            listen: settings.lan_listen,
            forced: AtomicBool::new(settings.lan_open),
            peers_env: settings.lan_peers.clone(),
            discovery: Mutex::new(None),
            screens: screens::Screens::default(),
            resolved: Default::default(),
            http: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
            door: Mutex::new(None),
            door_addr: watch::channel(None).0,
            served: Mutex::new(None),
            rebind_at: Mutex::new(None),
        }))
    }

    /// The switch on, whatever sets it: a test, the host of a phone, or the supervisor's own state.
    pub fn set_forced(&self, open: bool) {
        self.forced.store(open, Ordering::SeqCst);
    }

    /// Where the door is bound now, None while it is closed.
    pub fn door_addr(&self) -> watch::Receiver<Option<SocketAddr>> {
        self.door_addr.subscribe()
    }

    /// The discovery this device uses, started on first use so a device nobody browses from opens no multicast socket.
    pub(crate) fn discovery(&self) -> Arc<dyn Discovery> {
        let mut slot = self.discovery.lock().unwrap_or_else(|p| p.into_inner());
        slot.get_or_insert_with(|| {
            if let Some(list) = &self.peers_env {
                let list = if list == "none" { "" } else { list };
                match k7_lan::Static::parse(list) {
                    Ok(s) => return Arc::new(s) as Arc<dyn Discovery>,
                    Err(e) => eprintln!("k7-front: {e}; no peers are listed"),
                }
                return Arc::new(k7_lan::Static::parse("").expect("empty"));
            }
            if let Some(bridge) = k7_lan::HostBridge::installed() {
                return Arc::new(bridge);
            }
            match k7_lan::MdnsSd::start() {
                Ok(m) => Arc::new(m),
                Err(e) => {
                    eprintln!("k7-front: no discovery on this network: {e}");
                    Arc::new(k7_lan::Static::parse("").expect("empty"))
                }
            }
        })
        .clone()
    }

    fn is_open(&self, state: &AppState) -> bool {
        self.forced.load(Ordering::SeqCst)
            || k7_lan::host_open()
            || state.local.as_ref().is_some_and(|l| {
                l.engine
                    .state()
                    .snapshot
                    .is_some_and(|s| s.get("lanOpen") == Some(&serde_json::Value::Bool(true)))
            })
    }

    /// Follows the switch until the front stops: binds the door and advertises while it is on, drops both when it goes off.
    pub async fn launch(
        self: Arc<Self>,
        state: Weak<AppState>,
        mut closing: watch::Receiver<bool>,
    ) {
        let mut tick = tokio::time::interval(TICK);
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                _ = closing.wait_for(|c| *c) => break,
            }
            let Some(state) = state.upgrade() else { break };
            self.follow(&state).await;
        }
        self.stop();
    }

    async fn follow(self: &Arc<Self>, state: &Arc<AppState>) {
        let entry = state
            .engine
            .as_ref()
            .and_then(|e| e.core.registry.local_entry());
        let want = self.is_open(state) && entry.is_some();
        let bound = self.door_addr.borrow().is_some();
        if want && !bound {
            self.bind(state).await;
        } else if !want && bound {
            self.unbind();
        }
        // Said again every tick: the advert is idempotent, and the engine's id or port may have moved.
        let bound = *self.door_addr.borrow();
        match (want, bound, entry) {
            (true, Some(door), Some(e)) => {
                let advert = Advert {
                    engine_id: e.id.clone(),
                    name: e.hostname.clone(),
                    door: door.port(),
                    engine: e.port as u16,
                    api: API,
                    proto: e.protocol.unwrap_or(0.0) as u32,
                };
                self.discovery().advertise(Some(&advert));
            }
            _ => {
                if let Some(d) = self
                    .discovery
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone()
                {
                    d.advertise(None);
                }
            }
        }
    }

    async fn bind(self: &Arc<Self>, state: &Arc<AppState>) {
        {
            let mut at = self.rebind_at.lock().unwrap_or_else(|p| p.into_inner());
            if at.is_some_and(|t| t > Instant::now()) {
                return;
            }
            *at = None;
        }
        let listener = match tokio::net::TcpListener::bind(self.listen).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "k7-front: the LAN door could not listen on {}: {e}",
                    self.listen
                );
                *self.rebind_at.lock().unwrap_or_else(|p| p.into_inner()) =
                    Some(Instant::now() + REBIND);
                return;
            }
        };
        let Ok(addr) = listener.local_addr() else {
            return;
        };
        let (stop, stopped) = oneshot::channel::<()>();
        let app = door::router(state.clone());
        tokio::spawn(crate::listen::serve(
            listener,
            app,
            crate::listen::Limits::default(),
            async move {
                let _ = stopped.await;
            },
        ));
        *self.door.lock().unwrap_or_else(|p| p.into_inner()) = Some(Bound { addr, stop });
        self.door_addr.send_replace(Some(addr));
        eprintln!("k7-front: the LAN door listens on {addr}");
    }

    fn unbind(&self) {
        if let Some(b) = self.door.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = b.stop.send(());
            eprintln!("k7-front: the LAN door is closed ({})", b.addr);
        }
        self.door_addr.send_replace(None);
    }

    /// Closes the door and stops saying this device is here.
    pub fn stop(&self) {
        self.unbind();
        if let Some(d) = self
            .discovery
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        {
            d.advertise(None);
        }
    }
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// The routes this device's page reaches, behind the loopback guard and its session.
pub fn routes() -> Router<Arc<AppState>> {
    let p = |m: axum::routing::MethodRouter<Arc<AppState>>| m.fallback(crate::declined);
    Router::new()
        .route("/api/lan/peers", p(get(peers::list)))
        .route("/api/lan/self", p(get(peers::this_device)))
        .route("/api/lan/screens", p(get(door::screens_here)))
        .route("/api/lan/{id}/hello", any(relay::hello))
        .route("/api/lan/{id}/screens/{*rest}", any(relay::screens))
        .route("/api/lan/{id}/files/{*rest}", any(relay::files))
        .route("/api/lan/{id}/engine", p(get(relay::engine)))
}
