//! whoami.rs — `whoami` and `hello`: the identity probe, answered to the asking socket on its own task and never
//! through the message thread, since the reads are atomics a wedged message thread cannot hold up.
//! What a reader gets wrong: a `hello` with `role: "driver"` also starts counting the socket as a driver, once
//! per socket, because the keepalive re-sends whoami.

use super::ctx::Request;
use super::handlers::HandlerTable;
use super::msg::Malformed;
use crate::wire::version::{ENGINE_PROTOCOL, build};
use serde_json::json;

pub fn add(t: &mut HandlerTable) {
    t.add(&["whoami", "hello"], answer);
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
        reply["safeMode"] = json!({"crashes": r.ctx.safe_mode_crashes});
    }
    r.client.send(&reply);
    if r.msg.str("role")? == "driver" {
        r.ctx
            .bindings()
            .drivers
            .entry(r.client.id)
            .or_insert_with(|| r.session.clone());
    }
    Ok(())
}
