//! apply.rs — the enabled slot sent to the engine at every start, and followed afterwards: it counts as
//! playing once a sessions poll shows it loaded, and seen gone after that the part forgets it was enabled,
//! since the engine drops the slot when a project opens on it (`apps/k7-daw-engine/k7-runtime/docs/sessions.md`).

use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::sleep;

use super::{Inner, State};
use crate::engine_client::EngineClient;
use crate::log;
use crate::protocol::Session;
use crate::slots;

/// How long a spawned engine gets to listen before the slot is given up on for this start.
const LISTEN_BOUND: Duration = Duration::from_secs(30);

/// Waits for the engine's port to accept; false when it is replaced or gone meanwhile, or never listens.
async fn listening(client: &EngineClient, port: u16, generation: u64) -> bool {
    let until = Instant::now() + LISTEN_BOUND;
    while client.generation() == generation {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return true;
        }
        if Instant::now() > until {
            return false;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

impl State {
    /// The store as the app lists it, and the enabled one: read whole, so only when it changes.
    pub(super) fn refresh(&mut self) {
        let dir = self.cx.paths.slots_dir();
        self.rows = slots::list(&dir);
        self.enabled = slots::enabled(&dir).unwrap_or_default();
        let (rows, enabled) = (self.rows.clone(), self.enabled.clone());
        self.cx.patch(false, move |s| {
            s.slots.clone_from(&rows);
            s.enabled_slot.clone_from(&enabled);
        });
    }

    pub(super) fn engine_up(&mut self, port: u16) {
        self.client.set_port(port);
        self.up = true;
        self.port = port;
        // The enabled slot, at every start: a machine boots into its chain.
        self.apply();
    }

    pub(super) fn engine_down(&mut self) {
        self.client.close();
        self.up = false;
        // Gone with the engine: the next start sends the enabled slot again.
        self.pending = false;
        self.live = false;
    }

    /// Send the enabled slot to a running engine; it counts as playing once a poll shows it loaded.
    pub(super) fn apply(&mut self) {
        self.pending = false;
        self.live = false;
        if !self.up || self.enabled.is_empty() {
            return;
        }
        let Some(doc) = slots::document(&self.cx.paths.slots_dir(), &self.enabled) else {
            return;
        };
        self.pending = true;
        log::host(&format!("slot '{}' sent to the engine", self.enabled));
        let (client, port) = (self.client.clone(), self.port);
        let generation = client.generation();
        tokio::spawn(async move {
            if listening(&client, port, generation).await {
                client.apply_project(slots::SESSION, &doc).await;
            }
        });
    }

    /// Ask the sessions while a slot is being followed: one poll in flight, none when there is nothing to follow.
    pub(super) fn poll_sessions(&mut self) {
        if !self.up || self.polling || !(self.pending || self.live) {
            return;
        }
        self.polling = true;
        let (client, inner) = (self.client.clone(), self.inner.clone());
        tokio::spawn(async move {
            let generation = client.generation();
            let sessions = client.list_sessions().await;
            let _ = inner.send(Inner::Sessions(generation, sessions));
        });
    }

    pub(super) fn inner(&mut self, done: Inner) {
        match done {
            Inner::Saved(saved, reply) => self.saved(saved, reply),
            Inner::Sessions(generation, sessions) => {
                self.polling = false;
                if let Some(s) = sessions.filter(|_| generation == self.client.generation()) {
                    self.follow(&s);
                }
            }
        }
    }

    /// After a poll: the slot seen loaded, or seen gone once it was, which is a project taking the engine over.
    fn follow(&mut self, sessions: &[Session]) {
        if self.enabled.is_empty() || !(self.pending || self.live) {
            return;
        }
        if sessions.iter().any(|s| s.id == slots::SESSION && s.loaded) {
            self.pending = false;
            self.live = true;
            return;
        }
        if !self.live {
            return;
        }
        self.live = false;
        log::host(&format!(
            "slot '{}' disabled: a project opened on the engine",
            self.enabled
        ));
        slots::set_enabled(&self.cx.paths.slots_dir(), None);
        self.refresh();
    }
}
