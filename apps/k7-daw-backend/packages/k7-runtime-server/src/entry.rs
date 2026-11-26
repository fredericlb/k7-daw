//! entry.rs — the C door of the Rust server: `k7_server_serve` and `k7_server_request_quit`, what the engine's
//! executable, Android's JNI and iOS's embed call once the C++ server is gone (`engine/capi/k7_server.h`).
//! What a reader gets wrong: `serve` runs on the thread that becomes the host's message thread and blocks there,
//! so the listener starts from a job posted to that loop, never before it, and a quit can never precede the loop.

use crate::engine::{Api, Engine, ffi::K7EngineApi};
use crate::stub;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

unsafe extern "C" {
    // Defined by the C++ engine (`engine/capi/EngineApi.cpp`): the symbol this crate's link depends on.
    fn k7_engine_api() -> *const K7EngineApi;
}

static RUNNING: Mutex<Option<Engine>> = Mutex::new(None);

fn running() -> MutexGuard<'static, Option<Engine>> {
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
    if let Some(engine) = running().as_ref() {
        engine.quit();
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
    let Some(host) = api.create_host() else {
        return 1;
    };
    let engine = host.engine();
    *running() = Some(engine.clone());
    let code = Arc::new(AtomicI32::new(0));
    let listener = Arc::new(Mutex::new(None));
    let (job_code, job_listener, job_engine) = (code.clone(), listener.clone(), engine.clone());
    engine.post(move |_mt| match stub::listen(port, job_engine.clone()) {
        Ok(l) => *job_listener.lock().unwrap_or_else(|p| p.into_inner()) = Some(l),
        Err(e) => {
            eprintln!("[server] cannot listen on {port}: {e}");
            job_code.store(1, Ordering::SeqCst);
            job_engine.quit();
        }
    });
    host.run();
    if let Some(l) = listener.lock().unwrap_or_else(|p| p.into_inner()).take() {
        l.stop();
    }
    *running() = None;
    drop(host);
    code.load(Ordering::SeqCst)
}
