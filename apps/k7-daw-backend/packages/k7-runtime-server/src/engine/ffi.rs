//! ffi.rs — k7-runtime's `engine/capi/k7_engine*.h`, field for field: the table of area tables and each area's
//! functions, `#[repr(C)]`. `tests/all/engine_abi.rs` holds every table's field names to its header, in order.
//! What a reader gets wrong: an area not built yet is a `*const c_void` slot here and NULL in C++; its milestone
//! turns the slot into its own table type, defined in a module of its own beside this one.

// Each unsafe function states its condition in its one-line doc, which the comment rule keeps to one line.
#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_char, c_void};
use std::marker::{PhantomData, PhantomPinned};

/// The version of the table this side was written against (`K7_ENGINE_ABI`).
pub const K7_ENGINE_ABI: u32 = 1;

/// UTF-8 bytes, not NUL-terminated, borrowed for the call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct K7Str {
    pub ptr: *const c_char,
    pub len: usize,
}

impl K7Str {
    /// A view of `s` for the duration of one call.
    pub fn of(s: &str) -> K7Str {
        K7Str {
            ptr: s.as_ptr().cast(),
            len: s.len(),
        }
    }

    /// The bytes as a `String`, invalid UTF-8 replaced. Safety: `ptr` names `len` readable bytes, or is NULL.
    pub unsafe fn to_string_lossy(self) -> String {
        if self.ptr.is_null() || self.len == 0 {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr.cast::<u8>(), self.len) };
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// The engine host, opaque.
#[repr(C)]
pub struct K7Host {
    _data: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

pub type K7Task = Option<unsafe extern "C" fn(ctx: *mut c_void)>;
pub type K7StrEach = Option<unsafe extern "C" fn(ctx: *mut c_void, item: K7Str)>;

/// Declares a table and the list of its field names, which the header test compares in order.
macro_rules! table {
    ($(#[$meta:meta])* $name:ident { $($field:ident : $ty:ty,)* }) => {
        $(#[$meta])*
        #[repr(C)]
        pub struct $name {
            $(pub $field: $ty,)*
        }

        impl $name {
            pub const FIELDS: &'static [&'static str] = &[$(stringify!($field)),*];
        }
    };
}

table! {
    /// What `k7_engine_api()` returns; the area tables other than `core` are NULL until built.
    K7EngineApi {
        abi: u32,
        size: u32,
        core: *const K7CoreApi,
        project: *const c_void,
        transport: *const c_void,
        state: *const c_void,
        plugins: *const c_void,
        machine: *const c_void,
        render: *const c_void,
        cli: *const c_void,
    }
}

// The table is built once, never written, and lives as long as the process.
unsafe impl Sync for K7EngineApi {}
unsafe impl Send for K7EngineApi {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct K7AudioTiming {
    pub buffer_frames: i32,
    pub sample_rate: f64,
}

#[repr(C)]
pub struct K7Warning {
    pub session: K7Str,
    pub subject: K7Str,
    pub message: K7Str,
}

pub type K7WarningSink = Option<unsafe extern "C" fn(ctx: *mut c_void, warning: *const K7Warning)>;

table! {
    /// `k7_engine_core.h`: the process's setup, the host's life, its message thread, the any-thread answers.
    K7CoreApi {
        layout_consistent: unsafe extern "C" fn() -> i32,
        install_crash_handler: unsafe extern "C" fn(),
        init_settings_dir: unsafe extern "C" fn(flag: K7Str),
        settings_dir: unsafe extern "C" fn(ctx: *mut c_void, out: K7StrEach),
        state_path: unsafe extern "C" fn(ctx: *mut c_void, out: K7StrEach),
        enable_load_guards: unsafe extern "C" fn(),
        set_safe_mode: unsafe extern "C" fn(on: i32),
        use_default_path_roots: unsafe extern "C" fn(),
        set_warning_sink: unsafe extern "C" fn(sink: K7WarningSink, ctx: *mut c_void),
        host_create: unsafe extern "C" fn() -> *mut K7Host,
        host_destroy: unsafe extern "C" fn(host: *mut K7Host),
        run: unsafe extern "C" fn(host: *mut K7Host),
        quit: unsafe extern "C" fn(host: *mut K7Host),
        post: unsafe extern "C" fn(host: *mut K7Host, run: K7Task, drop: K7Task, ctx: *mut c_void),
        open_audio_device: unsafe extern "C" fn(host: *mut K7Host),
        output_device_name: unsafe extern "C" fn(host: *mut K7Host, ctx: *mut c_void, out: K7StrEach),
        suspend_audio: unsafe extern "C" fn(host: *mut K7Host, suspended: i32),
        audio_timing: unsafe extern "C" fn(host: *const K7Host) -> K7AudioTiming,
        message_thread_age_ms: unsafe extern "C" fn(host: *const K7Host) -> f64,
        session_ids: unsafe extern "C" fn(host: *const K7Host, ctx: *mut c_void, each: K7StrEach),
        has_session: unsafe extern "C" fn(host: *const K7Host, id: K7Str) -> i32,
    }
}

/// A `K7StrEach` that pushes each string into the `Vec<String>` its context points at.
unsafe extern "C" fn push_string(ctx: *mut c_void, item: K7Str) {
    let out = unsafe { &mut *ctx.cast::<Vec<String>>() };
    out.push(unsafe { item.to_string_lossy() });
}

/// Every string a `K7StrEach` function hands over, in order.
pub fn collect(call: impl FnOnce(*mut c_void, K7StrEach)) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    call((&mut out as *mut Vec<String>).cast(), Some(push_string));
    out
}

/// The one string a `K7StrEach` answer hands over, empty when none came.
pub fn collect_one(call: impl FnOnce(*mut c_void, K7StrEach)) -> String {
    collect(call).into_iter().next().unwrap_or_default()
}
