//! server — the engine's WebSocket server: `start` binds the port and builds the host, `Running::run` is the
//! message loop, `serve` is both. Everything between a socket and the engine is a module here, one concern each.
//! What a reader gets wrong: the port is bound before the host's loop runs and so before the audio device
//! opens (a refused connection reads as an absent engine), and `whoami` is answered with the loop not yet running.

pub mod conn;
pub mod ctx;
pub mod dispatch;
pub mod handlers;
pub mod held_notes;
pub mod msg;
pub mod outbox;
mod project;
pub mod supersede;
mod transport;
mod whoami;

use crate::engine::{Api, Engine, Host};
use crate::pairing::gate::{Gate, GateConfig};
use crate::pairing::store::Pairing;
use conn::Timers;
use ctx::{Ctx, Identity};
use handlers::HandlerTable;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;
use tokio::sync::watch;

/// How long a task still running is given when the server stops.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// What the server is started with; `from_env` reads the supervisor's variables.
pub struct ServeConfig {
    pub port: u16,
    pub bind_host: String,
    pub engine_id: String,
    pub engine_token: String,
    pub audio_to_browser: bool,
    pub safe_mode_crashes: u32,
    pub allowed_origins: Vec<String>,
    /// The paired devices' file; `None` is `paired-devices.json` in the engine's state folder.
    pub pairing_file: Option<std::path::PathBuf>,
    pub timers: Timers,
    /// Whether SIGINT and SIGTERM quit the loop: off in a test, which shares its process.
    pub handle_signals: bool,
    /// The handlers beyond the core's own: a milestone's table lines come through `handlers::table`, a test's here.
    pub extra_handlers: Option<fn(&mut HandlerTable)>,
}

impl ServeConfig {
    /// Loopback on `port`, no identity, nothing optional: what a test starts from.
    pub fn new(port: u16) -> Self {
        ServeConfig {
            port,
            bind_host: "127.0.0.1".into(),
            engine_id: String::new(),
            engine_token: String::new(),
            audio_to_browser: false,
            safe_mode_crashes: 0,
            allowed_origins: Vec::new(),
            pairing_file: None,
            timers: Timers::default(),
            handle_signals: false,
            extra_handlers: None,
        }
    }

    /// The supervisor's environment: `STUDIO_ENGINE_ID`, `_TOKEN`, `_HOST`, `_SAFE_MODE`, `_ALLOWED_ORIGINS`, `STUDIO_AUDIO_TO_BROWSER`.
    pub fn from_env(port: u16) -> Self {
        let var = |k: &str| std::env::var(k).unwrap_or_default();
        let mut c = ServeConfig::new(port);
        c.engine_id = var("STUDIO_ENGINE_ID");
        c.engine_token = var("STUDIO_ENGINE_TOKEN");
        let host = var("STUDIO_ENGINE_HOST");
        if !host.is_empty() {
            c.bind_host = host;
        }
        c.safe_mode_crashes = var("STUDIO_ENGINE_SAFE_MODE").trim().parse().unwrap_or(0);
        c.allowed_origins =
            crate::pairing::net::parse_origins(&var("STUDIO_ENGINE_ALLOWED_ORIGINS"));
        c.audio_to_browser = var("STUDIO_AUDIO_TO_BROWSER") == "1";
        c.handle_signals = true;
        c
    }
}

/// The server up and listening, its host not yet running: the thread that holds it runs the message loop.
pub struct Running {
    host: Host,
    runtime: Runtime,
    ctx: Arc<Ctx>,
    addr: SocketAddr,
    stop: watch::Sender<bool>,
}

impl Running {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn ctx(&self) -> Arc<Ctx> {
        self.ctx.clone()
    }

    pub fn engine(&self) -> Engine {
        self.host.engine()
    }

    /// Blocks on the message loop until a quit, then stops the server; returns the process's exit code.
    pub fn run(self) -> i32 {
        self.host.run();
        let _ = self.stop.send(true);
        // Each socket is told it is going away and given its moment to say so.
        let waited = Instant::now();
        while self.ctx.client_count() > 0 && waited.elapsed() < SHUTDOWN_GRACE {
            std::thread::sleep(Duration::from_millis(10));
        }
        self.runtime.shutdown_timeout(SHUTDOWN_GRACE);
        0
    }
}

fn millis_since_epoch() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// Builds the host on this thread (which becomes the message thread), queues the audio device's opening, binds
/// the port and starts accepting; `Err` is the reason the server could not come up, already said on stderr.
pub fn start(api: Api, cfg: ServeConfig) -> Result<Running, String> {
    let boot = Instant::now();
    let since = move || boot.elapsed().as_secs_f64() * 1000.0;
    // Before anything loads: a plugin the last engine died loading is struck now, and refused at the second strike.
    api.enable_load_guards();
    api.set_safe_mode(cfg.safe_mode_crashes > 0);
    if cfg.safe_mode_crashes > 0 {
        eprintln!(
            "[safe-mode] started after {} quick crashes: no external plugin will load",
            cfg.safe_mode_crashes
        );
    }
    api.use_default_path_roots();

    // Constructing the host opens NO audio device: that is queued below so the port binds first.
    let host = api
        .create_host()
        .ok_or_else(|| "k7-runtime: the engine could not be created".to_string())?;
    let ctor_ms = since();
    host.engine().post(move |mt| {
        let t0 = since();
        mt.open_audio_device();
        let t1 = since();
        eprintln!(
            "[startup] tracktion={ctor_ms}ms audioDevice={}ms (device ready at {t1}ms)",
            t1 - t0
        );
        if mt.output_device_name().is_empty() {
            eprintln!(
                "warning: no audio OUTPUT device is open — the transport will run but the playhead won't advance and nothing will sound. Pick an output with 'k7 config --set interface'."
            );
        }
    });

    let listener = std::net::TcpListener::bind((cfg.bind_host.as_str(), cfg.port))
        .and_then(|l| l.set_nonblocking(true).map(|()| l))
        .map_err(|e| {
            let why = format!("k7-runtime: listen failed: {e}");
            eprintln!("{why}");
            why
        })?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("k7-socket")
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let entered = runtime.enter();

    let identity = Identity {
        id: cfg.engine_id.clone(),
        token: cfg.engine_token,
        boot_id: millis_since_epoch().to_string(),
    };
    let file = cfg
        .pairing_file
        .unwrap_or_else(|| std::path::Path::new(&api.state_path()).join("paired-devices.json"));
    let gate = Arc::new(Gate::new(
        Pairing::open(file),
        GateConfig {
            engine_id: identity.id.clone(),
            engine_token: identity.token.clone(),
            boot_id: identity.boot_id.clone(),
            allowed_origins: cfg.allowed_origins,
        },
    ));
    let mut ctx = Ctx::new(host.engine(), identity, gate);
    ctx.audio_to_browser = cfg.audio_to_browser;
    ctx.safe_mode_crashes = cfg.safe_mode_crashes;
    let ctx = Arc::new(ctx);
    let mut table = handlers::table();
    if let Some(extra) = cfg.extra_handlers {
        extra(&mut table);
    }
    let table = Arc::new(table);
    let (stop, stopped) = watch::channel(false);

    let listener = tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;
    runtime.spawn(accept_loop(
        listener,
        ctx.clone(),
        table,
        cfg.timers,
        stopped,
    ));
    if cfg.handle_signals {
        quit_on_signals(&runtime, host.engine())?;
    }
    drop(entered);

    eprintln!(
        "k7-runtime listening on ws://{}:{}{}",
        cfg.bind_host,
        addr.port(),
        if cfg.engine_id.is_empty() {
            String::new()
        } else {
            format!(" (id {})", cfg.engine_id)
        }
    );
    eprintln!(
        "[startup] port bound at {}ms (audio device opens on the message loop; its timing is logged when it completes)",
        since()
    );
    Ok(Running {
        host,
        runtime,
        ctx,
        addr,
        stop,
    })
}

async fn accept_loop(
    listener: tokio::net::TcpListener,
    ctx: Arc<Ctx>,
    table: Arc<HandlerTable>,
    timers: Timers,
    mut stopped: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                tokio::spawn(conn::serve_socket(
                    stream,
                    ctx.clone(),
                    table.clone(),
                    timers,
                    stopped.clone(),
                ));
            }
            _ = stopped.changed() => break,
        }
    }
}

/// SIGINT and SIGTERM end the message loop, from a task: the handler itself does nothing a signal may not.
fn quit_on_signals(runtime: &Runtime, engine: Engine) -> Result<(), String> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut int = signal(SignalKind::interrupt()).map_err(|e| e.to_string())?;
        let mut term = signal(SignalKind::terminate()).map_err(|e| e.to_string())?;
        runtime.spawn(async move {
            tokio::select! {
                _ = int.recv() => {}
                _ = term.recv() => {}
            }
            engine.quit();
        });
    }
    #[cfg(not(unix))]
    runtime.spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        engine.quit();
    });
    Ok(())
}

/// The whole server on this thread: 0 after a quit, 1 when it could not come up.
pub fn serve(api: Api, cfg: ServeConfig) -> i32 {
    match start(api, cfg) {
        Ok(running) => running.run(),
        Err(_) => 1,
    }
}
