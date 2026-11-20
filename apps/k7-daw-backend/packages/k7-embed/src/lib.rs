//! k7-embed — the front as a C library for the phones: `k7_front_start(config JSON)`, `k7_front_stop()` and
//! `k7_roam_install(list, put)` over `k7_front::embedded`, plus the JNI names Android looks up (`apps/k7-daw-backend/docs/embed.md`).
//! A Rust host (the tray) links `k7-front` itself and never builds this crate's C outputs.

#[cfg(target_os = "android")]
mod jni;
#[cfg(target_os = "android")]
mod logcat;

use std::ffi::{CStr, c_char};

/// Starts the front from a JSON `Config`, a NUL-terminated UTF-8 string; returns its port, or a negative code.
// The safety contract is the one line above; a multi-line `# Safety` section breaks the repo's comment rule.
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn k7_front_start(config: *const c_char) -> i32 {
    // On Android stdout and stderr go nowhere: the front's lines go to logcat, its last words written there directly.
    #[cfg(target_os = "android")]
    {
        logcat::capture();
        k7_front::exit::watch(Some(logcat::line));
    }
    if config.is_null() {
        return -1;
    }
    let Ok(text) = (unsafe { CStr::from_ptr(config) }).to_str() else {
        return -1;
    };
    let Ok(config) = serde_json::from_str::<k7_front::Config>(text) else {
        return -2;
    };
    match k7_front::embedded::start(config) {
        Ok(port) => i32::from(port),
        Err(code) => code,
    }
}

/// Stops the running front and waits for it; returns 0, or -1 when none runs.
#[unsafe(no_mangle)]
pub extern "C" fn k7_front_stop() -> i32 {
    if k7_front::embedded::stop() { 0 } else { -1 }
}

/// Installs the host's list store as two callbacks, before `k7_front_start` (`apps/k7-daw-backend/docs/roam.md`).
#[unsafe(no_mangle)]
pub extern "C" fn k7_roam_install(list: k7_roam::ffi::List, put: k7_roam::ffi::Put) {
    k7_roam::ffi::install_callbacks(list, put);
}
