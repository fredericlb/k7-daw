//! paths.rs — the state directory's layout: `~/.k7`, or `$STUDIO_HOME`. The twin of
//! `apps/k7-daw-frontend/packages/engine-client/src/paths.ts`: a name spelt otherwise does not fail, it reads
//! as an empty file. Held as a value rather than read from the environment at each call, so a test owns its home.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Paths {
    home: PathBuf,
    /// `$K7_SETTINGS_DIR`, where the engine keeps its plugin scan when set.
    engine_settings: Option<PathBuf>,
}

/// `p` with `.` dropped and `..` folded, without touching the disk.
fn lexically_normal(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

impl Paths {
    /// `$STUDIO_HOME`, else `~/.k7` (`USERPROFILE` on Windows, `HOME` elsewhere, as Node's `os.homedir()`).
    pub fn from_env() -> Paths {
        let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let dir = env_path("STUDIO_HOME")
            .unwrap_or_else(|| env_path(home_var).unwrap_or_default().join(".k7"));
        Paths::at(&dir, env_path("K7_SETTINGS_DIR"))
    }

    /// A state directory named outright, absolute and normalised.
    pub fn at(dir: &Path, engine_settings: Option<PathBuf>) -> Paths {
        let abs = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
        Paths {
            home: lexically_normal(&abs),
            engine_settings,
        }
    }

    pub fn studio_dir(&self) -> &Path {
        &self.home
    }
    /// The engine's `{ id, token, tunnelSecret, createdAt }`.
    pub fn engine_id(&self) -> PathBuf {
        self.home.join("engine-id.json")
    }
    /// The running engine's identity, what lets a fresh supervisor adopt an orphaned engine.
    pub fn engine_pid(&self) -> PathBuf {
        self.home.join("engine.pid")
    }
    /// The running supervisor's lock, one per machine.
    pub fn host_pid(&self) -> PathBuf {
        self.home.join("engine-host.pid")
    }
    /// The supervisor's own settings.
    pub fn settings(&self) -> PathBuf {
        self.home.join("engine-host.json")
    }
    /// The plugin scan cache the engine writes; deleted before a forced rescan.
    pub fn exts_cache(&self) -> PathBuf {
        self.engine_settings
            .as_deref()
            .unwrap_or(&self.home)
            .join("exts.json")
    }
    /// The engine log written when NOT under the supervisor.
    pub fn engine_log(&self) -> PathBuf {
        self.home.join(".studio-engine.log")
    }
    /// The merged log: the engine's and the supervisor's lines, tagged.
    pub fn merged_log(&self) -> PathBuf {
        self.home.join(".studio.log")
    }
    /// `<state>/workspace`, which the snapshot still names.
    pub fn root_dir(&self) -> PathBuf {
        self.home.join("workspace")
    }
    /// The frozen chains this machine keeps compiled (`docs/slots.md`).
    pub fn slots_dir(&self) -> PathBuf {
        self.home.join("slots")
    }
    /// One server's files as this machine holds them.
    pub fn remote_cache(&self, key: &str) -> PathBuf {
        self.home.join("remote").join(key)
    }
}

/// A path as UTF-8 for JSON and logs; a name that is not UTF-8 costs a replacement character.
pub fn utf8(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
