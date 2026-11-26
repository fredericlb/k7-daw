//! ctx.rs — what a PART sees of the core: the paths, the identity, the settings and the engine as watched
//! values, and the few things it may ask (a slice of the snapshot, a settings change, the engine's origins,
//! a restart). A part is a task of its own fed `Event`s; it is listed by one line in `parts/mod.rs`.
//! What a reader gets wrong: a part never writes the settings file, the core does, as its one writer.

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, watch};

use crate::frames::Frames;
use crate::identity::Identity;
use crate::paths::Paths;
use crate::protocol::{Command, SafeMode, Snapshot, result_frame};
use crate::settings::Settings;

/// Answered once the part is done with what it was told; the core waits on it, bounded.
pub type Ack = oneshot::Sender<()>;

/// A part's slice of the snapshot, applied over the core's on every publish.
pub type Patch = Arc<dyn Fn(&mut Snapshot) + Send + Sync>;

/// The engine as the parts follow it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EngineView {
    pub up: bool,
    pub port: u16,
    pub owns: bool,
}

/// An engine just up, spawned or adopted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineUp {
    pub port: u16,
    pub adopted: bool,
    pub safe_mode_crashes: u32,
}

/// What the core tells a part.
#[derive(Debug)]
pub enum Event {
    /// Once, before the engine first starts: what the start needs from this part.
    Boot(Ack),
    EngineUp(EngineUp),
    /// The engine went away; `user` when stopped by hand, whose `ack` the stop's reply waits for.
    EngineDown {
        user: bool,
        ack: Option<Ack>,
    },
    /// A command this part named in its spec; the part answers it with `Ctx::reply`.
    Command(Command),
    /// The supervisor is going: deregister, close, ack; the core waits 1.5 s at most.
    Shutdown(Ack),
}

/// What a part may ask of the engine's lifecycle.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineAsk {
    /// Stop it and start a normal one, saying `why` meanwhile: an adopted engine of another build.
    Replace(String),
    /// Kill the engine we own and let the crash watch relaunch it: a wedged one.
    Kill,
}

pub(crate) enum ToCore {
    Patch {
        part: &'static str,
        engine_scoped: bool,
        patch: Patch,
    },
    Settings(Box<dyn FnOnce(&mut Settings) + Send>),
    Origins(String),
    SafeMode(SafeMode),
    Engine(EngineAsk),
}

/// One part: its name, the commands it answers, and how it is started.
pub struct PartSpec {
    pub name: &'static str,
    pub commands: &'static [&'static str],
    /// Spawns the part's task; called once, inside the runtime.
    pub start: fn(Ctx, mpsc::UnboundedReceiver<Event>),
}

#[derive(Clone)]
pub struct Ctx {
    pub name: &'static str,
    pub paths: Paths,
    pub identity: Identity,
    pub frames: Frames,
    pub settings: watch::Receiver<Settings>,
    pub engine: watch::Receiver<EngineView>,
    pub(crate) to_core: mpsc::UnboundedSender<ToCore>,
}

impl Ctx {
    /// This part's slice from now on, replacing its last of the same scope; an `engine_scoped` one goes with a stop.
    pub fn patch(&self, engine_scoped: bool, f: impl Fn(&mut Snapshot) + Send + Sync + 'static) {
        let patch: Patch = Arc::new(f);
        let _ = self.to_core.send(ToCore::Patch {
            part: self.name,
            engine_scoped,
            patch,
        });
    }

    /// A settings change, applied and saved by the core, then seen on `settings`.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings) + Send + 'static) {
        let _ = self.to_core.send(ToCore::Settings(Box::new(f)));
    }

    /// The origins the next engine start lets in (`STUDIO_ENGINE_ALLOWED_ORIGINS`).
    pub fn set_origins(&self, origins: String) {
        let _ = self.to_core.send(ToCore::Origins(origins));
    }

    /// The safe mode the running engine says it is in, with its suspect.
    pub fn set_safe_mode(&self, mode: SafeMode) {
        let _ = self.to_core.send(ToCore::SafeMode(mode));
    }

    pub fn ask(&self, ask: EngineAsk) {
        let _ = self.to_core.send(ToCore::Engine(ask));
    }

    /// The `result` frame for command `id`.
    pub fn reply(&self, id: i64, ok: bool, message: &str) {
        self.frames.emit(&result_frame(id, ok, message));
    }
}
