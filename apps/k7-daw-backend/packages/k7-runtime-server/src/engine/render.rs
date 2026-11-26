//! render.rs — the render area of the C ABI (`k7_engine_render.h`) made safe: a validated document bounced inside
//! the serving engine, on the caller's thread, which waits for the whole bounce.
//! What a reader gets wrong: `render_in_process` is never called from the message thread or a socket's task, since
//! the bounce needs the message loop running; an engine without the area answers a failed outcome, not a panic.

use super::ffi::{K7Str, table};
use super::Api;
use std::ffi::c_void;

#[repr(C)]
pub struct K7RenderOutcome {
    pub code: i32,
    pub manifest: K7Str,
    pub error: K7Str,
}

pub type K7RenderDone =
    Option<unsafe extern "C" fn(ctx: *mut c_void, outcome: *const K7RenderOutcome)>;

table! {
    /// `k7_engine_render.h`.
    K7RenderApi {
        render: unsafe extern "C" fn(doc_json: K7Str, out_dir: K7Str, stems: i32, ctx: *mut c_void, done: K7RenderDone),
        cancel: unsafe extern "C" fn(),
    }
}

/// What one bounce came to: 0 and the manifest `--render` prints, or a non-zero code and why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderOutcome {
    pub code: i32,
    pub manifest: String,
    pub error: String,
}

unsafe extern "C" fn take_outcome(ctx: *mut c_void, outcome: *const K7RenderOutcome) {
    let out = unsafe { &mut *ctx.cast::<RenderOutcome>() };
    let o = unsafe { &*outcome };
    *out = unsafe {
        RenderOutcome {
            code: o.code,
            manifest: o.manifest.to_string_lossy(),
            error: o.error.to_string_lossy(),
        }
    };
}

impl Api {
    /// The render area, `None` while the engine has not built it.
    pub fn render_area(&self) -> Option<&'static K7RenderApi> {
        let p = self.table().render.cast::<K7RenderApi>();
        unsafe { p.as_ref() }
    }

    /// BLOCKS for the whole bounce: any thread but the message thread.
    pub fn render_in_process(&self, doc_json: &str, out_dir: &str, stems: bool) -> RenderOutcome {
        let Some(r) = self.render_area() else {
            return RenderOutcome {
                code: 2,
                manifest: String::new(),
                error: "the engine has no render area".into(),
            };
        };
        let mut out = RenderOutcome {
            code: 2,
            manifest: String::new(),
            error: "the render never answered".into(),
        };
        unsafe {
            (r.render)(
                K7Str::of(doc_json),
                K7Str::of(out_dir),
                i32::from(stems),
                (&mut out as *mut RenderOutcome).cast(),
                Some(take_outcome),
            )
        };
        out
    }

    /// Once the message loop has stopped: renders in progress give up what they waited on.
    pub fn cancel_renders(&self) {
        if let Some(r) = self.render_area() {
            unsafe { (r.cancel)() }
        }
    }
}
