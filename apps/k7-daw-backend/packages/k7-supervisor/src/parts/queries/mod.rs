//! queries — the part that talks to the engine over its WebSocket: asks who it is on every start (replace a
//! foreign build or a safe one we adopted, name the suspect), watches for a wedged message thread, polls the
//! interfaces (and switches by the user's ranking), the catalogue, the plugin instances, the sessions and the
//! pairing lists, and answers the commands that act on them. One call is ever in flight (`engine_client`).
//! What a reader gets wrong: a result is kept only if the engine it came from is still the one running.

mod commands;
mod inspect;
mod polls;
mod view;

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::{Notify, mpsc};
use tokio::time::sleep;

use crate::core::{Ctx, Event, PartSpec};
use crate::engine_client::EngineClient;
use crate::protocol::Command;
use view::View;

/// How often each poll runs; the first of each runs at once when the engine comes up.
#[derive(Debug, Clone, Copy)]
pub struct Paces {
    pub hang: Duration,
    pub devices: Duration,
    pub edit_plugins: Duration,
    pub sessions: Duration,
    pub pairing: Duration,
    /// The catalogue once it is known and the scan is over.
    pub plugins: Duration,
    /// The catalogue until it lands, and while the engine says a scan runs.
    pub plugins_fast: Duration,
}

impl Default for Paces {
    fn default() -> Paces {
        Paces {
            hang: Duration::from_secs(10),
            devices: Duration::from_secs(10),
            edit_plugins: Duration::from_secs(3),
            sessions: Duration::from_secs(2),
            pairing: Duration::from_secs(1),
            plugins: Duration::from_secs(60),
            plugins_fast: Duration::from_secs(2),
        }
    }
}

impl Paces {
    /// Every pace `n` times faster, for a test that waits on polls.
    pub fn divided(self, n: u32) -> Paces {
        Paces {
            hang: self.hang / n,
            devices: self.devices / n,
            edit_plugins: self.edit_plugins / n,
            sessions: self.sessions / n,
            pairing: self.pairing / n,
            plugins: self.plugins / n,
            plugins_fast: self.plugins_fast / n,
        }
    }
}

static PACES: OnceLock<Paces> = OnceLock::new();

/// The paces of every run in this process, once; later calls are ignored. Only a test has a reason to.
pub fn set_paces(p: Paces) {
    let _ = PACES.set(p);
}

/// The commands this part answers, and what `parts::all` lists.
pub fn spec() -> PartSpec {
    PartSpec {
        name: "queries",
        commands: commands::NAMES,
        start,
    }
}

/// The notices a poller wakes on at once, besides its timer.
#[derive(Default)]
pub(crate) struct Kicks {
    pub devices: Notify,
    pub plugins: Notify,
    pub edit_plugins: Notify,
    pub sessions: Notify,
    pub pairing: Notify,
}

pub(crate) struct Shared {
    pub cx: Ctx,
    pub client: EngineClient,
    pub paces: Paces,
    pub view: Mutex<View>,
    pub kicks: Kicks,
    /// An adopted engine of another protocol or build was replaced once: never twice, or one that cannot be killed loops.
    pub replaced_adopted: AtomicBool,
    /// The last automatic switch asked for (device, device set); a failing one is retried only once the set moves.
    pub last_attempt: Mutex<Option<(String, String)>>,
    /// The engine said its scan was still running at the last ask, so the list is about to change.
    pub scanning: AtomicBool,
    /// A pairing poll is in flight: a slow call ahead of it must not stack a second one in the queue.
    pub pairing_busy: AtomicBool,
    /// Set once `inspect` has asked its first questions, so nothing is asked of an engine ahead of them.
    pub ready: AtomicBool,
}

impl Shared {
    /// The engine is up and has been told what this machine wants: the polls begin after the first questions.
    pub fn up(&self) -> bool {
        self.ready.load(Ordering::Acquire) && self.cx.engine.borrow().up
    }

    /// Changes the view when the engine is still the one `generation` was read from; `f` says whether it changed.
    pub fn update(&self, generation: u64, f: impl FnOnce(&mut View) -> bool) {
        let mut v = self.view.lock().unwrap_or_else(|e| e.into_inner());
        if self.client.generation() != generation || !f(&mut v) {
            return;
        }
        self.patch(&v);
    }

    fn patch(&self, v: &View) {
        let copy = Arc::new(v.clone());
        self.cx.patch(true, move |s| copy.apply(s));
    }

    /// Everything the engine told us forgotten: the engine is gone by the user's hand, and the core dropped the slice.
    fn forget(&self) {
        let mut v = self.view.lock().unwrap_or_else(|e| e.into_inner());
        *v = View::default();
        self.patch(&v);
        *self.last_attempt.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// `step` run at each pace while the engine is up, and at once when kicked; it answers the pause before its next run.
fn poller<F, Fut>(shared: &Arc<Shared>, kick: fn(&Kicks) -> &Notify, mut step: F)
where
    F: FnMut(Arc<Shared>) -> Fut + Send + 'static,
    Fut: Future<Output = Duration> + Send,
{
    let shared = shared.clone();
    tokio::spawn(async move {
        loop {
            let pause = step(shared.clone()).await;
            tokio::select! {
                () = sleep(pause) => {}
                () = kick(&shared.kicks).notified() => {}
            }
        }
    });
}

fn start(cx: Ctx, mut events: mpsc::UnboundedReceiver<Event>) {
    let paces = PACES.get().copied().unwrap_or_default();
    let port = cx.engine.borrow().port;
    let shared = Arc::new(Shared {
        client: EngineClient::new(port),
        cx,
        paces,
        view: Mutex::default(),
        kicks: Kicks::default(),
        replaced_adopted: AtomicBool::new(false),
        last_attempt: Mutex::default(),
        scanning: AtomicBool::new(false),
        pairing_busy: AtomicBool::new(false),
        ready: AtomicBool::new(false),
    });
    polls::spawn_all(&shared);
    inspect::spawn_hang_watch(&shared);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    commands::spawn_worker(&shared, cmd_rx);
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            match ev {
                Event::Boot(ack) => {
                    let _ = ack.send(());
                }
                Event::EngineUp(up) => {
                    shared.ready.store(false, Ordering::Release);
                    shared.client.set_port(up.port);
                    inspect::spawn_inspect(&shared, up);
                }
                Event::EngineDown { user, ack } => {
                    shared.client.close();
                    if user {
                        shared.forget();
                    }
                    if let Some(a) = ack {
                        let _ = a.send(());
                    }
                }
                Event::Command(c) => {
                    let _ = cmd_tx.send(c);
                }
                Event::Shutdown(ack) => {
                    shared.client.close();
                    let _ = ack.send(());
                }
            }
        }
    });
}
