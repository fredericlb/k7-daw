//! jobs — what the account asks of its engine beyond playing, as models with no I/O (1k, 2302): the automatic
//! freezer as events in and effects out, the plugin pair, the audition's window, the separations' registry and a
//! render's manifest. No tokio, no process, no clock of its own: the front runs the timer, the compiles and the children.
//! What a reader gets wrong: a pure function here repeats the TypeScript's SENTENCES, which a body is compared by.

pub mod base_path;
pub mod freezer;
pub mod manifest;
pub mod plugin;
pub mod preview;
pub mod separations;

use std::time::{Duration, Instant};

pub use song::freeze::AutoFreeze;

/// How long the engine must sit stopped with no edit before a thawed module is frozen again (`THAW_IDLE_MS`).
pub const THAW_IDLE: Duration = Duration::from_secs(20);
/// How often the front runs a pass over each served project.
pub const FREEZE_EVERY: Duration = Duration::from_secs(3);
/// The plugin queries' waits: the parameters, the native window, the saved base (`enginePluginQueries.ts`).
pub const PLUGIN_PARAMS_TIMEOUT: Duration = Duration::from_secs(20);
pub const NATIVE_UI_TIMEOUT: Duration = Duration::from_secs(30);
pub const PLUGIN_BASE_TIMEOUT: Duration = Duration::from_secs(5);
/// A render a machine makes is let go of after this (`MACHINE_RENDER_TIMEOUT_MS`), a local one's child is killed after it.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// What happens to one project folder's freezer. Decision 2302; the added fields are decision 2302's B1 (pending the user).
#[derive(Clone, Debug, PartialEq)]
pub enum FreezeEvent {
    /// One pass is due: the project's revision, whether the transport runs, the time.
    Tick {
        rev: String,
        playing: bool,
        now: Instant,
    },
    /// The compile asked by `Compile` finished; `playing` is read again after it, as the TypeScript does.
    Compiled {
        freezes: Vec<AutoFreeze>,
        playing: bool,
    },
    /// The compile, or any step of the pass, threw: the sentence of the error.
    CompileFailed(String),
    /// The render asked by `Render` ended; `made` is whether its file exists, whatever `error` says.
    Rendered {
        module: String,
        made: bool,
        error: Option<String>,
    },
    /// A knob or a plugin window asks `module` to stay live (`on`) or freeze again.
    Thaw {
        module: String,
        on: bool,
        now: Instant,
    },
    /// `renderAllNow`: every missing render now, transport or not.
    RenderAll,
}

/// What the front does next; the machine itself touches nothing.
// `Render(AutoFreeze)` is the shape the brief fixed; an effect lives one hop, so its size costs nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum FreezeEffect {
    Compile,
    Render(AutoFreeze),
    Reload,
    SetThawed(Vec<String>),
    Log(String),
    /// The modules a pass rendered (empty when it did nothing); one per `Tick`.
    Done(Vec<String>),
    /// `renderAllNow`'s progress lines.
    Progress {
        done: usize,
        total: usize,
        module: String,
    },
    /// `renderAllNow`'s answer: the modules no connected machine can make, with their plugins' names.
    Impossible(Vec<(String, Vec<String>)>),
    /// `renderAllNow`'s compile or folder failed: the share gets this sentence.
    RenderAllFailed(String),
}
