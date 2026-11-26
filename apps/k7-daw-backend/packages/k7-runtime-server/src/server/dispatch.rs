//! dispatch.rs — what happens to one text frame, in the C++ server's order: the size screen, the parse, the
//! gate, the session's length and ceiling, the first real message's line and subscription, then the handler.
//! What a reader gets wrong: the one `catch_unwind` and the `Malformed` answer are load-bearing — a handler's
//! failure is answered and logged and the socket stays, where no answer would leave a client deaf for good.

use super::ctx::{Client, Ctx, MAX_SESSION_ID_BYTES, MAX_SESSIONS, Request};
use super::handlers::HandlerTable;
use super::msg::{self, Malformed, Msg};
use crate::wire::catalogue::{OVERRIDE_QUERIES, PROBE_QUERIES, SUPERSEDABLE, is};
use serde_json::json;
use std::collections::hash_map::Entry;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

/// An `error` with a message, to `client` alone.
fn error(client: &Client, message: &str) {
    client.send(&json!({"type": "error", "message": message}));
}

/// Answers one text frame; `announced` is whether this connection has earned its `[client] connected` line.
pub fn handle(
    ctx: &Arc<Ctx>,
    table: &HandlerTable,
    client: &Arc<Client>,
    text: &str,
    announced: &mut bool,
) {
    if let Some(out) = ctx
        .gate
        .screen_frame_size(client.id, &client.ip, text.len())
    {
        ctx.perform(client, vec![out]);
        return;
    }
    let value = match msg::parse(text) {
        Ok(v) => v,
        Err(e) => {
            error(client, &format!("bad json: {e}"));
            return;
        }
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        route(ctx, table, client, Msg(value), announced)
    }));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(Malformed(what))) => {
            eprintln!("[client] refused a message: {what}");
            error(client, &format!("malformed message: {what}"));
        }
        Err(_) => {
            eprintln!("[client] refused a message (unknown exception)");
            error(client, "malformed message");
        }
    }
}

fn route(
    ctx: &Arc<Ctx>,
    table: &HandlerTable,
    client: &Arc<Client>,
    msg: Msg,
    announced: &mut bool,
) -> Result<(), Malformed> {
    let ty = msg.str("type")?;
    if let Some(outs) = ctx.gate.screen_message(client.id, &client.ip, &msg.0, &ty) {
        ctx.perform(client, outs);
        return Ok(());
    }
    // WHICH project this message is about; absent means the default session.
    let session = msg.str("session")?;
    if session.len() > MAX_SESSION_ID_BYTES {
        error(client, "session id too long");
        return Ok(());
    }
    let probe = is(PROBE_QUERIES, &ty);
    // Naming a session creates it, so only a ceiling on HOW MANY: a NEW one past it is refused, an existing one answered.
    let creates = !probe || msg.str("role")? == "driver";
    if !session.is_empty()
        && creates
        && !ctx.engine.has_session(&session)
        && ctx.engine.session_ids().len() >= MAX_SESSIONS
    {
        error(client, "this engine holds too many sessions");
        return Ok(());
    }
    // The first REAL message earns the deferred line, and — unless it names another project — the subscription.
    if !probe {
        if !*announced {
            *announced = true;
            eprintln!("[client] connected ({})", client.ip);
        }
        if !is(OVERRIDE_QUERIES, &ty) {
            let joined = match ctx.bindings().subscribers.entry(client.id) {
                Entry::Vacant(slot) => {
                    slot.insert(session.clone());
                    true
                }
                Entry::Occupied(_) => false,
            };
            if joined {
                super::state::joined(ctx, &session);
            }
        }
    }
    // Every verb but the three that can be superseded, and the read-only probes, fences what was queued before it.
    if !is(SUPERSEDABLE, &ty) && !probe {
        ctx.supersede.barrier(&session);
    }
    match table.find(&ty) {
        Some(handle) => handle(&Request {
            ctx: ctx.clone(),
            client: client.clone(),
            msg,
            session,
            ty,
        }),
        None => {
            // A message we don't understand is version skew: logged here too, or the only trace is an error the client may ignore.
            eprintln!("[client] unknown message type: '{ty}'");
            error(client, &format!("unknown message type: {ty}"));
            Ok(())
        }
    }
}
