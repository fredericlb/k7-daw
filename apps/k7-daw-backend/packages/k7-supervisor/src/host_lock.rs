//! host_lock.rs — one supervisor per machine, held in `<state>/engine-host.pid`. Two would each ADOPT the one
//! engine, and shutdown does not spare an adopted engine, so quitting the second would stop the first's. The
//! holder rewrites the file every 2 s and one nobody refreshed for 15 s is stale, whatever its pid claims:
//! a pid is not an identity (`apps/k7-daw-engine/k7-supervisor/docs/supervisorPolicy.md`, "HostLock").

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How often `refresh` must run for the lock to stay believed.
pub const REFRESH: Duration = Duration::from_secs(2);
const STALE: Duration = Duration::from_secs(15);

/// The lock file and the pid that takes it, this process's outside tests.
#[derive(Debug, Clone)]
pub struct HostLock {
    pub path: PathBuf,
    pub pid: u32,
}

/// Who holds the lock, as `held_by` reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Holder {
    Nobody,
    /// A fresh file naming nobody yet: a supervisor between its create and its write.
    Unknown,
    Pid(u32),
}

fn read_pid(p: &Path) -> u32 {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|t| t.split_whitespace().next().and_then(|w| w.parse().ok()))
        .unwrap_or(0)
}

impl HostLock {
    pub fn new(path: PathBuf) -> HostLock {
        HostLock {
            path,
            pid: std::process::id(),
        }
    }

    /// Who holds it; a stale lock, or one naming a dead pid, is removed on the way.
    pub fn held_by(&self) -> Holder {
        let Ok(meta) = std::fs::metadata(&self.path) else {
            return Holder::Nobody;
        };
        let pid = read_pid(&self.path);
        let age = meta
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok());
        let fresh = age.is_some_and(|a| a < STALE);
        if pid > 0 && pid != self.pid && k7_local::pidfile::alive(pid) && fresh {
            return Holder::Pid(pid);
        }
        if pid == 0 && fresh {
            return Holder::Unknown;
        }
        let _ = std::fs::remove_file(&self.path);
        Holder::Nobody
    }

    /// Created AND written through one handle, so the file names its holder from the moment it exists.
    fn create_exclusive(&self) -> bool {
        let Ok(mut f) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        else {
            return false;
        };
        let _ = writeln!(f, "{}", self.pid);
        true
    }

    /// Taken by exclusive create on every attempt, or the holder that has it.
    pub fn acquire(&self) -> Holder {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        for _ in 0..3 {
            match self.held_by() {
                Holder::Nobody if self.create_exclusive() => return Holder::Nobody,
                Holder::Nobody => {}
                held => return held,
            }
        }
        match read_pid(&self.path) {
            0 => Holder::Unknown,
            pid => Holder::Pid(pid),
        }
    }

    /// Our pid written again, which keeps the lock fresh; a lock naming somebody else is said, not overwritten.
    pub fn refresh(&self) {
        let held = read_pid(&self.path);
        if held != 0 && held != self.pid {
            crate::log::stderr_only(&format!(
                "[host] the supervisor lock was taken by pid {held} — two supervisors are running"
            ));
            return;
        }
        let _ = std::fs::write(&self.path, format!("{}\n", self.pid));
    }

    /// Let go, but only if the file still names us.
    pub fn release(&self) {
        if read_pid(&self.path) == self.pid {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
