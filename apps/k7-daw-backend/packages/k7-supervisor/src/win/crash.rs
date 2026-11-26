//! crash.rs — what a crash leaves on Windows beside its `fatal:` line: a minidump, `crash-supervisor-<pid>.dmp` in the
//! state folder, written once by whichever comes first: a panic, a C library's abort(), or an unhandled exception.
//! What a reader gets wrong: Rust's own abort is a fail-fast that no handler sees; the SIGABRT one catches the C
//! runtime's, which ONNX Runtime and DirectML would reach (`docs/windows.md`).

use std::path::{Path, PathBuf};

/// The dump's file in the state folder `dir`, named as the C++ supervisor named it.
pub fn dump_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("crash-supervisor-{pid}.dmp"))
}

#[cfg(windows)]
pub use imp::{dump_once, install};

#[cfg(windows)]
mod imp {
    use std::os::windows::io::AsRawHandle;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Diagnostics::Debug::{
        EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS, MINIDUMP_EXCEPTION_INFORMATION,
        MINIDUMP_TYPE, MiniDumpWithIndirectlyReferencedMemory, MiniDumpWithThreadInfo,
        MiniDumpWriteDump, SetUnhandledExceptionFilter,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId,
    };

    /// The state folder and the merged log's writer, set once by `install`.
    type Setup = (PathBuf, fn(&str));
    static SETUP: OnceLock<Setup> = OnceLock::new();
    static REPORTED: AtomicBool = AtomicBool::new(false);

    /// The handlers, first thing in `main`: the unhandled-exception filter and the C runtime's SIGABRT.
    pub fn install(dir: PathBuf, say: fn(&str)) {
        let _ = SETUP.set((dir, say));
        let on_abort: extern "C" fn(libc::c_int) = on_abort;
        // SAFETY: both handlers are plain functions that live as long as the process.
        unsafe {
            SetUnhandledExceptionFilter(Some(on_unhandled));
            libc::signal(libc::SIGABRT, on_abort as libc::sighandler_t);
        }
    }

    /// A dump with no exception record, once: a panic's, whose `fatal:` line the panic hook has written.
    pub fn dump_once() {
        if !REPORTED.swap(true, Ordering::SeqCst) {
            write(std::ptr::null());
        }
    }

    extern "C" fn on_abort(_: libc::c_int) {
        if !REPORTED.swap(true, Ordering::SeqCst) {
            if let Some((_, say)) = SETUP.get() {
                say("fatal: abort()");
            }
            write(std::ptr::null());
        }
    }

    unsafe extern "system" fn on_unhandled(info: *const EXCEPTION_POINTERS) -> i32 {
        if !REPORTED.swap(true, Ordering::SeqCst) {
            // SAFETY: Windows hands the filter a valid record for the exception being raised.
            let record = unsafe { info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) };
            let code = record.map_or(0, |r| r.ExceptionCode.0 as u32);
            if let Some((_, say)) = SETUP.get() {
                say(&format!("fatal: unhandled exception 0x{code:08X}"));
            }
            write(info);
        }
        EXCEPTION_CONTINUE_SEARCH
    }

    /// The minidump beside the merged log; nothing when the file cannot be made.
    fn write(info: *const EXCEPTION_POINTERS) {
        let Some((dir, say)) = SETUP.get() else {
            return;
        };
        let path = super::dump_path(dir, std::process::id());
        let Ok(file) = std::fs::File::create(&path) else {
            return;
        };
        // SAFETY: no precondition; the thread that crashed, which the dump names as the faulting one.
        let thread = unsafe { GetCurrentThreadId() };
        let kind =
            MINIDUMP_TYPE(MiniDumpWithThreadInfo.0 | MiniDumpWithIndirectlyReferencedMemory.0);
        // Raw pointers are not Send: the writing thread gets their addresses.
        let (handle, record) = (file.as_raw_handle() as usize, info as usize);
        // From another thread: the faulting thread's own stack is not captured reliably otherwise.
        let ok = std::thread::scope(|s| {
            s.spawn(move || {
                let mei = MINIDUMP_EXCEPTION_INFORMATION {
                    ThreadId: thread,
                    ExceptionPointers: record as *mut EXCEPTION_POINTERS,
                    ClientPointers: false.into(),
                };
                let param = (record != 0).then_some(&mei as *const MINIDUMP_EXCEPTION_INFORMATION);
                // SAFETY: the handle is the file opened above and the record the filter's, both alive until the join.
                let done = unsafe {
                    MiniDumpWriteDump(
                        GetCurrentProcess(),
                        GetCurrentProcessId(),
                        HANDLE(handle as *mut _),
                        kind,
                        param,
                        None,
                        None,
                    )
                };
                done.is_ok()
            })
            .join()
            .unwrap_or(false)
        });
        drop(file);
        if ok {
            say(&format!("crash dump: {}", path.display()));
        }
    }
}
