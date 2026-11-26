//! engine.rs — find, start, adopt and stop `k7-runtime`. One engine per machine, `<state>/engine.pid` the
//! guard that lets a supervisor crash be survived: the engine outlives us and the next supervisor ADOPTS it,
//! asking the PORT as well since a pid is not an identity. On a phone the engine is a library in the app,
//! so a start and a stop are REQUESTS to the app (the `engine` frame). Blocking: called off the core's task.
//! The rules: `apps/k7-daw-backend/packages/k7-supervisor/docs/supervisorPolicy.md`, "EngineProcess".

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde_json::json;

use crate::frames::Frames;
use crate::identity::Identity;
use crate::log;
use crate::paths::Paths;
use crate::pidfile::{self, Check, Record};
use crate::ports::{self, Probe};
use crate::process;

/// On a phone the engine is a thread of the app, never a program we spawn.
pub const IN_APP: bool = cfg!(any(target_os = "android", target_os = "ios"));
/// How long the app gets to bind the engine's port: JUCE and the audio device come up first.
const APP_START_BOUND: Duration = Duration::from_secs(20);
const NAMES: [&str; 2] = ["k7-runtime.exe", "k7-runtime"];

#[derive(Debug, Clone, PartialEq)]
pub enum StartKind {
    Spawned,
    Adopted,
    Missing,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Started {
    pub kind: StartKind,
    /// The port the engine serves on, spawned or adopted; 0 when none came up.
    pub port: u16,
}

/// What a start is asked for.
#[derive(Debug, Clone)]
pub struct StartAsk {
    pub first_port: u16,
    pub port_fixed: bool,
    /// The engine's `STUDIO_ENGINE_ALLOWED_ORIGINS`.
    pub origins: String,
    /// Safe mode when above zero (`docs/crashLoop.md`).
    pub safe_mode_crashes: u32,
    /// On every interface rather than loopback alone.
    pub lan: bool,
}

/// The origins of the studios this machine is listed at, comma-separated and deduplicated.
pub fn allowed_origins(registry_urls: &[String]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for r in registry_urls {
        let Ok(u) = url::Url::parse(r) else { continue };
        if !matches!(u.scheme(), "http" | "https") {
            continue;
        }
        let o = u.origin().ascii_serialization();
        if !o.contains(',') && !seen.contains(&o) {
            seen.push(o);
        }
    }
    seen.join(",")
}

/// Loopback unless the musician opened the LAN: the engine has no auth beyond its pairing.
fn bind_host(lan: bool) -> &'static str {
    if lan { "0.0.0.0" } else { "127.0.0.1" }
}

/// The checkout an executable was built in, found by walking up to `apps/k7-daw-engine/k7-runtime`.
fn repo_root(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .skip(1)
        .find(|d| d.join("apps/k7-daw-engine/k7-runtime").is_dir())
        .map(Path::to_path_buf)
}

/// `$STUDIO_ENGINE`, the copy beside `exe` (a packaged app), then the repo's newest build, Debug last.
pub fn find_binary(override_: Option<&Path>, exe: Option<&Path>) -> Option<PathBuf> {
    if let Some(o) = override_.filter(|o| o.exists()) {
        return Some(o.to_path_buf());
    }
    let exe = exe?;
    let beside = exe
        .parent()
        .into_iter()
        .flat_map(|d| NAMES.map(|n| d.join(n)));
    if let Some(found) = beside.into_iter().find(|p| p.is_file()) {
        return Some(found);
    }
    let build = repo_root(exe)?.join("apps/k7-daw-engine/k7-runtime/build");
    let mut found: Vec<(PathBuf, bool, SystemTime)> = Vec::new();
    for preset in std::fs::read_dir(build).ok()?.flatten() {
        for cfg in ["", "Release", "RelWithDebInfo", "Debug"] {
            for name in NAMES {
                let p = preset.path().join(cfg).join(name);
                if let Some(m) = std::fs::metadata(&p).ok().filter(|m| m.is_file()) {
                    found.push((
                        p,
                        cfg == "Debug",
                        m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    ));
                }
            }
        }
    }
    // Newest first WITHIN a tier, Debug a tier of its own below every other: at -O0 the DSP cannot keep up.
    found.sort_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)));
    found.into_iter().next().map(|f| f.0)
}

/// The engine THIS supervisor spawned, and whether its reaper has seen it exit.
#[derive(Debug, Default)]
struct Owned {
    pid: u32,
    alive: bool,
}

/// The machine's engine as this supervisor knows it.
#[derive(Clone)]
pub struct Engine {
    paths: Paths,
    identity: Identity,
    frames: Frames,
    /// `$STUDIO_ENGINE`, read once.
    binary: Option<PathBuf>,
    owned: Arc<Mutex<Owned>>,
}

impl Engine {
    pub fn new(
        paths: Paths,
        identity: Identity,
        frames: Frames,
        binary: Option<PathBuf>,
    ) -> Engine {
        Engine {
            paths,
            identity,
            frames,
            binary,
            owned: Arc::default(),
        }
    }

    fn pid_path(&self) -> PathBuf {
        self.paths.engine_pid()
    }

    /// Our own child's liveness by its reaper, when `pid` is it.
    fn owned_child(&self, pid: u32) -> Option<bool> {
        let o = self.owned.lock().unwrap_or_else(|e| e.into_inner());
        (pid > 0 && pid == o.pid).then_some(o.alive)
    }

    /// Is an engine up: the pidfile names a live process that may be it.
    pub fn running(&self) -> bool {
        let rec = pidfile::read(&self.pid_path());
        if IN_APP {
            return rec.pid > 0 && pidfile::alive(rec.pid);
        }
        if let Some(alive) = self.owned_child(rec.pid) {
            return alive;
        }
        rec.pid > 0 && pidfile::judge(&rec) != Check::Stale
    }

    /// Bring an engine up on the first free candidate port, or adopt ours.
    pub fn start(&self, ask: &StartAsk) -> Started {
        let _ = std::fs::create_dir_all(self.paths.studio_dir());
        if IN_APP {
            self.start_in_app(ask)
        } else {
            self.start_process(ask)
        }
    }

    /// The app may serve its engine ONCE per process, so a held port is skipped before asking.
    fn start_in_app(&self, ask: &StartAsk) -> Started {
        let app = process::app_pid();
        let rec = pidfile::read(&self.pid_path());
        // Adopted only when THIS app started it, on the port it recorded.
        if rec.pid == app && rec.port > 0 && ports::is_serving(rec.port) {
            log::host(&format!(
                "adopting the engine the app already serves on :{}",
                rec.port
            ));
            return Started {
                kind: StartKind::Adopted,
                port: rec.port,
            };
        }
        let mut chosen = None;
        for p in ports::candidates(ask.first_port, ask.port_fixed) {
            if ports::bindable(p) {
                chosen = Some(p);
                break;
            }
            log::host(&format!(
                ":{p} is held by another program — trying the next port"
            ));
        }
        let Some(port) = chosen else {
            return Started {
                kind: StartKind::Error(format!("no free port from :{}", ask.first_port)),
                port: 0,
            };
        };
        self.frames.emit(&json!({
            "t": "engine", "action": "start", "id": self.identity.id, "token": self.identity.token, "port": port,
            "host": bind_host(ask.lan), "origins": ask.origins, "safeMode": ask.safe_mode_crashes,
        }));
        let until = Instant::now() + APP_START_BOUND;
        while !ports::is_serving(port) {
            if Instant::now() > until {
                return Started {
                    kind: StartKind::Error(format!("the app did not start the engine on :{port}")),
                    port: 0,
                };
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        pidfile::write(
            &self.pid_path(),
            &Record {
                pid: app,
                port,
                ..Record::default()
            },
        );
        log::host(&format!("the app started the engine on :{port}"));
        Started {
            kind: StartKind::Spawned,
            port,
        }
    }

    fn start_process(&self, ask: &StartAsk) -> Started {
        let first = ask.first_port;
        if self.running() {
            let rec = pidfile::read(&self.pid_path());
            let port = if rec.port > 0 { rec.port } else { first };
            match ports::probe(port) {
                Probe::Up if ask.port_fixed && port != first => {
                    log::host(&format!(
                        "the engine on the pidfile serves :{port}, not :{first}, the port --port fixes — replacing it"
                    ));
                    self.stop();
                }
                Probe::Up => {
                    // A bare-pid file whose process the OS now shows to be the engine is upgraded, so a later stop may reach it.
                    if self.owned_child(rec.pid).is_none() && !rec.verified() {
                        let live = pidfile::record_of(rec.pid);
                        if pidfile::check(&live.local(), Some(&live.local()), true) == Check::Same {
                            pidfile::write(&self.pid_path(), &Record { port, ..live });
                        }
                    }
                    log::host(&format!(
                        "adopting the engine already on the pidfile, on :{port}"
                    ));
                    return Started {
                        kind: StartKind::Adopted,
                        port,
                    };
                }
                Probe::Unknown => {
                    let why = format!(
                        "could not open a socket to ask :{port} — keeping the engine on the pidfile"
                    );
                    return Started {
                        kind: StartKind::Error(why),
                        port: 0,
                    };
                }
                Probe::Down => {
                    log::host(&format!(
                        "pidfile names a live process but nothing answers :{port} — stale, starting a fresh engine"
                    ));
                    let _ = std::fs::remove_file(self.pid_path());
                }
            }
        }
        let candidates = ports::candidates(first, ask.port_fixed);
        for &port in &candidates {
            if let Some(adopted) = self.adopt_unnamed(port) {
                return adopted;
            }
            if ports::bindable(port) {
                return self.spawn_on(port, ask);
            }
            // Something else on the port is no engine of ours: never recorded, so never killed.
            let holder = holder_of(port);
            if ask.port_fixed {
                let why = format!(
                    "{holder} already holds :{port}, the port --port fixes — stop it, or choose another"
                );
                return Started {
                    kind: StartKind::Error(why),
                    port: 0,
                };
            }
            log::host(&format!("{holder} holds :{port} — trying the next port"));
        }
        let why = match (candidates.first(), candidates.last()) {
            (Some(a), Some(b)) => format!(
                "every port from :{a} to :{b} is taken — free one, or start from another \"port\""
            ),
            _ => "no port to start the engine on".to_string(),
        };
        Started {
            kind: StartKind::Error(why),
            port: 0,
        }
    }

    /// An engine answering with no pidfile naming it: a fresh one could only fail to bind, so that one is taken.
    fn adopt_unnamed(&self, port: u16) -> Option<Started> {
        if ports::probe(port) != Probe::Up {
            return None;
        }
        let pid = ports::listener_pid(port)?;
        let rec = pidfile::record_of(pid);
        // An executable the OS will not name is taken for the engine's.
        if rec
            .exe
            .as_deref()
            .is_some_and(|e| !pidfile::is_engine_exe(e))
        {
            return None;
        }
        pidfile::write(&self.pid_path(), &Record { port, ..rec });
        log::host(&format!(
            "adopting the engine already serving :{port} (pid {pid}), which no pidfile named"
        ));
        Some(Started {
            kind: StartKind::Adopted,
            port,
        })
    }

    fn spawn_on(&self, port: u16, ask: &StartAsk) -> Started {
        let exe = std::env::current_exe().ok();
        let Some(bin) = find_binary(self.binary.as_deref(), exe.as_deref()) else {
            return Started {
                kind: StartKind::Missing,
                port: 0,
            };
        };
        let owned = self.owned.clone();
        let env = [
            ("STUDIO_ENGINE_HOST", bind_host(ask.lan).to_string()),
            ("STUDIO_ENGINE_ID", self.identity.id.clone()),
            ("STUDIO_ENGINE_TOKEN", self.identity.token.clone()),
            ("STUDIO_ENGINE_ALLOWED_ORIGINS", ask.origins.clone()),
            // Always set, so a value in our own environment never puts a normal start in safe mode.
            ("STUDIO_ENGINE_SAFE_MODE", ask.safe_mode_crashes.to_string()),
        ];
        // Held across the spawn, so an engine that dies at once is not marked alive after its reaper ran.
        let mut guard = self.owned.lock().unwrap_or_else(|e| e.into_inner());
        let spawned = process::spawn(process::SpawnOptions {
            program: bin.clone(),
            args: vec![port.to_string()],
            env: env.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            on_line: Arc::new(|line, _| log::line("engine", line)),
            on_exit: Box::new(move |pid| {
                let mut o = owned.lock().unwrap_or_else(|e| e.into_inner());
                if o.pid == pid {
                    o.alive = false;
                }
            }),
        });
        let pid = match spawned {
            Ok(pid) => pid,
            Err(e) => {
                return Started {
                    kind: StartKind::Error(e),
                    port: 0,
                };
            }
        };
        *guard = Owned { pid, alive: true };
        drop(guard);
        let mut rec = pidfile::record_of(pid);
        rec.exe.get_or_insert_with(|| crate::paths::utf8(&bin));
        rec.port = port;
        pidfile::write(&self.pid_path(), &rec);
        let safe = if ask.safe_mode_crashes > 0 {
            " in safe mode"
        } else {
            ""
        };
        log::host(&format!("engine started (pid {pid}) on :{port}{safe}"));
        Started {
            kind: StartKind::Spawned,
            port,
        }
    }

    /// Stop the engine the pidfile names, its whole process tree, and clear the pidfile; never what is unproven.
    pub fn stop(&self) {
        let path = self.pid_path();
        if IN_APP {
            // Never a kill: the pid is the app's own, and the engine is a thread of it.
            self.frames.emit(&json!({"t": "engine", "action": "stop"}));
            let _ = std::fs::remove_file(path);
            return;
        }
        let rec = pidfile::read(&path);
        match self.owned_child(rec.pid) {
            Some(true) => pidfile::kill_tree(rec.pid),
            Some(false) => {}
            None if rec.pid > 0 => match pidfile::judge(&rec) {
                Check::Same => pidfile::kill_tree(rec.pid),
                Check::Unverified => log::host(&format!(
                    "not killing pid {}: the pidfile cannot prove it is the engine",
                    rec.pid
                )),
                Check::Stale => {}
            },
            None => {}
        }
        let _ = std::fs::remove_file(path);
    }
}

/// Who holds `port`, for the log: the program and its pid when the OS says, else only that something does.
fn holder_of(port: u16) -> String {
    let Some(pid) = ports::listener_pid(port) else {
        return "something".into();
    };
    match pidfile::record_of(pid).exe {
        Some(exe) => {
            let name = Path::new(&exe)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(exe);
            format!("{name} (pid {pid})")
        }
        None => format!("pid {pid}"),
    }
}
