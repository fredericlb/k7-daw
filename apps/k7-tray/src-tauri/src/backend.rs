// backend.rs — the one process the tray starts: node running k7's backend bundle on 127.0.0.1, behind k7's Rust
// front, which holds the page's port in this process and runs the machine's supervisor and so its engine. Node is
// spawned into the kill-on-close job, kept running with a backoff, stopped by closing its stdin (the stop Windows
// has no signal for). Both are given one environment, standalone.json's and the host's. See docs/tray.md.

use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::state::{Backend, SharedState};
use crate::{http, logbus, paths, tray};

/// The child and its stdin, kept so a stop can close the pipe and then make sure.
#[derive(Default)]
pub struct Process {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    stopping: AtomicBool,
    /// Bumped at every spawn, so a readiness watch knows when the child it watched is gone.
    generation: AtomicU64,
    /// The thread keeping the backend running, joined by a restart before it starts the next.
    runner: Mutex<Option<JoinHandle<()>>>,
    restarting: AtomicBool,
}

/// What a backend needs from standalone.json: its fixed port, its environment, and its folders under the state root.
#[derive(serde::Deserialize)]
struct Table {
    port: u16,
    env: std::collections::BTreeMap<String, String>,
    dirs: std::collections::BTreeMap<String, String>,
}

/// How long a stop waits for the backend to stop its supervisor and itself before it is killed.
const STOP_GRACE: Duration = Duration::from_secs(15);
/// A run this long was a healthy one: the next crash starts the backoff over.
const HEALTHY: Duration = Duration::from_secs(60);
/// How long a launch waits for the port to free up: a previous tray's backend may still be in its STOP_GRACE.
const PORT_WAIT: Duration = Duration::from_secs(30);
/// How long a started backend has to answer before the tray says it does not.
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

/// A loopback port nothing holds now, for node behind the front; losing it to another program is a failed start.
fn free_port() -> Result<u16, String> {
    let socket = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("no free loopback port: {e}"))?;
    socket
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}

/// What node is told beside the table, and the front too: the folders under the state root and the session secret.
fn shared_env(table: &Table, root: &Path) -> std::collections::BTreeMap<String, String> {
    let mut env = table.env.clone();
    for (k, rel) in &table.dirs {
        env.insert(k.clone(), root.join(rel).display().to_string());
    }
    env.insert("SESSION_SECRET".into(), session_secret(root));
    env
}

/// What node and the front are both told beyond the table: the switches, what the front's supervisor and file hub read.
fn host_env(app: &AppHandle, table: &Table, token: &str) -> Result<Env, String> {
    let bundle = paths::backend_bundle(app).ok_or("k7's backend is missing from this install")?;
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
    // node's bytecode of the bundle, kept between launches: only the first after an update parses it (docs/tray.md).
    if let Some(cache) = paths::node_compile_cache(app) {
        set("NODE_COMPILE_CACHE", cache.display().to_string());
    }
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
    // The backend's last routes in the front: the page from the bundle's dist, or the dev tray's Vite (docs/tray.md).
    set("K7_LAST_ROUTES", "front".into());
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

/// The front on the table's port, in front of node on `upstream`; stopped first if one runs.
fn start_front(
    app: &AppHandle,
    port: u16,
    upstream: u16,
    secret: &str,
    token: &str,
) -> Result<(), String> {
    let table = read_table(app).ok_or("standalone.json is missing from this install")?;
    let env = host_env(app, &table, token)?;
    k7_front::embedded::stop();
    let config = k7_front::Config {
        listen: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        upstream: format!("127.0.0.1:{upstream}"),
        secret: secret.to_string(),
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

/// The command, or why there is none — every miss is said on the status line, never a silent empty tray.
fn command(app: &AppHandle, token: &str, upstream: u16, secret: &str) -> Result<Command, String> {
    let bundle = paths::backend_bundle(app).ok_or("k7's backend is missing from this install")?;
    let table = read_table(app).ok_or("standalone.json is missing from this install")?;
    let root = paths::backend_root(app);
    let env = host_env(app, &table, token)?;

    let mut c = Command::new(paths::node(app));
    c.arg(bundle.join("bin").join("backend.mjs"))
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("NODE_OPTIONS")
        .env_remove("TRUSTED_ORIGINS")
        .envs(env);
    c.env("PORT", upstream.to_string())
        .env("K7_FRONT_PORT", table.port.to_string())
        .env("K7_FRONT_SECRET", secret)
        .env("K7_STDIN_STOP", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: node gets no console
    }
    Ok(c)
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
    // An answer already there is another k7 on our port, or a previous tray's backend still stopping.
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
    // One front and one node port for this run; node's restarts below reuse both.
    let secret = random_hex(32);
    let upstream =
        match free_port().and_then(|up| start_front(&app, port, up, &secret, &token).map(|_| up)) {
            Ok(up) => up,
            Err(why) => {
                logbus::line("host", &why);
                return set(&app, Backend::Down(why));
            }
        };
    logbus::line(
        "host",
        &format!("k7's front on 127.0.0.1:{port}, node behind it on {upstream}"),
    );
    let mut delay = Duration::from_secs(2);
    let mut quick = 0;
    loop {
        let proc = app.state::<Process>();
        if proc.stopping.load(Ordering::SeqCst) {
            return;
        }
        set(&app, Backend::Starting);
        let started = Instant::now();
        let exit = match command(&app, &token, upstream, &secret)
            .and_then(|mut c| c.spawn().map_err(|e| e.to_string()))
        {
            Ok(mut child) => {
                if let Some(out) = child.stdout.take() {
                    logbus::pump(out, "backend");
                }
                if let Some(err) = child.stderr.take() {
                    logbus::pump(err, "backend");
                }
                *proc.stdin.lock().unwrap() = child.stdin.take();
                *proc.child.lock().unwrap() = Some(child);
                // A stop that came while it spawned found no pipe to close: close it now, or the wait never ends.
                if proc.stopping.load(Ordering::SeqCst) {
                    drop(proc.stdin.lock().unwrap().take());
                }
                logbus::line("host", "backend started");
                let generation = proc.generation.fetch_add(1, Ordering::SeqCst) + 1;
                watch_ready(&app, port, upstream, generation);
                wait(&proc)
            }
            Err(why) => {
                logbus::line("host", &format!("backend not started: {why}"));
                return set(&app, Backend::Down(why));
            }
        };
        if proc.stopping.load(Ordering::SeqCst) {
            return;
        }
        logbus::line("host", &format!("backend exited ({exit})"));
        if started.elapsed() >= HEALTHY {
            delay = Duration::from_secs(2);
            quick = 0;
        } else {
            quick += 1;
        }
        // Six quick deaths in a row is a backend that cannot start: say so and stop trying, the log says why.
        if quick >= 6 {
            return set(
                &app,
                Backend::Down(format!("k7 keeps stopping ({exit}) — see the logs")),
            );
        }
        set(
            &app,
            Backend::Down(format!("k7 stopped ({exit}), restarting…")),
        );
        nap(&app, delay);
        delay = (delay * 2).min(Duration::from_secs(60));
    }
}

/// Block until the child exits; the description of how.
fn wait(proc: &Process) -> String {
    loop {
        if let Some(child) = proc.child.lock().unwrap().as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => return status.to_string(),
                Ok(None) => {}
                Err(e) => return e.to_string(),
            }
        } else {
            return "gone".into();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Ready once node's own health answers on `upstream`, the phone's probe too: node listens once its library is in place.
fn watch_ready(app: &AppHandle, port: u16, upstream: u16, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        let until = Instant::now() + READY_WAIT;
        let current = |app: &AppHandle| {
            let proc = app.state::<Process>();
            !proc.stopping.load(Ordering::SeqCst)
                && proc.generation.load(Ordering::SeqCst) == generation
        };
        let mut late = false;
        while current(&app) {
            if let Some((200, _)) = http::get(upstream, "/api/health", None) {
                logbus::line("host", &format!("k7 answers on http://127.0.0.1:{port}"));
                return set(&app, Backend::Ready);
            }
            // Still silent past the wait: said, so the menu's Restart is the way out rather than a spinner.
            if !late && Instant::now() >= until {
                late = true;
                logbus::line("host", "the backend has not answered in 90 s");
                set(
                    &app,
                    Backend::Down("k7 does not answer — see the logs, or Restart".into()),
                );
            }
            std::thread::sleep(Duration::from_millis(if late { 2000 } else { 300 }));
        }
    });
}

/// Stop the backend: the front first, which stops the supervisor, then node's stdin closed; killed past the grace.
pub fn stop(app: &AppHandle) {
    let proc = app.state::<Process>();
    proc.stopping.store(true, Ordering::SeqCst);
    // A request in flight then fails at once, rather than waiting on a backend going away.
    k7_front::embedded::stop();
    if let Some(mut stdin) = proc.stdin.lock().unwrap().take() {
        let _ = stdin.flush();
    } // dropped here: the pipe closes
    let until = Instant::now() + STOP_GRACE;
    loop {
        let mut guard = proc.child.lock().unwrap();
        let Some(child) = guard.as_mut() else { return };
        if matches!(child.try_wait(), Ok(Some(_))) {
            *guard = None;
            return;
        }
        if Instant::now() >= until {
            logbus::line("host", "backend did not stop in time — killed");
            let _ = child.kill();
            *guard = None;
            return;
        }
        drop(guard);
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The address the menu opens: the backend's root, or one of its pages.
pub fn url(app: &AppHandle, path: &str) -> String {
    let port = app.state::<SharedState>().lock().unwrap().port;
    format!("http://127.0.0.1:{port}{path}")
}
