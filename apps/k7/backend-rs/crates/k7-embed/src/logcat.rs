//! logcat.rs — Android's log under the tag `k7-front`, where the app's stdout and stderr are not kept: the front's
//! panics and its stop, and every line the front prints, beside BackendHost's own (`apps/k7/backend-rs/docs/embed.md`).
//! What a reader gets wrong: stderr is the process's, so once the engine serves here its pipe takes it (`k7-runtime`).

use std::ffi::{CString, c_char, c_int};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::FromRawFd;
use std::sync::Once;

const INFO: c_int = 4;
const WARN: c_int = 5;
const ERROR: c_int = 6;
/// Logcat cuts an entry near 4 KB: a longer line is written in pieces of this many bytes at most.
const PIECE: usize = 4000;

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

unsafe extern "C" {
    fn pipe(fds: *mut c_int) -> c_int;
    fn dup2(from: c_int, to: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}

fn write(prio: c_int, text: &str) {
    let mut rest = text;
    while !rest.is_empty() {
        let mut end = rest.len().min(PIECE);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let piece = CString::new(rest[..end].replace('\0', "")).unwrap_or_default();
        unsafe { __android_log_write(prio, c"k7-front".as_ptr(), piece.as_ptr()) };
        rest = &rest[end..];
    }
}

/// One line, its NUL bytes dropped so the whole of it is written.
pub fn line(text: &str) {
    write(ERROR, text);
}

/// Whether `fd` is a pipe already: the engine's, which started first and reads it into logcat itself.
fn piped(fd: c_int) -> bool {
    use std::os::unix::fs::FileTypeExt;
    let held = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
    held.metadata().is_ok_and(|m| m.file_type().is_fifo())
}

/// `fd` (1 or 2) made the write end of a pipe whose lines a thread of its own writes to logcat at `prio`.
fn pump(fd: c_int, prio: c_int, name: &str) {
    if piped(fd) {
        return;
    }
    let mut fds: [c_int; 2] = [-1, -1];
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        return;
    }
    if unsafe { dup2(fds[1], fd) } < 0 {
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        return;
    }
    unsafe { close(fds[1]) };
    let read = unsafe { File::from_raw_fd(fds[0]) };
    let spawned = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let mut lines = BufReader::new(read);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match lines.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => write(
                        prio,
                        String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']),
                    ),
                }
            }
        });
    // Nobody would read the pipe without its thread, and a full pipe blocks every print: the fd goes back to nowhere.
    if spawned.is_err() {
        let null = File::options().write(true).open("/dev/null");
        if let Ok(null) = null {
            use std::os::fd::AsRawFd;
            unsafe { dup2(null.as_raw_fd(), fd) };
        }
    }
}

/// The process's stdout at INFO and stderr at WARN, into logcat as lines, once per process.
pub fn capture() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        pump(1, INFO, "k7-front-stdout");
        pump(2, WARN, "k7-front-stderr");
    });
}
