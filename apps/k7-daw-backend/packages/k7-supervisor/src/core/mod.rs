//! core — the supervisor proper: one task owns the state (the engine's lifecycle, the settings, the snapshot)
//! and everything else reaches it by message, so nothing needs a lock. It starts or adopts the engine, relaunches
//! one that died through the crash loop, answers `engine.*` and `shutdown`, routes every other command to the
//! part that named it, and emits a snapshot only when its bytes changed. Blocking work runs off this task.
//! The paces and the order of a shutdown: `apps/k7-daw-backend/packages/k7-supervisor/docs/supervisorPolicy.md`.

mod ctx;

pub use ctx::{Ack, Ctx, EngineAsk, EngineUp, EngineView, Event, PartSpec, Patch};

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{MissedTickBehavior, interval, timeout};

use crate::crash_loop::{self, CrashLoop};
use crate::engine::{Engine, StartAsk, StartKind, Started};
use crate::frames::Frames;
use crate::host_lock::{Holder, HostLock};
use crate::identity::{self, Identity};
use crate::log;
use crate::paths::{self, Paths};
use crate::protocol::{Command, SafeMode, Snapshot, result_frame, state_frame};
use crate::settings::{Settings, Store};
use ctx::ToCore;

/// The engine's crash watch.
const WATCH: Duration = Duration::from_secs(2);
/// A hung registry must not hold the quit open: an app gives a few seconds in all.
const DEREGISTER_BOUND: Duration = Duration::from_millis(1500);

/// What drives the core from outside: the app's commands, and the end of the app or a signal.
#[derive(Debug)]
pub enum Input {
    Command(Command),
    Shutdown,
}

/// Everything a run is started with.
pub struct Boot {
    pub paths: Paths,
    pub store: Store,
    pub frames: Frames,
    /// `$STUDIO_ENGINE`.
    pub engine_binary: Option<PathBuf>,
    pub parts: Vec<PartSpec>,
}

enum Msg {
    Started {
        started: Started,
        safe: u32,
        reply: Option<i64>,
    },
}

struct PartHandle {
    name: &'static str,
    tx: mpsc::UnboundedSender<Event>,
}

struct Core {
    paths: Paths,
    store: Store,
    frames: Frames,
    identity: Identity,
    settings: Settings,
    settings_tx: watch::Sender<Settings>,
    view_tx: watch::Sender<EngineView>,
    engine: Engine,
    lock: HostLock,
    snap: Snapshot,
    patches: Vec<(&'static str, bool, Patch)>,
    parts: Vec<PartHandle>,
    routes: HashMap<&'static str, usize>,
    last_frame: String,
    crashes: CrashLoop,
    starting: bool,
    shutting_down: bool,
    origins: String,
    tx: mpsc::UnboundedSender<Msg>,
}

/// Run the supervisor until `inputs` ends or says Shutdown; Err with why it could not start.
pub async fn run(boot: Boot, mut inputs: mpsc::UnboundedReceiver<Input>) -> Result<(), String> {
    let lock = HostLock::new(boot.paths.host_pid());
    // Before anything is spawned or adopted: a second supervisor would adopt the FIRST one's engine.
    match lock.acquire() {
        Holder::Nobody => {}
        Holder::Pid(p) => {
            return Err(format!(
                "an engine supervisor is already running (pid {p}) — quit it first"
            ));
        }
        Holder::Unknown => {
            return Err("an engine supervisor is already running (pid -1) — quit it first".into());
        }
    }
    let settings = boot.store.load();
    let identity = identity::ensure(&boot.paths.engine_id());
    let (settings_tx, settings_rx) = watch::channel(settings.clone());
    let (view_tx, view_rx) = watch::channel(EngineView::default());
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let (part_tx, mut part_rx) = mpsc::unbounded_channel::<ToCore>();
    let engine = Engine::new(
        boot.paths.clone(),
        identity.clone(),
        boot.frames.clone(),
        boot.engine_binary,
    );
    let mut core = Core {
        snap: Snapshot {
            engine_port: settings.port,
            ..Snapshot::default()
        },
        paths: boot.paths,
        store: boot.store,
        frames: boot.frames,
        identity,
        settings,
        settings_tx,
        view_tx,
        engine,
        lock,
        patches: Vec::new(),
        parts: Vec::new(),
        routes: HashMap::new(),
        last_frame: String::new(),
        crashes: CrashLoop::default(),
        starting: false,
        shutting_down: false,
        origins: String::new(),
        tx: tx.clone(),
    };
    for spec in boot.parts {
        let (ptx, prx) = mpsc::unbounded_channel();
        let cx = Ctx {
            name: spec.name,
            paths: core.paths.clone(),
            identity: core.identity.clone(),
            frames: core.frames.clone(),
            settings: settings_rx.clone(),
            engine: view_rx.clone(),
            to_core: part_tx.clone(),
        };
        for c in spec.commands {
            core.routes.insert(c, core.parts.len());
        }
        core.parts.push(PartHandle {
            name: spec.name,
            tx: ptx,
        });
        (spec.start)(cx, prx);
    }
    // The first snapshot: the app draws nothing until it arrives.
    core.publish();
    log::host("supervisor started (rust)");
    let acks = core.broadcast(Event::Boot);
    for ack in acks {
        let _ = ack.await;
    }
    // What the parts asked during their boot (origins, settings) is in before the engine starts.
    while let Ok(m) = part_rx.try_recv() {
        core.on_part(m);
    }
    core.start_engine(None, 0);
    let mut watch_tick = interval(WATCH);
    let mut lock_tick = interval(crate::host_lock::REFRESH);
    watch_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    lock_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            input = inputs.recv() => match input {
                Some(Input::Command(c)) if c.cmd == "shutdown" => break,
                Some(Input::Command(c)) => core.on_command(c),
                Some(Input::Shutdown) | None => break,
            },
            Some(m) = rx.recv() => core.on_msg(m),
            Some(m) = part_rx.recv() => core.on_part(m),
            _ = watch_tick.tick() => core.crash_watch(),
            _ = lock_tick.tick() => core.lock.refresh(),
        }
    }
    core.shutdown().await;
    Ok(())
}

/// Every ack awaited, at most `bound` in all.
async fn settle(acks: Vec<oneshot::Receiver<()>>, bound: Duration) {
    let _ = timeout(bound, async {
        for a in acks {
            let _ = a.await;
        }
    })
    .await;
}

impl Core {
    fn publish(&mut self) {
        let mut s = self.snap.clone();
        for (_, _, p) in &self.patches {
            p(&mut s);
        }
        s.engine_id = self.identity.id.clone();
        s.audio_priority = self.settings.audio_priority.clone();
        s.lan_open = self.settings.lan_open;
        s.root_path = paths::utf8(&self.paths.root_dir());
        s.merged_log_path = paths::utf8(&self.paths.merged_log());
        s.status.clone_from(&self.snap.status);
        s.error.clone_from(&self.snap.error);
        s.safe_mode.clone_from(&self.snap.safe_mode);
        s.owns_engine = self.snap.owns_engine;
        s.user_stopped = self.snap.user_stopped;
        s.engine_port = self.snap.engine_port;
        let bytes = state_frame(&s).to_string();
        if bytes != self.last_frame {
            self.frames.emit_line(&bytes);
            self.last_frame = bytes;
        }
    }

    /// `make(ack)` sent to every part; the acks to wait on.
    fn broadcast(&self, make: impl Fn(Ack) -> Event) -> Vec<oneshot::Receiver<()>> {
        let mut acks = Vec::new();
        for p in &self.parts {
            let (a, r) = oneshot::channel();
            if p.tx.send(make(a)).is_ok() {
                acks.push(r);
            }
        }
        acks
    }

    fn tell_all(&self, make: impl Fn() -> Event) {
        for p in &self.parts {
            let _ = p.tx.send(make());
        }
    }

    fn reply(&self, id: i64, ok: bool, message: &str) {
        self.frames.emit(&result_frame(id, ok, message));
    }

    fn on_part(&mut self, m: ToCore) {
        match m {
            ToCore::Patch {
                part,
                engine_scoped,
                patch,
            } => {
                match self
                    .patches
                    .iter_mut()
                    .find(|p| p.0 == part && p.1 == engine_scoped)
                {
                    Some(slot) => *slot = (part, engine_scoped, patch),
                    None => self.patches.push((part, engine_scoped, patch)),
                }
                // Applied in the parts' order, whatever order they first patched in.
                let order = |n: &str| {
                    self.parts
                        .iter()
                        .position(|p| p.name == n)
                        .unwrap_or(usize::MAX)
                };
                let mut keyed: Vec<_> = std::mem::take(&mut self.patches)
                    .into_iter()
                    .map(|p| (order(p.0), p))
                    .collect();
                keyed.sort_by_key(|k| (k.0, k.1.1));
                self.patches = keyed.into_iter().map(|k| k.1).collect();
            }
            ToCore::Settings(f) => {
                f(&mut self.settings);
                self.store.save(&self.settings);
                let _ = self.settings_tx.send(self.settings.clone());
            }
            ToCore::Origins(o) => self.origins = o,
            ToCore::SafeMode(m) => self.snap.safe_mode = Some(m),
            ToCore::Engine(EngineAsk::Replace(why)) => {
                self.engine.stop();
                self.snap.owns_engine = false;
                self.set_error(&why);
                self.snap.safe_mode = None;
                self.engine_down(false, None);
                self.start_engine(None, 0);
            }
            ToCore::Engine(EngineAsk::Kill) => {
                if self.snap.owns_engine {
                    self.engine.stop();
                }
            }
        }
        self.publish();
    }

    fn set_error(&mut self, why: &str) {
        self.snap.status = "error".into();
        self.snap.error = why.into();
    }

    fn on_command(&mut self, c: Command) {
        match c.cmd.as_str() {
            "engine.start" => {
                // Asked by the user, so a normal start whatever the crashes were: the way out of safe mode.
                self.crashes.reset();
                self.start_engine(Some(c.id), 0);
            }
            "engine.stop" => {
                let acks = self.stop_engine();
                let frames = self.frames.clone();
                // Out of the pickers NOW rather than aging out, which is what Stop is beside Restart.
                tokio::spawn(async move {
                    settle(acks, DEREGISTER_BOUND).await;
                    frames.emit(&result_frame(c.id, true, ""));
                });
            }
            "engine.restart" => {
                let _ = self.stop_engine();
                self.start_engine(Some(c.id), 0);
            }
            name => match self.routes.get(name).and_then(|&i| self.parts.get(i)) {
                Some(p) => {
                    let _ = p.tx.send(Event::Command(c));
                }
                None => self.reply(c.id, false, &format!("{name} is not a supervisor command")),
            },
        }
    }

    /// Spawn or adopt off this task, in safe mode when `safe` > 0; the result comes back as a message.
    fn start_engine(&mut self, reply: Option<i64>, safe: u32) {
        self.snap.user_stopped = false;
        self.starting = true;
        let ask = StartAsk {
            first_port: self.settings.port,
            port_fixed: self.settings.port_fixed,
            origins: self.origins.clone(),
            safe_mode_crashes: safe,
            lan: self.settings.lan_open,
        };
        let engine = self.engine.clone();
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let started = engine.start(&ask);
            let _ = tx.send(Msg::Started {
                started,
                safe,
                reply,
            });
        });
    }

    fn on_msg(&mut self, m: Msg) {
        let Msg::Started {
            started,
            safe,
            reply,
        } = m;
        self.starting = false;
        if started.port > 0 && started.port != self.snap.engine_port {
            log::host(&format!(
                "the engine serves on :{}, which the presence now carries",
                started.port
            ));
            self.snap.engine_port = started.port;
        }
        match &started.kind {
            StartKind::Spawned | StartKind::Adopted => {
                self.snap.owns_engine = started.kind == StartKind::Spawned;
                self.snap.status = "running".into();
                self.snap.error.clear();
            }
            StartKind::Missing => self.set_error("engine binary not found (build it)"),
            StartKind::Error(why) => self.set_error(&format!("spawn failed: {why}")),
        }
        let up = self.snap.status == "running";
        if up {
            self.crashes.started(Instant::now());
            let spawned_safe = started.kind == StartKind::Spawned && safe > 0;
            self.snap.safe_mode = spawned_safe.then(|| SafeMode {
                crashes: safe,
                suspect: String::new(),
            });
            let _ = self.view_tx.send(EngineView {
                up: true,
                port: self.snap.engine_port,
                owns: self.snap.owns_engine,
            });
            let ev = EngineUp {
                port: self.snap.engine_port,
                adopted: started.kind == StartKind::Adopted,
                safe_mode_crashes: safe,
            };
            self.tell_all(|| Event::EngineUp(ev));
        } else {
            // The app shows the error; the log is what is still there tomorrow.
            log::host(&format!("engine did not start: {}", self.snap.error));
            // A start the user asked for is no crash watch's: only a relaunch backs off.
            if reply.is_none() {
                self.crashes.start_failed(Instant::now());
            }
        }
        self.publish();
        if let Some(id) = reply {
            let error = self.snap.error.clone();
            self.reply(id, up, &error);
        }
    }

    fn engine_down(&mut self, user: bool, ack: Option<&mut Vec<oneshot::Receiver<()>>>) {
        let _ = self.view_tx.send(EngineView {
            up: false,
            port: self.snap.engine_port,
            owns: false,
        });
        match ack {
            Some(acks) => acks.extend(self.broadcast(|a| Event::EngineDown { user, ack: Some(a) })),
            None => self.tell_all(|| Event::EngineDown { user, ack: None }),
        }
    }

    /// The user's stop: the crash count goes, only an engine we OWN is killed; the acks of the parts that deregister.
    fn stop_engine(&mut self) -> Vec<oneshot::Receiver<()>> {
        self.snap.user_stopped = true;
        self.crashes.reset();
        self.snap.safe_mode = None;
        if self.snap.owns_engine {
            self.engine.stop();
        }
        self.snap.owns_engine = false;
        self.snap.status = "stopped".into();
        self.snap.error.clear();
        // Gone with the engine: what the parts read of it.
        self.patches.retain(|p| !p.1);
        let mut acks = Vec::new();
        self.engine_down(true, Some(&mut acks));
        self.publish();
        acks
    }

    /// Relaunch an engine that died unasked, backing off and then in safe mode when it keeps dying.
    fn crash_watch(&mut self) {
        if self.shutting_down || self.snap.user_stopped {
            self.crashes.reset();
            return;
        }
        if self.snap.status == "stopped" || self.starting || self.engine.running() {
            return;
        }
        let now = Instant::now();
        if self.crashes.up() {
            self.crashes.died(now);
            log::host(&format!(
                "engine is gone ({} quick crash(es) in a row)",
                self.crashes.quick_crashes()
            ));
            self.set_error("engine exited — restarting…");
            self.engine_down(false, None);
            self.publish();
        }
        if !self.crashes.due(now) {
            return;
        }
        // In safe mode the count says how many crashes put it there, never less, even after a long run.
        let safe = if self.crashes.safe_mode() {
            self.crashes
                .quick_crashes()
                .max(crash_loop::SAFE_MODE_AFTER)
        } else {
            0
        };
        log::host(if safe > 0 {
            "relaunching the engine in SAFE MODE: no external plugin will load until it is restarted by hand"
        } else {
            "relaunching the engine"
        });
        self.start_engine(None, safe);
    }

    /// Parts told first and given 1.5 s, then the engine stopped, adopted or not: one left listening holds the device.
    async fn shutdown(&mut self) {
        self.shutting_down = true;
        settle(self.broadcast(Event::Shutdown), DEREGISTER_BOUND).await;
        let engine = self.engine.clone();
        let _ = tokio::task::spawn_blocking(move || engine.stop()).await;
        self.lock.release();
    }
}
