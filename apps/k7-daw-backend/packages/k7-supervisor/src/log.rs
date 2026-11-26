//! log.rs — the merged log: one `HH:MM:SS [tag] text` line per event, APPENDED to `<state>/.studio.log`,
//! which the Logs panel reads by byte offset, so the format is load-bearing. This process is the file's only
//! writer, so it keeps it bounded: past `ROTATE_ABOVE` it becomes `.studio.log.1`. Under an app nothing is
//! echoed to stderr; run from a config (`standalone`) the session is ours, so the file is truncated and echoed.
//! One process-wide log, set up by `init`; a line before it goes nowhere (`docs/supervisorPolicy.md`).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Past this size the log is set aside as `.studio.log.1`, at its open and while it runs.
pub const ROTATE_ABOVE: u64 = 8 * 1024 * 1024;

/// One log file and its size so far.
pub struct Log {
    path: PathBuf,
    file: Option<File>,
    written: u64,
    limit: u64,
    standalone: bool,
}

/// Move `file` to `<file>.1`, replacing the one there, when it is larger than `limit`; true when it moved.
pub fn rotate(file: &Path, limit: u64) -> bool {
    match std::fs::metadata(file) {
        Ok(m) if m.len() > limit => {}
        _ => return false,
    }
    let mut aside = file.as_os_str().to_owned();
    aside.push(".1");
    // Removed first, so the move never rests on the platform replacing a file.
    let _ = std::fs::remove_file(&aside);
    std::fs::rename(file, &aside).is_ok()
}

fn stamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

impl Log {
    /// The log at `path`, opened at its first line; `standalone` truncates it then and echoes every line.
    pub fn new(path: PathBuf, standalone: bool, limit: u64) -> Log {
        Log {
            path,
            file: None,
            written: 0,
            limit,
            standalone,
        }
    }

    fn open(&mut self, truncate: bool) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if !truncate {
            rotate(&self.path, self.limit);
        }
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .append(!truncate)
            .truncate(truncate)
            .open(&self.path);
        if let Ok(f) = f {
            self.written = if truncate {
                0
            } else {
                f.metadata().map(|m| m.len()).unwrap_or(0)
            };
            self.file = Some(f);
        }
    }

    /// One tagged, timestamped line, ONE write; an unwritable file is silence.
    pub fn line(&mut self, tag: &str, text: &str) {
        let l = format!("{} [{tag}] {text}\n", stamp());
        if self.file.is_none() {
            self.open(self.standalone);
        } else if self.written + l.len() as u64 > self.limit {
            self.file = None;
            let moved = rotate(&self.path, 0);
            self.open(moved);
            // A file Windows would not let go of is kept, and tried again a whole size later.
            if !moved {
                self.written = 0;
            }
        }
        if let Some(f) = &mut self.file
            && f.write_all(l.as_bytes()).is_ok()
        {
            self.written += l.len() as u64;
        }
        if self.standalone {
            let _ = std::io::stderr().write_all(l.as_bytes());
        }
    }
}

static LOG: Mutex<Option<Log>> = Mutex::new(None);

/// The process's log from now on: at `path`, `standalone` under `--config`.
pub fn init(path: PathBuf, standalone: bool) {
    *LOG.lock().unwrap_or_else(|e| e.into_inner()) = Some(Log::new(path, standalone, ROTATE_ABOVE));
}

/// One line to the process's log, from any thread.
pub fn line(tag: &str, text: &str) {
    if let Some(log) = LOG.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        log.line(tag, text);
    }
}

/// A line unless the log is held, as it is when the panic being reported happened inside it.
pub fn try_line(tag: &str, text: &str) {
    if let Ok(mut guard) = LOG.try_lock()
        && let Some(log) = guard.as_mut()
    {
        log.line(tag, text);
    }
}

/// A line under the supervisor's own tag.
pub fn host(text: &str) {
    line("host", text);
}

/// To stderr alone, which an app files under its own tag.
pub fn stderr_only(text: &str) {
    let _guard = LOG.lock();
    let _ = writeln!(std::io::stderr(), "{text}");
}
