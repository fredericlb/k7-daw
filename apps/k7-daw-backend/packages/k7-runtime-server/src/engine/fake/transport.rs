//! transport.rs — the fake engine's transport area: which sessions play, which tracks are armed, and every call
//! written to the host's log as one line, so a test reads what the server asked of the engine and in what order.
//! What a reader gets wrong: a toggle is resolved here as the real engine resolves it, from the state the fake keeps.

use super::super::ffi::{K7Host, K7Str, K7StrEach};
use super::super::transport::K7TransportApi;
use super::{FakeState, fake, give};
use std::collections::HashSet;
use std::ffi::c_void;

/// What a test reads and scripts of the transport area.
#[derive(Default)]
pub struct TransportFake {
    pub playing: HashSet<String>,
    /// `(session, track)` armed.
    pub armed: HashSet<(String, String)>,
    /// `(session, node)` recording.
    pub recording: HashSet<(String, String)>,
    pub record_enabled: HashSet<String>,
    /// When set, `set_armed` refuses with it.
    pub arm_error: Option<String>,
    /// When set, `recorder` refuses with it.
    pub recorder_error: Option<String>,
}

/// `f` on the host's state under its lock; the call is logged first, as one line.
fn call<R>(host: *const K7Host, line: String, f: impl FnOnce(&mut FakeState) -> R) -> Option<R> {
    fake(host).state.lock().ok().map(|mut s| {
        s.calls.push(line);
        f(&mut s)
    })
}

fn text(s: K7Str) -> String {
    unsafe { s.to_string_lossy() }
}

unsafe extern "C" fn play(host: *mut K7Host, session: K7Str) {
    let s = text(session);
    call(host, format!("play {s}"), |st| {
        st.transport.playing.insert(s)
    });
}

unsafe extern "C" fn stop(host: *mut K7Host, session: K7Str) {
    let s = text(session);
    call(host, format!("stop {s}"), |st| {
        st.transport.playing.remove(&s)
    });
}

unsafe extern "C" fn panic(host: *mut K7Host, session: K7Str) {
    let s = text(session);
    call(host, format!("panic {s}"), |st| {
        st.transport.playing.remove(&s)
    });
}

unsafe extern "C" fn rewind(host: *mut K7Host, session: K7Str) {
    call(host, format!("rewind {}", text(session)), |_| ());
}

unsafe extern "C" fn seek(host: *mut K7Host, session: K7Str, seconds: f64) {
    call(host, format!("seek {} {seconds}", text(session)), |_| ());
}

unsafe extern "C" fn set_loop(
    host: *mut K7Host,
    session: K7Str,
    start: f64,
    end: f64,
    enabled: i32,
) {
    call(
        host,
        format!("set_loop {} {start} {end} {enabled}", text(session)),
        |_| (),
    );
}

unsafe extern "C" fn is_playing(host: *mut K7Host, session: K7Str) -> i32 {
    let s = text(session);
    call(host, format!("is_playing {s}"), |st| {
        i32::from(st.transport.playing.contains(&s))
    })
    .unwrap_or(0)
}

unsafe extern "C" fn preview_sample(
    host: *mut K7Host,
    session: K7Str,
    file: K7Str,
    start: f64,
    length: f64,
) {
    call(
        host,
        format!(
            "preview_sample {} {} {start} {length}",
            text(session),
            text(file)
        ),
        |_| (),
    );
}

unsafe extern "C" fn preview_note(
    host: *mut K7Host,
    session: K7Str,
    track: K7Str,
    key: i32,
    velocity: f32,
) {
    call(
        host,
        format!(
            "preview_note {} {} {key} {velocity}",
            text(session),
            text(track)
        ),
        |_| (),
    );
}

unsafe extern "C" fn live_midi(
    host: *mut K7Host,
    session: K7Str,
    track: K7Str,
    bytes: *const u8,
    len: usize,
) {
    let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
    call(
        host,
        format!("live_midi {} {} {bytes:?}", text(session), text(track)),
        |_| (),
    );
}

unsafe extern "C" fn set_armed(
    host: *mut K7Host,
    session: K7Str,
    track: K7Str,
    want: i32,
    armed: *mut i32,
    ctx: *mut c_void,
    error: K7StrEach,
) -> i32 {
    let key = (text(session), text(track));
    let outcome = call(
        host,
        format!("set_armed {} {} {want}", key.0, key.1),
        |st| {
            if let Some(why) = &st.transport.arm_error {
                return Err(why.clone());
            }
            let now = if want < 0 {
                !st.transport.armed.contains(&key)
            } else {
                want != 0
            };
            if now {
                st.transport.armed.insert(key);
            } else {
                st.transport.armed.remove(&key);
            }
            Ok(now)
        },
    )
    .unwrap_or_else(|| Err("no host".into()));
    match outcome {
        Ok(now) => {
            unsafe { *armed = i32::from(now) };
            1
        }
        Err(why) => {
            give(error, ctx, &why);
            0
        }
    }
}

unsafe extern "C" fn recorder(
    host: *mut K7Host,
    session: K7Str,
    node: K7Str,
    clear: i32,
    on: i32,
    ctx: *mut c_void,
    error: K7StrEach,
) -> i32 {
    let key = (text(session), text(node));
    let refused = call(
        host,
        format!("recorder {} {} {clear} {on}", key.0, key.1),
        |st| {
            if let Some(why) = &st.transport.recorder_error {
                return Some(why.clone());
            }
            if clear == 0 {
                let now = if on < 0 {
                    !st.transport.recording.contains(&key)
                } else {
                    on != 0
                };
                if now {
                    st.transport.recording.insert(key);
                } else {
                    st.transport.recording.remove(&key);
                }
            }
            None
        },
    )
    .flatten();
    match refused {
        Some(why) => {
            give(error, ctx, &why);
            0
        }
        None => 1,
    }
}

unsafe extern "C" fn set_record_enabled(host: *mut K7Host, session: K7Str, want: i32) {
    let s = text(session);
    call(host, format!("set_record_enabled {s} {want}"), |st| {
        let now = if want < 0 {
            !st.transport.record_enabled.contains(&s)
        } else {
            want != 0
        };
        if now {
            st.transport.record_enabled.insert(s);
        } else {
            st.transport.record_enabled.remove(&s);
        }
    });
}

pub static TABLE: K7TransportApi = K7TransportApi {
    play,
    stop,
    panic,
    rewind,
    seek,
    set_loop,
    is_playing,
    preview_sample,
    preview_note,
    live_midi,
    set_armed,
    recorder,
    set_record_enabled,
};
