//! entry.rs — the C doors of the Rust server (`engine/capi/k7_server.h`): `k7_server_main` for the executable's
//! argv, `k7_server_serve` for Android's JNI and iOS's embed, and the quit, suspend and log-tag calls beside them.
//! What a reader gets wrong: `serve` runs on the thread that becomes the host's message thread and blocks there,
//! and the engine is held for the quit from the moment the server is up, so a quit is never lost.

use crate::cli;
use crate::engine::{Api, Engine, ffi::K7EngineApi};
use crate::log::say;
use crate::server::{self, ServeConfig};
use std::ffi::{CStr, c_char};
use std::panic::catch_unwind;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};

unsafe extern "C" {
    // Defined by the C++ engine (`engine/capi/EngineApi.cpp`): the symbol this crate's link depends on.
    fn k7_engine_api() -> *const K7EngineApi;
}

static RUNNING: Mutex<Option<Engine>> = Mutex::new(None);

fn held() -> MutexGuard<'static, Option<Engine>> {
    RUNNING.lock().unwrap_or_else(|p| p.into_inner())
}

/// The executable's whole run: argv read, then a one-shot mode or the server. Safety: `argv` is C's `main`'s.
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn k7_server_main(argc: i32, argv: *const *const c_char) -> i32 {
    let args: Vec<String> = (1..usize::try_from(argc).unwrap_or(0))
        .map(|i| {
            unsafe { CStr::from_ptr(*argv.add(i)) }
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    catch_unwind(|| main(&args)).unwrap_or(1)
}

/// Serves on `port` until `k7_server_request_quit`; the exit code. Blocks on the host's message loop.
#[unsafe(no_mangle)]
pub extern "C" fn k7_server_serve(port: i32) -> i32 {
    catch_unwind(|| match api() {
        Ok(api) => serve(api, port).0,
        Err(code) => code,
    })
    .unwrap_or(1)
}

/// Asks a running `k7_server_serve` to return, from any thread; a no-op when none runs.
#[unsafe(no_mangle)]
pub extern "C" fn k7_server_request_quit() {
    if let Some(engine) = held().as_ref() {
        // A job, not a direct quit: the loop may not have started, and a posted job waits for it.
        let again = engine.clone();
        engine.post(move |_| again.quit());
    }
}

/// Closes the audio device (non-zero) or reopens it, on the message thread; a no-op when none runs.
#[unsafe(no_mangle)]
pub extern "C" fn k7_server_suspend_audio(suspended: i32) {
    if let Some(engine) = held().as_ref() {
        engine.post(move |mt| mt.suspend_audio(suspended != 0));
    }
}

/// From here on the server's own stderr lines start with `[engine] `, as the iOS app's machine log reads them.
#[unsafe(no_mangle)]
pub extern "C" fn k7_server_tag_log() {
    crate::log::tag();
}

fn api() -> Result<Api, i32> {
    Api::new(unsafe { &*k7_engine_api() }).map_err(|e| {
        say!("[server] the engine's table is refused: {e:?}");
        3
    })
}

fn main(args: &[String]) -> i32 {
    let api = match api() {
        Ok(api) => api,
        Err(code) => return code,
    };
    api.install_crash_handler();
    let (dir, rest) = cli::take_settings_dir(args);
    api.init_settings_dir(&dir);
    let named = api.settings_dir();
    if !named.is_empty() {
        say!("[startup] settings-dir={named}");
    }
    if !layout_consistent(api) {
        return 4;
    }
    cli::run(api, &rest, |port| {
        let (code, abandoned) = serve(api, port);
        // Detached work still holds what the process frees at exit: leave without unwinding it.
        if abandoned {
            say!("[shutdown] background work still running — exiting without cleanup");
            exit_now(code);
        }
        code
    })
}

/// A half-rebuilt engine LINKS and dies later on a garbage `doc.tracks`, so each reader's layout is compared.
fn layout_consistent(api: Api) -> bool {
    if api.layout_consistent() {
        return true;
    }
    say!(
        "[fatal] this engine is HALF REBUILT: ProjectDoc has a different layout in different objects.\n        It would crash on the first project, in keyOf, naming nothing.\n        Delete the objects for engine/*.cpp and rebuild — apps/k7-daw-engine/k7-runtime/AGENTS.md."
    );
    false
}

#[cfg(unix)]
fn exit_now(code: i32) -> ! {
    unsafe { libc::_exit(code) }
}

#[cfg(not(unix))]
fn exit_now(code: i32) -> ! {
    std::process::exit(code)
}

/// The exit code, and whether detached work was still running when the server let go of it.
fn serve(api: Api, port: i32) -> (i32, bool) {
    let Ok(port) = u16::try_from(port) else {
        say!("[server] port {port} is not a port");
        return (2, false);
    };
    api.install_terminate_handler();
    if !layout_consistent(api) {
        return (4, false);
    }
    let running = match server::start(api, ServeConfig::from_env(port)) {
        Ok(running) => running,
        Err(_) => return (1, false),
    };
    let ctx = running.ctx();
    *held() = Some(running.engine());
    let code = running.run();
    *held() = None;
    (code, ctx.plugins.detached.load(Ordering::SeqCst) > 0)
}
