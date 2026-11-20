//! engine.rs — the machine's engine as its own backend runs it: the supervisor hosted over stdio, its last snapshot
//! and why it is away, its notes filed under `[host]` in the merged log, and `engine-host.json` pinned before each
//! start. The choices are in `apps/k7-daw-backend/docs/localEngine.md` and `local.md`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value, json};
use tokio::sync::watch;

use crate::host::{Backoff, Events, Options, SupervisorHost};
use crate::settings::pin_file;

/// A supervisor takes about 4.1 s from `shutdown` to its exit on Windows: 6 s lets it end on its own.
pub const STOP_GRACE: Duration = Duration::from_secs(6);

/// The last snapshot, None before the first; and why the supervisor is away, None while it answers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub snapshot: Option<Map<String, Value>>,
    pub down: Option<String>,
}

impl State {
    /// `{snapshot, down}`, as the page reads it.
    pub fn to_json(&self) -> Value {
        json!({
            "snapshot": self.snapshot.clone().map(Value::Object),
            "down": self.down,
        })
    }
}

/// What a host is told to run the engine with.
pub struct Config {
    pub bin: PathBuf,
    /// The supervisor's `$STUDIO_HOME`, `~/.k7` unless a host names another.
    pub home: PathBuf,
    /// This backend's address, which the supervisor heartbeats to first.
    pub registry: String,
    /// The folders the engine opens by path, `$STUDIO_ENGINE_ROOTS`.
    pub roots: Vec<PathBuf>,
    /// The environment the supervisor gets over this process's, before the four the host sets.
    pub env: Vec<(String, String)>,
    pub backoff: Backoff,
    /// How long a stop waits for the supervisor to end on its own before it is killed.
    pub stop_grace: Duration,
}

pub struct LocalEngine {
    host: Arc<SupervisorHost>,
    stop_grace: Duration,
    state: watch::Sender<State>,
    home: PathBuf,
    registry: String,
    log: Arc<dyn Fn(&str) + Send + Sync>,
}

/// `HH:MM:SS`, local time: the merged log's format, which the web Logs panel reads.
fn stamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// Appends `HH:MM:SS [host] text` to `file`; a log never takes the backend down.
pub fn host_logger(file: PathBuf) -> Arc<dyn Fn(&str) + Send + Sync> {
    Arc::new(move |text: &str| {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
        {
            let _ = writeln!(f, "{} [host] {text}", stamp());
        }
    })
}

/// The roots joined as the OS joins a path list.
fn path_list(roots: &[PathBuf]) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    roots
        .iter()
        .map(|r| r.display().to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

impl LocalEngine {
    pub fn new(config: Config) -> Arc<Self> {
        Self::with_log(config.home.join(".studio.log"), config)
    }

    /// `new`, with the merged log at `log_file`.
    pub fn with_log(log_file: PathBuf, config: Config) -> Arc<Self> {
        let log = host_logger(log_file);
        let (state, _) = watch::channel(State::default());
        let mut env = config.env;
        env.push(("STUDIO_HOME".into(), config.home.display().to_string()));
        env.push(("STUDIO_REGISTRY_URL".into(), config.registry.clone()));
        env.push(("STUDIO_ENGINE_ROOTS".into(), path_list(&config.roots)));
        let (snap_tx, down_tx, l1, l2) = (state.clone(), state.clone(), log.clone(), log.clone());
        let events = Events {
            log: Box::new(move |t| l1(t)),
            snapshot: Box::new(move |snapshot| {
                snap_tx.send_replace(State {
                    snapshot: Some(snapshot),
                    down: None,
                });
            }),
            notify: Box::new(move |n| {
                let field = |k: &str| crate::protocol::js_text(n.get(k));
                l2(&format!("{}: {}", field("title"), field("body")));
            }),
            down: Box::new(move |down| {
                down_tx.send_modify(|s| s.down = down);
            }),
        };
        let host = SupervisorHost::new(
            Options {
                bin: config.bin,
                env,
                engine_pid_file: Some(config.home.join("engine.pid")),
                backoff: config.backoff,
            },
            events,
        );
        Arc::new(LocalEngine {
            host,
            stop_grace: config.stop_grace,
            state,
            home: config.home,
            registry: config.registry,
            log,
        })
    }

    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    /// Every change from now on; the receiver's current value is the state as it is.
    pub fn subscribe(&self) -> watch::Receiver<State> {
        self.state.subscribe()
    }

    pub fn send(&self, cmd: &str, arg: Option<&str>) {
        self.host.send(cmd, arg);
    }

    /// A line under `[host]` in the merged log.
    pub fn log(&self, text: &str) {
        (self.log)(text);
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Once this backend listens and its registry answers: the supervisor's first heartbeat goes to it.
    pub fn start(&self) {
        let _ = std::fs::create_dir_all(&self.home);
        if let Err(e) = pin_file(&self.home, &self.registry) {
            self.log(&format!(
                "could not pin {} in engine-host.json: {e}",
                self.registry
            ));
        }
        self.host.start();
    }

    /// Before this backend stops: the supervisor deregisters from it on the way out.
    pub async fn stop(&self) {
        self.host.stop(self.stop_grace).await;
    }
}
