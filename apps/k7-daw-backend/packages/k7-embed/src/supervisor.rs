//! supervisor.rs — the supervisor as a library for the iOS app, which may run no program of its own: the three C
//! calls its Swift half makes (`SupervisorHost.swift`, `StdioLog.swift`, `Busy.swift`), over `k7_supervisor::run::embedded`.
//! What a reader gets wrong: the app closes both descriptors after a run, so the run reads and writes duplicates.

use std::ffi::{CStr, c_char};
use std::fs::File;
use std::os::fd::FromRawFd;

/// A duplicate of the app's descriptor, owned by the run; None when it cannot be had.
fn owned(fd: i32) -> Option<File> {
    // SAFETY: dup has no precondition; a non-negative answer is a descriptor of our own.
    let copy = unsafe { libc::dup(fd) };
    // SAFETY: `copy` was just made and nothing else owns it.
    (copy >= 0).then(|| unsafe { File::from_raw_fd(copy) })
}

/// Runs until the app closes the write end of `commands_fd` (its quit) or the supervisor exits; blocks the calling thread.
#[unsafe(no_mangle)]
pub extern "C" fn k7_supervisor_run(commands_fd: i32, frames_fd: i32) -> i32 {
    match (owned(commands_fd), owned(frames_fd)) {
        (Some(commands), Some(frames)) => k7_supervisor::run::embedded(commands, frames),
        _ => 1,
    }
}

/// Stem cuts queued or running in this process: the app stays alive behind another while it is above zero.
#[unsafe(no_mangle)]
pub extern "C" fn k7_supervisor_cuts_in_flight() -> i32 {
    k7_supervisor::separation::cuts_in_flight()
}

/// Appends one `[tag] text` line to the machine log, as a child's line is appended; any thread, any time.
// The safety contract is the one line above: two NUL-terminated strings or null, which is ignored.
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn k7_supervisor_log(tag: *const c_char, text: *const c_char) {
    if tag.is_null() || text.is_null() {
        return;
    }
    // SAFETY: the caller hands two NUL-terminated strings.
    let (tag, text) = unsafe { (CStr::from_ptr(tag), CStr::from_ptr(text)) };
    k7_supervisor::log::line(&tag.to_string_lossy(), &text.to_string_lossy());
}
