//! render.rs — the fake engine's render area: a bounce that answers what a test scripted for its output folder,
//! optionally held until released, and a log of the requests it was asked.
//! What a reader gets wrong: the area has no host to hang state on, so it is process-wide and keyed by the output
//! folder, which a test names uniquely; a folder nobody scripted answers a manifest naming a mix.

use super::super::ffi::K7Str;
use super::super::render::{K7RenderApi, K7RenderDone, K7RenderOutcome};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Condvar, Mutex};

/// What one output folder's bounce comes to.
#[derive(Clone, Default)]
pub struct Script {
    pub code: i32,
    pub manifest: String,
    pub error: String,
    pub hold: bool,
}

/// One request the fake was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    pub doc_json: String,
    pub out_dir: String,
    pub stems: bool,
}

#[derive(Default)]
struct Shared {
    scripts: HashMap<String, Script>,
    asked: Vec<Asked>,
    cancels: usize,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
static WAKE: Condvar = Condvar::new();

fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut guard = SHARED.lock().unwrap();
    f(guard.get_or_insert_with(Shared::default))
}

/// What the bounce into `out_dir` will answer.
pub fn script(out_dir: &str, s: Script) {
    with(|sh| sh.scripts.insert(out_dir.to_string(), s));
}

/// Lets a held bounce into `out_dir` finish.
pub fn release(out_dir: &str) {
    with(|sh| {
        if let Some(s) = sh.scripts.get_mut(out_dir) {
            s.hold = false;
        }
    });
    WAKE.notify_all();
}

/// The requests made into `out_dir`.
pub fn asked(out_dir: &str) -> Vec<Asked> {
    with(|sh| {
        sh.asked
            .iter()
            .filter(|a| a.out_dir == out_dir)
            .cloned()
            .collect()
    })
}

/// How many times the server cancelled the renders.
pub fn cancels() -> usize {
    with(|sh| sh.cancels)
}

unsafe extern "C" fn render(
    doc_json: K7Str,
    out_dir: K7Str,
    stems: i32,
    ctx: *mut c_void,
    done: K7RenderDone,
) {
    let (doc_json, out_dir) = unsafe { (doc_json.to_string_lossy(), out_dir.to_string_lossy()) };
    with(|sh| {
        sh.asked.push(Asked {
            doc_json,
            out_dir: out_dir.clone(),
            stems: stems != 0,
        })
    });
    let mut guard = SHARED.lock().unwrap();
    let script = loop {
        let sh = guard.get_or_insert_with(Shared::default);
        let s = sh.scripts.get(&out_dir).cloned().unwrap_or(Script {
            manifest: r#"{"mix":"mix.wav","stems":[]}"#.into(),
            ..Script::default()
        });
        if !s.hold {
            break s;
        }
        guard = WAKE.wait(guard).unwrap();
    };
    drop(guard);
    if let Some(done) = done {
        let outcome = K7RenderOutcome {
            code: script.code,
            manifest: K7Str::of(&script.manifest),
            error: K7Str::of(&script.error),
        };
        unsafe { done(ctx, &outcome) };
    }
}

unsafe extern "C" fn cancel() {
    with(|sh| sh.cancels += 1);
}

pub static TABLE: K7RenderApi = K7RenderApi { render, cancel };
