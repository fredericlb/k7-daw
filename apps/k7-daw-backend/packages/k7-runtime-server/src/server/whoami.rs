//! whoami.rs — `whoami` and `hello`: the identity probe, answered to the asking socket on its own task and never
//! through the message thread, since the reads are atomics a wedged message thread cannot hold up.
//! What a reader gets wrong: a `hello` with `role: "driver"` also starts counting the socket as a driver, once
//! per socket, because the keepalive re-sends whoami.

use super::ctx::Request;
use super::handlers::HandlerTable;
use super::msg::Malformed;
use crate::wire::version::{ENGINE_PROTOCOL, build};
use serde_json::{Value, json};
use std::collections::hash_map::Entry;

pub fn add(t: &mut HandlerTable) {
    t.add(&["whoami", "hello"], answer);
}

/// A path's last component, either separator: a reply names a plugin's file, never the user's folders.
fn file_name_of(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The crashes, and the file the last engine died loading or the plugin it loaded last, each only when known.
fn safe_mode(r: &Request) -> Value {
    let api = r.ctx.engine.api();
    let mut out = json!({"crashes": r.ctx.safe_mode_crashes});
    for (key, path) in [
        ("diedLoading", api.previous_load_death()),
        ("lastPlugin", api.previous_last_plugin()),
    ] {
        if !path.is_empty() {
            out[key] = file_name_of(&path).into();
        }
    }
    out
}

fn answer(r: &Request) -> Result<(), Malformed> {
    let (frames, rate) = r
        .ctx
        .engine
        .audio_timing()
        .map_or((0, 0.0), |t| (t.buffer_frames, t.sample_rate));
    let mut reply = json!({
        "type": "whoami",
        "id": r.ctx.identity.id,
        "token": r.ctx.identity.token,
        "boot": r.ctx.identity.boot_id,
        "protocol": ENGINE_PROTOCOL,
        "build": build(),
        "bufferSize": frames,
        "sampleRate": rate,
        "audioToBrowser": r.ctx.audio_to_browser,
        // How long the message thread has been silent: the supervisor restarts an engine wedged there.
        "messageThreadMs": r.ctx.engine.message_thread_age_ms().unwrap_or(0.0),
    });
    if r.ctx.safe_mode_crashes > 0 {
        reply["safeMode"] = safe_mode(r);
    }
    r.client.send(&reply);
    if r.msg.str("role")? == "driver" {
        // Once per socket: a later hello naming another session does not move it.
        let new = match r.ctx.bindings().drivers.entry(r.client.id) {
            Entry::Vacant(slot) => {
                slot.insert(r.session.clone());
                true
            }
            Entry::Occupied(_) => false,
        };
        if new {
            let session = r.session.clone();
            r.ctx.engine.post(move |mt| mt.add_driver(&session));
            // A driver arriving after the build is told what that build complained of, which was said only once.
            for (subject, message) in r.ctx.engine.standing_warnings(&r.session) {
                r.client.send(&json!({
                    "type": "engine-warning",
                    "subject": subject,
                    "message": message,
                    "session": r.session,
                }));
            }
        }
    }
    Ok(())
}
