//! jni.rs — the two calls `RustFront.kt` declares, under the names JNI looks them up by. The config string
//! is read through the JNIEnv's own function table, so no JNI crate is needed; `apps/k7/backend-rs/docs/embed.md`.

use std::ffi::{c_char, c_void};
use std::ptr::null_mut;

/// A `JNIEnv*`: a pointer to the pointer to JNI's table of functions.
type Env = *mut *const *const c_void;
type GetStringUtfChars = unsafe extern "system" fn(Env, *mut c_void, *mut u8) -> *const c_char;
type ReleaseStringUtfChars = unsafe extern "system" fn(Env, *mut c_void, *const c_char);

// Their places in JNINativeInterface, fixed by the JNI specification since Java 1.1.
const GET_STRING_UTF_CHARS: usize = 169;
const RELEASE_STRING_UTF_CHARS: usize = 170;

/// `RustFront.nativeStart(config)`: `k7_front_start` with the string's bytes, released before returning.
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_se_frediver_k7_RustFront_nativeStart(
    env: Env,
    _class: *mut c_void,
    config: *mut c_void,
) -> i32 {
    if env.is_null() || config.is_null() {
        return -1;
    }
    unsafe {
        let table = *env;
        let get = std::mem::transmute::<*const c_void, GetStringUtfChars>(
            *table.add(GET_STRING_UTF_CHARS),
        );
        let release = std::mem::transmute::<*const c_void, ReleaseStringUtfChars>(
            *table.add(RELEASE_STRING_UTF_CHARS),
        );
        let chars = get(env, config, null_mut());
        if chars.is_null() {
            return -1;
        }
        let code = crate::k7_front_start(chars);
        release(env, config, chars);
        code
    }
}

/// `RustFront.nativeStop()`: `k7_front_stop`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_se_frediver_k7_RustFront_nativeStop(
    _env: Env,
    _class: *mut c_void,
) -> i32 {
    crate::k7_front_stop()
}
