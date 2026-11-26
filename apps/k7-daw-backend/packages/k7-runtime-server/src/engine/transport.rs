//! transport.rs — the transport area of the C ABI (`k7_engine_transport.h`) made safe: every call is a method of
//! `Mt`, so one cannot be made off the message thread. A `want` of `None` toggles, resolved by the engine.
//! What a reader gets wrong: an engine without this area does nothing, as it does without the project area.

use super::ffi::{K7Host, K7Str, K7StrEach, table};
use super::{Api, Mt};
use std::ffi::c_void;

table! {
    /// `k7_engine_transport.h`.
    K7TransportApi {
        play: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        stop: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        panic: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        rewind: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        seek: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, seconds: f64),
        set_loop: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, start_seconds: f64, end_seconds: f64, enabled: i32),
        is_playing: unsafe extern "C" fn(host: *mut K7Host, session: K7Str) -> i32,
        preview_sample: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, file: K7Str, start: f64, length: f64),
        preview_note: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, track: K7Str, key: i32, velocity: f32),
        live_midi: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, track: K7Str, bytes: *const u8, len: usize),
        set_armed: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, track: K7Str, want: i32, armed: *mut i32, ctx: *mut c_void, error: K7StrEach) -> i32,
        recorder: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, node: K7Str, clear: i32, on: i32, ctx: *mut c_void, error: K7StrEach) -> i32,
        set_record_enabled: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, want: i32),
    }
}

impl Api {
    /// The transport area, `None` while the engine has not built it.
    pub fn transport(&self) -> Option<&'static K7TransportApi> {
        let p = self.table().transport.cast::<K7TransportApi>();
        unsafe { p.as_ref() }
    }
}

/// -1 toggles, 0 and 1 are a state asked for.
fn want(on: Option<bool>) -> i32 {
    on.map_or(-1, i32::from)
}

unsafe extern "C" fn set_reason(ctx: *mut c_void, item: K7Str) {
    let out = unsafe { &mut *ctx.cast::<String>() };
    *out = unsafe { item.to_string_lossy() };
}

impl Mt<'_> {
    fn transport(&self) -> Option<&'static K7TransportApi> {
        self.api().transport()
    }

    pub fn play(&self, session: &str) {
        if let Some(t) = self.transport() {
            unsafe { (t.play)(self.host(), K7Str::of(session)) }
        }
    }

    pub fn stop(&self, session: &str) {
        if let Some(t) = self.transport() {
            unsafe { (t.stop)(self.host(), K7Str::of(session)) }
        }
    }

    /// Stop, then tell every plugin to let go and reset.
    pub fn panic(&self, session: &str) {
        if let Some(t) = self.transport() {
            unsafe { (t.panic)(self.host(), K7Str::of(session)) }
        }
    }

    pub fn rewind(&self, session: &str) {
        if let Some(t) = self.transport() {
            unsafe { (t.rewind)(self.host(), K7Str::of(session)) }
        }
    }

    pub fn seek(&self, session: &str, seconds: f64) {
        if let Some(t) = self.transport() {
            unsafe { (t.seek)(self.host(), K7Str::of(session), seconds) }
        }
    }

    pub fn set_loop(&self, session: &str, start: f64, end: f64, enabled: bool) {
        if let Some(t) = self.transport() {
            unsafe {
                (t.set_loop)(
                    self.host(),
                    K7Str::of(session),
                    start,
                    end,
                    i32::from(enabled),
                )
            }
        }
    }

    pub fn is_playing(&self, session: &str) -> bool {
        self.transport()
            .is_some_and(|t| unsafe { (t.is_playing)(self.host(), K7Str::of(session)) != 0 })
    }

    pub fn preview_sample(&self, session: &str, file: &str, start: f64, length: f64) {
        if let Some(t) = self.transport() {
            unsafe {
                (t.preview_sample)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(file),
                    start,
                    length,
                )
            }
        }
    }

    pub fn preview_note(&self, session: &str, track: &str, key: i32, velocity: f32) {
        if let Some(t) = self.transport() {
            unsafe {
                (t.preview_note)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(track),
                    key,
                    velocity,
                )
            }
        }
    }

    pub fn live_midi(&self, session: &str, track: &str, bytes: &[u8]) {
        if let Some(t) = self.transport() {
            unsafe {
                (t.live_midi)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(track),
                    bytes.as_ptr(),
                    bytes.len(),
                )
            }
        }
    }

    /// Arms or disarms a track (`None` toggles): the state it is now in, or why it was refused.
    pub fn set_armed(&self, session: &str, track: &str, on: Option<bool>) -> Result<bool, String> {
        let Some(t) = self.transport() else {
            return Err("the engine has no transport area".into());
        };
        let (mut armed, mut reason) = (0i32, String::new());
        let ok = unsafe {
            (t.set_armed)(
                self.host(),
                K7Str::of(session),
                K7Str::of(track),
                want(on),
                &mut armed,
                (&mut reason as *mut String).cast(),
                Some(set_reason),
            )
        } != 0;
        if ok { Ok(armed != 0) } else { Err(reason) }
    }

    /// The recorder node's `record` (`on` `None` toggles) or `clear`; the reason on a refusal.
    pub fn recorder(
        &self,
        session: &str,
        node: &str,
        clear: bool,
        on: Option<bool>,
    ) -> Result<(), String> {
        let Some(t) = self.transport() else {
            return Err("the engine has no transport area".into());
        };
        let mut reason = String::new();
        let ok = unsafe {
            (t.recorder)(
                self.host(),
                K7Str::of(session),
                K7Str::of(node),
                i32::from(clear),
                want(on),
                (&mut reason as *mut String).cast(),
                Some(set_reason),
            )
        } != 0;
        if ok { Ok(()) } else { Err(reason) }
    }

    /// Whether `play` records the armed tracks (`None` toggles).
    pub fn set_record_enabled(&self, session: &str, on: Option<bool>) {
        if let Some(t) = self.transport() {
            unsafe { (t.set_record_enabled)(self.host(), K7Str::of(session), want(on)) }
        }
    }
}
