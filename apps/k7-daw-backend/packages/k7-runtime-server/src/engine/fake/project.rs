//! project.rs — the fake engine's project area: the document each session was last applied, an apply that
//! answers what a test scripted, and the path check the real engine has (a UNC path never passes, no roots set).
//! What a reader gets wrong: the fake holds the validated JSON as it received it, so a test reads what the
//! Rust side handed the C++ reader, not what a diff made of it.

use super::super::ffi::{K7Host, K7Str, K7StrEach, K7Warning, K7WarningSink};
use super::super::project::K7ProjectApi;
use super::{fake, give, log, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

/// The standing warnings each session's project carries: a call that names no host reads them process-wide.
static STANDING: Mutex<Vec<(String, String, String)>> = Mutex::new(Vec::new());

/// Sets the standing warnings of `session` as `(subject, message)`.
pub fn set_standing(session: &str, warnings: &[(&str, &str)]) {
    if let Ok(mut all) = STANDING.lock() {
        all.retain(|(s, _, _)| s != session);
        all.extend(
            warnings
                .iter()
                .map(|(a, b)| (session.to_owned(), (*a).to_owned(), (*b).to_owned())),
        );
    }
}

/// What a test reads and scripts of the project area.
#[derive(Default)]
pub struct ProjectFake {
    /// The document each session was last applied, as the JSON the server sent.
    pub docs: HashMap<String, Value>,
    /// What `apply` answers as changes.
    pub changes: Vec<String>,
    /// When set, `apply` and `apply_tracks` fail with it.
    pub apply_error: Option<String>,
    /// Whether the slot's chain is loaded: what `release_slot` drops.
    pub slot_loaded: bool,
}

fn with<R>(host: *const K7Host, f: impl FnOnce(&mut super::FakeState) -> R) -> Option<R> {
    fake(host).state.lock().ok().map(|mut s| f(&mut s))
}

/// `\\host\share`, `//host/share`: a path that would make the engine dial a network share.
fn is_unc(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

unsafe extern "C" fn path_allowed(path: K7Str, _write: i32) -> i32 {
    i32::from(!is_unc(&unsafe { path.to_string_lossy() }))
}

unsafe extern "C" fn warn_under(session: K7Str, subject: K7Str, message: K7Str) {
    let (session, subject, message) = unsafe {
        (
            session.to_string_lossy(),
            subject.to_string_lossy(),
            message.to_string_lossy(),
        )
    };
    warn(&session, &subject, &message);
}

unsafe extern "C" fn standing_warnings(session: K7Str, ctx: *mut c_void, each: K7WarningSink) {
    let session = unsafe { session.to_string_lossy() };
    let Some(each) = each else { return };
    let mine: Vec<(String, String)> = STANDING
        .lock()
        .map(|all| {
            all.iter()
                .filter(|(s, _, _)| *s == session)
                .map(|(_, a, b)| (a.clone(), b.clone()))
                .collect()
        })
        .unwrap_or_default();
    for (subject, message) in mine {
        let w = K7Warning {
            session: K7Str::of(&session),
            subject: K7Str::of(&subject),
            message: K7Str::of(&message),
        };
        unsafe { each(ctx, &w) };
    }
}

unsafe extern "C" fn release_slot(host: *mut K7Host) -> i32 {
    with(host, |s| {
        s.calls.push("release_slot".into());
        i32::from(std::mem::take(&mut s.project.slot_loaded))
    })
    .unwrap_or(0)
}

unsafe extern "C" fn apply(
    host: *mut K7Host,
    session: K7Str,
    doc_json: K7Str,
    ctx: *mut c_void,
    change: K7StrEach,
    error: K7StrEach,
) -> i32 {
    let session = unsafe { session.to_string_lossy() };
    let doc: Result<Value, _> = serde_json::from_str(&unsafe { doc_json.to_string_lossy() });
    let (failure, changes) = with(host, |s| {
        s.calls.push(format!("apply {session}"));
        let failure = match (&doc, &s.project.apply_error) {
            (Err(e), _) => Some(e.to_string()),
            (_, Some(e)) => Some(e.clone()),
            _ => None,
        };
        if failure.is_none()
            && let Ok(doc) = doc
        {
            if session == "slot" {
                s.project.slot_loaded = true;
            }
            s.project.docs.insert(session.clone(), doc);
        }
        (failure, s.project.changes.clone())
    })
    .unwrap_or((Some("no host".into()), Vec::new()));
    if let Some(why) = failure {
        give(error, ctx, &why);
        return 0;
    }
    for c in changes {
        give(change, ctx, &c);
    }
    1
}

unsafe extern "C" fn track_ids(
    host: *mut K7Host,
    session: K7Str,
    ctx: *mut c_void,
    each: K7StrEach,
) {
    let session = unsafe { session.to_string_lossy() };
    let ids = with(host, |s| {
        s.project
            .docs
            .get(&session)
            .and_then(|d| d["tracks"].as_array())
            .map(|t| {
                t.iter()
                    .map(|t| t["id"].as_str().unwrap_or_default().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default();
    for id in ids {
        give(each, ctx, &id);
    }
}

unsafe extern "C" fn apply_tracks(
    host: *mut K7Host,
    session: K7Str,
    plan_json: K7Str,
    ctx: *mut c_void,
    error: K7StrEach,
) -> i32 {
    let session = unsafe { session.to_string_lossy() };
    let plan: Value =
        serde_json::from_str(&unsafe { plan_json.to_string_lossy() }).unwrap_or_default();
    let failure = with(host, |s| {
        s.calls.push(format!("apply_tracks {session}"));
        if let Some(e) = &s.project.apply_error {
            return Some(e.clone());
        }
        let Some(tracks) = s
            .project
            .docs
            .get_mut(&session)
            .and_then(|d| d["tracks"].as_array_mut())
        else {
            return Some("no project".to_owned());
        };
        for step in plan.as_array().into_iter().flatten() {
            let Some(slot) = step["index"]
                .as_u64()
                .and_then(|i| tracks.get_mut(i as usize))
            else {
                return Some("track index past the document".to_owned());
            };
            *slot = step["track"].clone();
        }
        None
    })
    .flatten();
    match failure {
        Some(why) => {
            give(error, ctx, &why);
            0
        }
        None => 1,
    }
}

unsafe extern "C" fn reset(host: *mut K7Host, session: K7Str) {
    let session = unsafe { session.to_string_lossy() };
    with(host, |s| {
        s.project.docs.remove(&session);
    });
    log(host, &format!("reset {session}"));
}

pub static TABLE: K7ProjectApi = K7ProjectApi {
    path_allowed,
    warn: warn_under,
    standing_warnings,
    release_slot,
    apply,
    track_ids,
    apply_tracks,
    reset,
};
