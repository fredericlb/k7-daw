//! entry.rs — the C door of the Rust server: `k7_server_serve` and `k7_server_request_quit`, what the engine's
//! executable, Android's JNI and iOS's embed call once the C++ server is gone (`engine/capi/k7_server.h`).
//! What a reader gets wrong: `serve` runs on the thread that becomes the host's message thread and blocks there,
//! the engine is held for the quit from the moment the server is up, so a quit is never lost.

use crate::engine::{Api, Engine, ffi::K7EngineApi};
use crate::server::{self, ServeConfig};
use std::panic::catch_unwind;
use std::sync::{Mutex, MutexGuard};

unsafe extern "C" {
    // Defined by the C++ engine (`engine/capi/EngineApi.cpp`): the symbol this crate's link depends on.
    fn k7_engine_api() -> *const K7EngineApi;
}

static RUNNING: Mutex<Option<Engine>> = Mutex::new(None);

fn held() -> MutexGuard<'static, Option<Engine>> {
    RUNNING.lock().unwrap_or_else(|p| p.into_inner())
}

/// Serves on `port` until `k7_server_request_quit`; the exit code. Blocks on the host's message loop.
#[unsafe(no_mangle)]
pub extern "C" fn k7_server_serve(port: i32) -> i32 {
    catch_unwind(|| serve(port)).unwrap_or(1)
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

fn serve(port: i32) -> i32 {
    let Ok(port) = u16::try_from(port) else {
        eprintln!("[server] port {port} is not a port");
        return 2;
    };
    let api = match Api::new(unsafe { &*k7_engine_api() }) {
        Ok(api) => api,
        Err(e) => {
            eprintln!("[server] the engine's table is refused: {e:?}");
            return 3;
        }
    };
    if !api.layout_consistent() {
        eprintln!(
            "[fatal] this engine is HALF REBUILT: ProjectDoc has a different layout in different objects"
        );
        return 4;
    }
    let running = match server::start(api, ServeConfig::from_env(port)) {
        Ok(running) => running,
        Err(_) => return 1,
    };
    *held() = Some(running.engine());
    let code = running.run();
    *held() = None;
    code
}
