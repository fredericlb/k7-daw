// paths.rs — where the app finds what it starts (k7's bundle of page and library, the supervisor) and where state lives.
// Packaged, everything is in the bundle's resources (scripts/stage.mjs); in a checkout, the staged copy under
// src-tauri/resources, or the repo's own builds. `~/.k7` mirrors studioHome.ts BY HAND. See docs/app.md.

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tauri::{AppHandle, Manager};

const fn exe(name: &'static str, windows: &'static str) -> &'static str {
    if cfg!(windows) {
        windows
    } else {
        name
    }
}
const SUPERVISOR: &str = exe("k7-supervisor", "k7-supervisor.exe");
const ENGINE: &str = exe("k7-runtime", "k7-runtime.exe");

/// `<state>` — the machine's engine home, the command-line engine's too: `$STUDIO_HOME`, else `~/.k7`.
pub fn studio_dir() -> PathBuf {
    match std::env::var("STUDIO_HOME") {
        Ok(h) if !h.is_empty() => PathBuf::from(h),
        _ => dirs::home_dir().expect("no home directory").join(".k7"),
    }
}

/// The running engine's pid: the backstop's way to reach an orphan.
pub fn engine_pid_path() -> PathBuf {
    studio_dir().join("engine.pid")
}

/// The merged log: the supervisor's and the engine's lines, the backend's under `[backend]`, the app's under `[host]`.
pub fn merged_log_path() -> PathBuf {
    studio_dir().join(".studio.log")
}

/// The backend's own state (workspaces, prefs): the OS's per-app folder, never `~/.k7`.
pub fn backend_root(app: &AppHandle) -> PathBuf {
    if let Some(scratch) = scratch() {
        return scratch.join("data");
    }
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| studio_dir().join("local-backend"))
}

/// A scratch run's own folder (`K7_DESKTOP_SCRATCH`, debug builds only): its data, and no single-instance lock.
pub fn scratch() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var("K7_DESKTOP_SCRATCH")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// A scratch run's port (`K7_DESKTOP_PORT`), never the table's 18789 that the person's app holds.
pub fn scratch_port() -> Option<u16> {
    scratch()?;
    std::env::var("K7_DESKTOP_PORT").ok()?.parse().ok()
}

/// The staged resources: the bundle's when packaged, `src-tauri/resources` in a checkout.
fn resources(app: &AppHandle) -> PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        if dir.join("backend").is_dir() {
            return dir;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")
}

/// What the front reads beside itself (apps/k7-daw-backend/scripts/stageBundle.mjs): `$K7_BACKEND_BUNDLE`, else the staged one.
pub fn backend_bundle(app: &AppHandle) -> Option<PathBuf> {
    let dir = std::env::var("K7_BACKEND_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| resources(app).join("backend"));
    dir.join("k7-core").is_dir().then_some(dir)
}

/// The Rust front's code root (scripts/stage.mjs's `code`): the staged one; in a checkout the front finds the checkout.
pub fn code_root(app: &AppHandle) -> Option<PathBuf> {
    let dir = resources(app).join("code");
    dir.join("apps/k7-daw-frontend/packages/commands/commands.yaml")
        .is_file()
        .then_some(dir)
}

/// The backend's table of ports, environment and folders, shared with the phone (apps/k7-daw-backend/standalone.json).
pub fn standalone_json(app: &AppHandle) -> Option<PathBuf> {
    let staged = resources(app).join("standalone.json");
    if staged.is_file() {
        return Some(staged);
    }
    let repo = repo_root()?.join("apps/k7-daw-backend/standalone.json");
    repo.is_file().then_some(repo)
}

/// The supervisor: `$STUDIO_ENGINE_HOST_CMD`, the staged copy, or the repo's newest build; None is a loud failure.
pub fn supervisor(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(cmd) = std::env::var("STUDIO_ENGINE_HOST_CMD") {
        let p = PathBuf::from(cmd);
        if p.is_file() {
            return Some(p);
        }
    }
    let staged = resources(app).join("bin").join(SUPERVISOR);
    if staged.is_file() {
        return Some(staged);
    }
    built_supervisor()
}

/// The checkout's own supervisor build, where `apps/k7-daw-engine/scripts/supervisorBuild.mjs` puts it.
fn built_supervisor() -> Option<PathBuf> {
    let repo = repo_root()?;
    let target = match std::env::var_os("K7_SUPERVISOR_TARGET_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let checkout = repo.file_name()?.to_string_lossy().into_owned();
            dirs::home_dir()?
                .join(".cache")
                .join(format!("k7-supervisor-{checkout}"))
        }
    };
    let p = target.join("release").join(SUPERVISOR);
    p.is_file().then_some(p)
}

/// The engine binary beside the supervisor, for the backend's offline renders (`$STUDIO_ENGINE`).
pub fn engine_beside(supervisor: &Path) -> Option<PathBuf> {
    let beside = supervisor.parent()?.join(ENGINE);
    if beside.is_file() {
        return Some(beside);
    }
    newest_engine_build()
}

/// The monorepo root of a checkout run: `$STUDIO_REPO_ROOT`, else up from this crate to the dir holding `apps/k7-daw-engine/k7-runtime`.
fn repo_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("STUDIO_REPO_ROOT") {
        let p = PathBuf::from(root);
        if p.is_dir() {
            return Some(p);
        }
    }
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("apps/k7-daw-engine/k7-runtime").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The freshest engine build under `apps/k7-daw-engine/k7-runtime/build/<preset>/[<config>/]`, Debug last; never a phone's.
fn newest_engine_build() -> Option<PathBuf> {
    let build = repo_root()?.join("apps/k7-daw-engine/k7-runtime/build");
    let mut found: Vec<(bool, SystemTime, PathBuf)> = Vec::new();
    for preset in std::fs::read_dir(&build).ok()?.flatten() {
        let preset_name = preset.file_name().to_string_lossy().into_owned();
        if preset_name.starts_with("android-") || preset_name.starts_with("ios-") {
            continue;
        }
        for cfg in ["", "Release", "RelWithDebInfo", "Debug"] {
            let p = preset.path().join(cfg).join(ENGINE);
            if let Ok(meta) = std::fs::metadata(&p) {
                if meta.is_file() {
                    let at = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    found.push((cfg == "Debug", at, p));
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    found.into_iter().next().map(|(_, _, p)| p)
}
