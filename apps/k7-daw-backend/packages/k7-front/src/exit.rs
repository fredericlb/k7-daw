//! exit.rs — every way the front's process ends leaves a line: a panic in any thread (with a backtrace), an abort
//! from C or Rust, a crash Windows did not hand to anyone; a signal and a stop are `main.rs`'s lines.
//! What a reader gets wrong: a signal handler cannot allocate, so an abort's line is a fixed text (`docs/front.md`).

use std::sync::{Once, OnceLock};

/// Where the front's last words go: stderr for the binary, the tray's log, Android's logcat.
pub type Sink = fn(&str);

static SINK: OnceLock<Sink> = OnceLock::new();
static WATCH: Once = Once::new();

fn stderr(line: &str) {
    eprintln!("{line}");
}

/// A line for the host's log, through the sink `watch` was given (stderr before it was).
pub fn say(line: &str) {
    SINK.get().copied().unwrap_or(stderr)(line);
}

/// The hooks installed once per process; the first caller's sink wins, as a process has one log.
pub fn watch(sink: Option<Sink>) {
    WATCH.call_once(|| {
        let _ = SINK.set(sink.unwrap_or(stderr));
        let elsewhere = sink.is_some();
        let before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("unnamed");
            let trace = std::backtrace::Backtrace::force_capture();
            say(&format!(
                "k7-front: panic in thread '{name}': {info}\n{trace}"
            ));
            // The default hook's own line too when the sink is not stderr: a host reading stderr still sees it.
            if elsewhere {
                before(info);
            }
        }));
        native::install();
    });
}

#[cfg(unix)]
mod native {
    use std::ffi::{c_int, c_void};

    const SIGABRT: c_int = 6;
    const LINE: &[u8] = b"k7-front: aborted (SIGABRT), by Wasmtime or a Rust abort\n";

    unsafe extern "C" {
        fn signal(sig: c_int, handler: usize) -> usize;
        fn write(fd: c_int, buf: *const c_void, n: usize) -> isize;
    }

    extern "C" fn on_abort(_: c_int) {
        // Only write(2) here: the handler runs inside abort(), where nothing may allocate; the default kills next.
        unsafe { write(2, LINE.as_ptr().cast(), LINE.len()) };
        unsafe { signal(SIGABRT, 0) };
    }

    /// SIGABRT alone: Wasmtime owns SIGSEGV and SIGBUS for its traps, and a host's launcher logs the signal.
    pub fn install() {
        unsafe { signal(SIGABRT, on_abort as *const () as usize) };
    }
}

#[cfg(windows)]
mod native {
    use std::ffi::{c_int, c_void};

    const SIGABRT: c_int = 22;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
    const ABORT: &[u8] = b"k7-front: aborted (abort()), by the C runtime\r\n";

    #[repr(C)]
    struct ExceptionRecord {
        code: u32,
        flags: u32,
        next: *mut ExceptionRecord,
        address: *mut c_void,
    }

    #[repr(C)]
    struct Pointers {
        record: *mut ExceptionRecord,
        context: *mut c_void,
    }

    type Filter = unsafe extern "system" fn(*const Pointers) -> i32;

    unsafe extern "system" {
        fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn WriteFile(
            h: *mut c_void,
            buf: *const u8,
            n: u32,
            written: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }

    unsafe extern "C" {
        fn signal(sig: c_int, handler: usize) -> usize;
    }

    fn write(bytes: &[u8]) {
        let mut n = 0u32;
        unsafe {
            WriteFile(
                GetStdHandle(STD_ERROR_HANDLE),
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut n,
                std::ptr::null_mut(),
            )
        };
    }

    /// A fixed-size line in hex, written without allocating: the process is about to end.
    fn hex(out: &mut [u8], at: usize, v: u64, digits: usize) {
        for i in 0..digits {
            let d = ((v >> (4 * (digits - 1 - i))) & 0xf) as u8;
            out[at + i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        }
    }

    unsafe extern "system" fn on_crash(p: *const Pointers) -> i32 {
        let mut line = *b"k7-front: crashed, exception 0x00000000 at 0x0000000000000000\r\n";
        if let Some(r) = unsafe { p.as_ref().and_then(|p| p.record.as_ref()) } {
            hex(&mut line, 31, u64::from(r.code), 8);
            hex(&mut line, 45, r.address as u64, 16);
        }
        write(&line);
        EXCEPTION_CONTINUE_SEARCH
    }

    extern "C" fn on_abort(_: c_int) {
        write(ABORT);
    }

    /// The last-chance filter (Wasmtime's traps are first-chance handlers, never reaching it) and the CRT's abort.
    pub fn install() {
        unsafe { SetUnhandledExceptionFilter(Some(on_crash)) };
        unsafe { signal(SIGABRT, on_abort as *const () as usize) };
    }
}

#[cfg(not(any(unix, windows)))]
mod native {
    pub fn install() {}
}
