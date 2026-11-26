//! project.rs — the project area of the C ABI (`k7_engine_project.h`) made safe: the path roots and warnings any
//! task may use, and the apply, the tracks replaced and the reset that only a job on the message thread may do.
//! What a reader gets wrong: an engine without this area answers "no" and does nothing, rather than panicking, so
//! a half-built engine refuses paths (closed) instead of letting every document through.

use super::ffi::{K7Host, K7Str, K7StrEach, K7Warning, K7WarningSink, collect, table};
use super::{Api, Engine, Mt};
use std::ffi::c_void;

table! {
    /// `k7_engine_project.h`.
    K7ProjectApi {
        path_allowed: unsafe extern "C" fn(path: K7Str, write: i32) -> i32,
        warn: unsafe extern "C" fn(session: K7Str, subject: K7Str, message: K7Str),
        standing_warnings: unsafe extern "C" fn(session: K7Str, ctx: *mut c_void, each: K7WarningSink),
        release_slot: unsafe extern "C" fn(host: *mut K7Host) -> i32,
        apply: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, doc_json: K7Str, ctx: *mut c_void, change: K7StrEach, error: K7StrEach) -> i32,
        track_ids: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, ctx: *mut c_void, each: K7StrEach),
        apply_tracks: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, plan_json: K7Str, ctx: *mut c_void, error: K7StrEach) -> i32,
        reset: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
    }
}

impl Api {
    /// The project area, `None` while the engine has not built it.
    pub fn project(&self) -> Option<&'static K7ProjectApi> {
        let p = self.table().project.cast::<K7ProjectApi>();
        unsafe { p.as_ref() }
    }
}

/// A `(subject, message)` the engine kept for a project.
pub type Standing = (String, String);

unsafe extern "C" fn push_warning(ctx: *mut c_void, w: *const K7Warning) {
    let out = unsafe { &mut *ctx.cast::<Vec<Standing>>() };
    let w = unsafe { &*w };
    out.push(unsafe { (w.subject.to_string_lossy(), w.message.to_string_lossy()) });
}

impl Engine {
    /// ANY THREAD. Whether a document may name `path` for reading or writing; closed without the area.
    pub fn path_allowed(&self, path: &str, write: bool) -> bool {
        self.api()
            .project()
            .is_some_and(|p| unsafe { (p.path_allowed)(K7Str::of(path), i32::from(write)) != 0 })
    }

    /// ANY THREAD. A warning as the engine's `warn()`: stderr, then the sink under `session`.
    pub fn warn(&self, session: &str, subject: &str, message: &str) {
        if let Some(p) = self.api().project() {
            unsafe { (p.warn)(K7Str::of(session), K7Str::of(subject), K7Str::of(message)) }
        }
    }

    /// ANY THREAD. What `session`'s project was built with, oldest first.
    pub fn standing_warnings(&self, session: &str) -> Vec<Standing> {
        let mut out: Vec<Standing> = Vec::new();
        if let Some(p) = self.api().project() {
            let ctx = (&mut out as *mut Vec<Standing>).cast();
            unsafe { (p.standing_warnings)(K7Str::of(session), ctx, Some(push_warning)) }
        }
        out
    }
}

impl Mt<'_> {
    fn project(&self) -> Option<&'static K7ProjectApi> {
        self.api().project()
    }

    /// Drops the slot's chain when it is loaded.
    pub fn release_slot(&self) -> bool {
        self.project()
            .is_some_and(|p| unsafe { (p.release_slot)(self.host()) != 0 })
    }

    /// Applies a validated document: each change described, or the reason it failed.
    pub fn apply_project(&self, session: &str, doc_json: &str) -> Result<Vec<String>, String> {
        let Some(p) = self.project() else {
            return Err("the engine has no project area".into());
        };
        let mut out = Applied::default();
        let ok = unsafe {
            (p.apply)(
                self.host(),
                K7Str::of(session),
                K7Str::of(doc_json),
                (&mut out as *mut Applied).cast(),
                Some(push_change),
                Some(set_error),
            )
        } != 0;
        if ok { Ok(out.changes) } else { Err(out.error) }
    }

    /// The ids of the tracks the session holds, in document order; empty when it holds no project.
    pub fn track_ids(&self, session: &str) -> Vec<String> {
        let Some(p) = self.project() else {
            return Vec::new();
        };
        collect(|ctx, each| unsafe { (p.track_ids)(self.host(), K7Str::of(session), ctx, each) })
    }

    /// Replaces tracks by position (`[{"index":n,"track":{..}}]`, validated) and applies.
    pub fn apply_tracks(&self, session: &str, plan_json: &str) -> Result<(), String> {
        let Some(p) = self.project() else {
            return Err("the engine has no project area".into());
        };
        let mut error = String::new();
        let ok = unsafe {
            (p.apply_tracks)(
                self.host(),
                K7Str::of(session),
                K7Str::of(plan_json),
                (&mut error as *mut String).cast(),
                Some(set_reason),
            )
        } != 0;
        if ok { Ok(()) } else { Err(error) }
    }

    /// Forgets the project: plugins, edit and diff baseline.
    pub fn reset_project(&self, session: &str) {
        if let Some(p) = self.project() {
            unsafe { (p.reset)(self.host(), K7Str::of(session)) }
        }
    }
}

/// What `apply` hands back through its one context: the changes, or the reason.
#[derive(Default)]
struct Applied {
    changes: Vec<String>,
    error: String,
}

unsafe extern "C" fn push_change(ctx: *mut c_void, item: K7Str) {
    let out = unsafe { &mut *ctx.cast::<Applied>() };
    out.changes.push(unsafe { item.to_string_lossy() });
}

unsafe extern "C" fn set_error(ctx: *mut c_void, item: K7Str) {
    let out = unsafe { &mut *ctx.cast::<Applied>() };
    out.error = unsafe { item.to_string_lossy() };
}

/// `apply_tracks`'s context: only the reason comes back.
unsafe extern "C" fn set_reason(ctx: *mut c_void, item: K7Str) {
    let out = unsafe { &mut *ctx.cast::<String>() };
    *out = unsafe { item.to_string_lossy() };
}
