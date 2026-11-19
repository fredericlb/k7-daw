// backend.rs — what the tray starts: k7's Rust front, in this process, on 127.0.0.1 at the table's port. It
// answers the page and every route and runs the machine's supervisor and so its engine. The front is kept
// running by this thread: ready once its health answers, said Down when it stops answering. The front's one
// environment is standalone.json's and the host's. See docs/tray.md.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::state::{Backend, SharedState};
use crate::{http, logbus, paths, tray};

/// The thread keeping the front running, and the flags a stop and a restart pass between threads.
#[derive(Default)]
pub struct Process {
    stopping: AtomicBool,
    /// Bumped at every start, so a readiness watch knows when the front it watched is gone.
    generation: AtomicU64,
    /// The thread keeping the front running, joined by a restart before it starts the next.
    runner: Mutex<Option<JoinHandle<()>>>,
    restarting: AtomicBool,
}

/// What the front needs from standalone.json: its fixed port, its environment, and its folders under the state root.
#[derive(serde::Deserialize)]
struct Table {
    port: u16,
    env: std::collections::BTreeMap<String, String>,
    dirs: std::collections::BTreeMap<String, String>,
}

/// Health checks missed in a row before the front is said Down.
const MISSES: u32 = 3;
/// How long the port may stay taken: a previous tray's front may still be stopping.
const PORT_WAIT: Duration = Duration::from_secs(30);
/// How long a started front has to answer before the tray says it does not.
const READY_WAIT: Duration = Duration::from_secs(90);

fn set(app: &AppHandle, b: Backend) {
    app.state::<SharedState>().lock().unwrap().backend = b;
    tray::refresh(app);
}

/// The session secret, kept beside the backend's state so sessions outlive a restart; created once.
fn session_secret(root: &Path) -> String {
    let at = root.join("session-secret");
    if let Ok(s) = std::fs::read_to_string(&at) {
        if !s.trim().is_empty() {
            return s.trim().to_string();
        }
    }
    let s = random_hex(32);
    let _ = std::fs::write(&at, &s);
    s
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("no OS randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// The table's environment, the folders under the state root and the session secret.
fn shared_env(table: &Table, root: &Path) -> std::collections::BTreeMap<String, String> {
    let mut env = table.env.clone();
    for (k, rel) in &table.dirs {
        env.insert(k.clone(), root.join(rel).display().to_string());
    }
    env.insert("SESSION_SECRET".into(), session_secret(root));
    env
}

/// What the front is told beyond the table: the switches, what its supervisor and file hub read.
fn host_env(app: &AppHandle, table: &Table, token: &str) -> Result<Env, String> {
    let bundle =
        paths::backend_bundle(app).ok_or("k7's page and library are missing from this install")?;
    let supervisor =
        paths::supervisor(app).ok_or("the engine's supervisor is missing from this install")?;
    let root = paths::backend_root(app);
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let mut env = shared_env(table, &root);
    let mut set = |k: &str, v: String| env.insert(k.to_string(), v);
    set("K7_ENGINE_CORE", "front".into());
    set("K7_LOCAL_ENGINE", "front".into());
    // The compiled modules in the tray's own data folder (docs/tray.md).
    set(
        "K7_FRONT_CACHE",
        root.join("front-cache").display().to_string(),
    );
    // The file hub too: every account's file service is the front's own (docs/tray.md).
    set("K7_FILES", "front".into());
    set("STUDIO_PORTS", format!("k7={}", table.port));
    set("K7_SUPERVISOR_BIN", supervisor.display().to_string());
    set(
        "K7_SUPERVISOR_HOME",
        paths::studio_dir().display().to_string(),
    );
    set("K7_TRAY_TOKEN", token.to_string());
    set("K7_RELEASE", env!("K7_RELEASE").to_string());
    let seed = bundle.join("k7-core");
    if seed.is_dir() {
        set("STUDIO_LIBRARY_SEED", format!("k7-core={}", seed.display()));
    }
    if let Some(engine) = paths::engine_beside(&supervisor) {
        set("STUDIO_ENGINE", engine.display().to_string());
    }
    // What the front's actor reads at its root (apps/k7/backend-rs/docs/service.md).
    if let Some(code) = paths::code_root(app) {
        set("K7_CODE_ROOT", code.display().to_string());
    }
    // The page from the bundle's dist, or the dev tray's Vite (docs/tray.md).
    let dist = bundle.join("apps/k7/frontend/dist");
    if dist.join("index.html").is_file() {
        set("K7_FRONTEND_DIST", dist.display().to_string());
    }
    if let Ok(vite) = std::env::var("K7_DEV_FRONTEND").map(|v| v.trim().to_string()) {
        if !vite.is_empty() {
            set("K7_DEV_FRONTEND", vite);
        }
    }
    Ok(env)
}

type Env = std::collections::BTreeMap<String, String>;

/// The front on the table's port; stopped first if one runs.
fn start_front(app: &AppHandle, port: u16, token: &str) -> Result<(), String> {
    let table = read_table(app).ok_or("standalone.json is missing from this install")?;
    let env = host_env(app, &table, token)?;
    k7_front::embedded::stop();
    let config = k7_front::Config {
        listen: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        secret: random_hex(32),
        trust_hops: 0,
        env,
    };
    k7_front::embedded::start(config)
        .map(|_| ())
        .map_err(|code| format!("k7's front did not start on {port} (code {code})"))
}

/// The table, read once: its port is also where the page lives.
pub fn port(app: &AppHandle) -> u16 {
    if let Some(p) = paths::scratch_port() {
        return p;
    }
    read_table(app).map(|t| t.port).unwrap_or(18789)
}

fn read_table(app: &AppHandle) -> Option<Table> {
    let file = paths::standalone_json(app)?;
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

/// Start the backend and keep it running until the tray stops it.
pub fn spawn(app: &AppHandle) {
    let handle = app.clone();
    let runner = std::thread::spawn(move || run_forever(handle));
    *app.state::<Process>().runner.lock().unwrap() = Some(runner);
}

/// The menu's Restart: stop the backend as Quit does, then start it again — the way out of every Down.
pub fn restart(app: &AppHandle) {
    if app
        .state::<Process>()
        .restarting
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        logbus::line("host", "restarting the backend");
        set(&app, Backend::Starting);
        stop(&app);
        let proc = app.state::<Process>();
        let runner = proc.runner.lock().unwrap().take();
        if let Some(runner) = runner {
            let _ = runner.join();
        }
        proc.stopping.store(false, Ordering::SeqCst);
        spawn(&app);
        proc.restarting.store(false, Ordering::SeqCst);
    });
}

/// Sleep `d`, cut short by a stop.
fn nap(app: &AppHandle, d: Duration) {
    let until = Instant::now() + d;
    while Instant::now() < until && !app.state::<Process>().stopping.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Whether the port is free, waited for with a backoff: an answer that outlasts PORT_WAIT is another program's.
fn wait_for_port(app: &AppHandle, port: u16) -> bool {
    let until = Instant::now() + PORT_WAIT;
    let mut pause = Duration::from_millis(500);
    let mut said = false;
    while http::get(port, "/", None).is_some() {
        if Instant::now() >= until || app.state::<Process>().stopping.load(Ordering::SeqCst) {
            return false;
        }
        if !said {
            logbus::line(
                "host",
                &format!("127.0.0.1:{port} answers already — waiting for it to free up"),
            );
            said = true;
        }
        nap(app, pause);
        pause = (pause * 2).min(Duration::from_secs(4));
    }
    true
}

fn run_forever(app: AppHandle) {
    let token = app.state::<SharedState>().lock().unwrap().token.clone();
    let port = app.state::<SharedState>().lock().unwrap().port;
    // An answer already there is another k7 on our port, or a previous tray's front still stopping.
    if !wait_for_port(&app, port) {
        if app.state::<Process>().stopping.load(Ordering::SeqCst) {
            return;
        }
        logbus::line("host", &format!("127.0.0.1:{port} is already in use"));
        return set(
            &app,
            Backend::Down(format!("port {port} is in use by another program")),
        );
    }
    set(&app, Backend::Starting);
    if let Err(why) = start_front(&app, port, &token) {
        logbus::line("host", &why);
        return set(&app, Backend::Down(why));
    }
    logbus::line("host", &format!("k7's front on 127.0.0.1:{port}"));
    let generation = app
        .state::<Process>()
        .generation
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    watch_ready(&app, port, generation);
}

/// Ready once the front's own health answers, Down when it stops answering for MISSES checks in a row.
fn watch_ready(app: &AppHandle, port: u16, generation: u64) {
    let current = |app: &AppHandle| {
        let proc = app.state::<Process>();
        !proc.stopping.load(Ordering::SeqCst)
            && proc.generation.load(Ordering::SeqCst) == generation
    };
    let until = Instant::now() + READY_WAIT;
    let mut late = false;
    while current(app) {
        if let Some((200, _)) = http::get(port, "/api/health", None) {
            logbus::line("host", &format!("k7 answers on http://127.0.0.1:{port}"));
            set(app, Backend::Ready);
            break;
        }
        // Still silent past the wait: said, so the menu's Restart is the way out rather than a spinner.
        if !late && Instant::now() >= until {
            late = true;
            logbus::line("host", "the front has not answered in 90 s");
            set(
                app,
                Backend::Down("k7 does not answer — see the logs, or Restart".into()),
            );
        }
        std::thread::sleep(Duration::from_millis(if late { 2000 } else { 300 }));
    }
    let mut misses = 0;
    while current(app) {
        std::thread::sleep(Duration::from_secs(5));
        if !current(app) {
            return;
        }
        if matches!(http::get(port, "/api/health", None), Some((200, _))) {
            misses = 0;
            continue;
        }
        misses += 1;
        if misses == MISSES {
            logbus::line("host", "the front stopped answering");
            set(
                app,
                Backend::Down("k7 stopped answering — see the logs, or Restart".into()),
            );
        }
    }
}

/// Stop the front, which stops the supervisor.
pub fn stop(app: &AppHandle) {
    app.state::<Process>()
        .stopping
        .store(true, Ordering::SeqCst);
    // A request in flight then fails at once, rather than waiting on a front going away.
    k7_front::embedded::stop();
}

/// The address the menu opens: the front's root, or one of its pages.
pub fn url(app: &AppHandle, path: &str) -> String {
    let port = app.state::<SharedState>().lock().unwrap().port;
    format!("http://127.0.0.1:{port}{path}")
}
