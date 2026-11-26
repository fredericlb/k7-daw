//! state.rs — what the engine says without being asked: the ~20 Hz `state` frame to the sockets that asked about
//! its project, a finished take, a plugin's parameters (to persist, and live), an engine warning; and the verb
//! that watches a plugin. `install` starts the engine's timer and hands it these sinks; `uninstall` clears the warning's.
//! What a reader gets wrong: only the state frame is addressed by socket and droppable; the others are replies to
//! a project, stamped with its session, that every trusted socket receives and filters by `forSession`.

use super::ctx::{Ctx, Request};
use super::handlers::HandlerTable;
use super::msg::Malformed;
use super::state_frame;
use crate::engine::state::{Recorded, StateSinks};
use crate::engine::{Mt, Warning};
use crate::wire::json;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The engine's state timer, in ms: ~20 frames a second for a session in use (a quiet one is slowed by the engine).
pub const STATE_INTERVAL_MS: i32 = 50;

pub fn add(t: &mut HandlerTable) {
    t.add(&["watch-plugin-params"], watch);
}

/// Subscribe a plugin to the live parameter stream; the watch set is the engine's, so a second watcher is a no-op.
fn watch(r: &Request) -> Result<(), Malformed> {
    let id = r.msg.str("id")?;
    let on = r.msg.bool_or("on", true)?;
    if !id.is_empty() {
        let session = r.session.clone();
        r.ctx
            .engine
            .post(move |mt: Mt<'_>| mt.watch_plugin_params(&session, &id, on));
    }
    Ok(())
}

/// A socket just bound to `session`: it holds none of the take being recorded, so the next frame carries it all.
pub fn joined(ctx: &Ctx, session: &str) {
    let session = session.to_string();
    ctx.engine
        .post(move |mt: Mt<'_>| mt.rewind_recorder_envelopes(&session));
}

/// The state frame to the sockets that asked about its session, droppable: the next frame supersedes it.
fn send_state(ctx: &Ctx, s: &crate::engine::state::StateFrame<'_>) {
    let targets = ctx.state_targets(&s.session());
    // Nobody asked about this session, so the frame is not even built.
    if targets.is_empty() {
        return;
    }
    let payload: Arc<str> = Arc::from(json::dump(&state_frame::frame(s)));
    for client in targets {
        client.send_text(&payload, true);
    }
}

fn recorded(ctx: &Ctx, take: Recorded) {
    ctx.broadcast_to(
        &take.session,
        json!({"type": "recorded", "track": take.track, "file": take.file, "start": take.start, "length": take.length}),
    );
}

/// The editor closed and the engine captured the plugin's state: text the client persists to the module.
fn plugin_params(ctx: &Ctx, session: &str, id: &str, params_json: &str) {
    let params =
        serde_json::from_str::<Value>(params_json).unwrap_or_else(|_| Value::Object(Map::new()));
    ctx.broadcast_to(
        session,
        json!({"type": "plugin-params", "id": id, "params": params}),
    );
}

/// A watched plugin's values: its own type, since `plugin-params` means "persist this".
fn plugin_params_live(ctx: &Ctx, session: &str, id: &str, values: &[(String, f64)]) {
    let params: Map<String, Value> = values
        .iter()
        .map(|(name, value)| (name.clone(), json!(value)))
        .collect();
    ctx.broadcast_to(
        session,
        json!({"type": "plugin-params-live", "id": id, "params": params}),
    );
}

/// Stamped when a warning named a project, unstamped otherwise: a device or startup warning belongs to no session.
fn warning(ctx: &Ctx, w: Warning) {
    let msg = json!({"type": "engine-warning", "subject": w.subject, "message": w.message});
    if w.session.is_empty() {
        ctx.broadcast(&msg);
    } else {
        ctx.broadcast_to(&w.session, msg);
    }
}

/// The warning sink, installed: engine warnings reach the clients and not just stderr.
pub fn install_warnings(ctx: &Arc<Ctx>) {
    let weak = Arc::downgrade(ctx);
    ctx.engine.api().set_warning_sink(Some(Box::new(move |w| {
        if let Some(ctx) = weak.upgrade() {
            warning(&ctx, w);
        }
    })));
}

/// The state timer and its events, started on the message thread, and the warning sink.
pub fn install(ctx: &Arc<Ctx>) {
    install_warnings(ctx);
    let (a, b, c, d) = (
        Arc::downgrade(ctx),
        Arc::downgrade(ctx),
        Arc::downgrade(ctx),
        Arc::downgrade(ctx),
    );
    let sinks = StateSinks {
        state: Box::new(move |s| {
            if let Some(ctx) = a.upgrade() {
                send_state(&ctx, s);
            }
        }),
        recorded: Box::new(move |take| {
            if let Some(ctx) = b.upgrade() {
                recorded(&ctx, take);
            }
        }),
        params: Box::new(move |session, id, text| {
            if let Some(ctx) = c.upgrade() {
                plugin_params(&ctx, session, id, text);
            }
        }),
        params_live: Box::new(move |session, id, values| {
            if let Some(ctx) = d.upgrade() {
                plugin_params_live(&ctx, session, id, values);
            }
        }),
    };
    ctx.engine
        .post(move |mt: Mt<'_>| mt.start_state(sinks, STATE_INTERVAL_MS));
}

/// After the server stops: the engine says its warnings on stderr alone.
pub fn uninstall(ctx: &Ctx) {
    ctx.engine.api().set_warning_sink(None);
}
