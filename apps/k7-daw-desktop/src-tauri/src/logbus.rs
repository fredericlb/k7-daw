// logbus.rs — the app's end of the merged log. The supervisor writes it; the app truncates
// it ONCE per session (it is the only process that knows a session began) and adds the
// `[host]` lines, which are what remain when the supervisor itself broke. See docs/app.md.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::paths::{merged_log_path, studio_dir};

/// The append handle every write goes through, behind a Mutex so our own lines can't interleave.
static LOG: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();

/// Truncate for a fresh session and open our append handle. Call once, BEFORE spawning the supervisor, which
/// opens the same file for append and would write into a file we are about to empty.
pub fn init() {
    let _ = std::fs::create_dir_all(studio_dir());
    let _ = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(merged_log_path());
    LOG.get_or_init(|| {
        Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(merged_log_path())
                .ok(),
        )
    });
}

/// Write one tagged, timestamped line, byte-identical to the supervisor's: the web Logs panel polls this file
/// by byte offset, so the format is contract, not decoration. Best-effort — a log must never take the app down.
pub fn line(tag: &str, text: &str) {
    let Some(m) = LOG.get() else { return }; // init() hasn't run — nothing to write to
    if let Ok(mut guard) = m.lock() {
        if let Some(f) = guard.as_mut() {
            let ts = chrono::Local::now().format("%H:%M:%S");
            let _ = writeln!(f, "{ts} [{tag}] {text}");
        }
    }
}
