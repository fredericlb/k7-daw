//! transport.rs — the verbs that act on one project's playing and recording: `transport`, the auditions
//! (`preview`, `note-preview`), a live keyboard (`midi-live`), `arm`, `recorder` and `record-enable`.
//! What a reader gets wrong: a field of the wrong type is a `Malformed` answer before anything is posted, and a
//! `midi-live` is followed per socket only when the engine will play it, so a dropped socket's notes are released.

use super::ctx::{Client, Ctx, Request};
use super::handlers::HandlerTable;
use super::msg::Malformed;
use serde_json::{Value, json};

pub fn add(t: &mut HandlerTable) {
    t.add(&["transport"], transport);
    t.add(&["preview"], preview);
    t.add(&["note-preview"], note_preview);
    t.add(&["midi-live"], midi_live);
    t.add(&["arm"], arm);
    t.add(&["recorder"], recorder);
    t.add(&["record-enable"], record_enable);
}

/// A boolean field only when it is one: an absent or mistyped one asks for a toggle.
fn flag(r: &Request, key: &str) -> Result<Option<bool>, Malformed> {
    Ok(r.msg.get(key)?.and_then(Value::as_bool))
}

fn transport(r: &Request) -> Result<(), Malformed> {
    let action = r.msg.str("action")?;
    let position = r.msg.f64_or("position", 0.0)?;
    let (start, end) = (r.msg.f64_or("start", 0.0)?, r.msg.f64_or("end", 0.0)?);
    let on = r.msg.bool_or("enabled", false)?;
    let session = r.session.clone();
    r.ctx.engine.post(move |mt| {
        // One line per move tells "the engine never got play" from "it played and you heard nothing".
        let mut line = format!("[transport] {action}");
        match action.as_str() {
            "seek" => line += &format!(" -> {position}s"),
            "loop" => line += &format!("{} [{start}, {end}]", if on { " on" } else { " off" }),
            "toggle" => {
                line += if mt.is_playing(&session) {
                    " (playing -> stop)"
                } else {
                    " (stopped -> play)"
                }
            }
            _ => {}
        }
        eprintln!("{line}");
        match action.as_str() {
            "play" => mt.play(&session),
            "stop" => mt.stop(&session),
            "toggle" if mt.is_playing(&session) => mt.stop(&session),
            "toggle" => mt.play(&session),
            "rewind" => mt.rewind(&session),
            "panic" => mt.panic(&session),
            "seek" => mt.seek(&session, position),
            "loop" => mt.set_loop(&session, start, end, on),
            _ => {}
        }
    });
    Ok(())
}

fn preview(r: &Request) -> Result<(), Malformed> {
    let file = r.msg.str("file")?;
    let (start, length) = (r.msg.f64_or("start", 0.0)?, r.msg.f64_or("length", 0.0)?);
    // A file named outside any document is held to the same read roots as one inside.
    if !file.is_empty() && !r.ctx.engine.path_allowed(&file, false) {
        eprintln!("[preview] refused {file}: outside the folders this engine reads");
        return Ok(());
    }
    if !file.is_empty() {
        let session = r.session.clone();
        r.ctx.engine.post(move |mt| {
            eprintln!("[preview] {file} [{start}, {length}]");
            mt.preview_sample(&session, &file, start, length);
        });
    }
    Ok(())
}

fn note_preview(r: &Request) -> Result<(), Malformed> {
    let track = r.msg.str("track")?;
    let key = r.msg.i64_or("key", -1)?;
    let vel = r.msg.f64_or("vel", 1.0)?;
    if !track.is_empty() && (0..=127).contains(&key) {
        let session = r.session.clone();
        r.ctx.engine.post(move |mt| {
            mt.preview_note(&session, &track, key as i32, vel as f32);
        });
    }
    Ok(())
}

fn midi_live(r: &Request) -> Result<(), Malformed> {
    let track = r.msg.str("track")?;
    let bytes: Vec<u8> = match r.msg.get("data")? {
        Some(Value::Array(data)) => data
            .iter()
            .filter(|b| b.is_i64() || b.is_u64())
            .map(|b| b.as_i64().unwrap_or(255).clamp(0, 255) as u8)
            .collect(),
        _ => Vec::new(),
    };
    if track.is_empty() || bytes.is_empty() {
        return Ok(());
    }
    // Followed per socket only when the engine will play it, so a socket that drops mid-note has its notes released.
    if bytes.len() <= 3 && (0x80..0xF0).contains(&bytes[0]) {
        r.ctx
            .held_notes
            .sent(r.client.id, &r.session, &track, &bytes);
    }
    let session = r.session.clone();
    r.ctx
        .engine
        .post(move |mt| mt.live_midi(&session, &track, &bytes));
    Ok(())
}

fn arm(r: &Request) -> Result<(), Malformed> {
    let track = r.msg.str("track")?;
    let armed = flag(r, "armed")?;
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx
        .engine
        .post(move |mt| match mt.set_armed(&session, &track, armed) {
            Ok(now) => ctx.broadcast_to(
                &session,
                json!({"type": "armed", "track": track, "armed": now}),
            ),
            Err(why) => refused(&ctx, &session, &why),
        });
    Ok(())
}

/// A recording verb the engine refused, said in the log and to the project's sockets.
fn refused(ctx: &Ctx, session: &str, why: &str) {
    eprintln!("[rec] refused: {why}");
    ctx.broadcast_to(session, json!({"type": "error", "message": why}));
}

fn recorder(r: &Request) -> Result<(), Malformed> {
    let node = r.msg.str("node")?;
    let action = r.msg.str("action")?;
    let on = flag(r, "on")?;
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.engine.post(move |mt| {
        if node.is_empty() {
            return;
        }
        // No reply on success: the state frame carries the light.
        if let Err(why) = mt.recorder(&session, &node, action == "clear", on) {
            refused(&ctx, &session, &why);
        }
    });
    Ok(())
}

fn record_enable(r: &Request) -> Result<(), Malformed> {
    let want = flag(r, "enabled")?;
    let session = r.session.clone();
    r.ctx
        .engine
        .post(move |mt| mt.set_record_enabled(&session, want));
    Ok(())
}

/// A socket closed: what its keyboard held down is released for it, since it never sends a note-off.
pub fn release_held(ctx: &Ctx, client: &Client) {
    for release in ctx.held_notes.closed(client.id) {
        ctx.engine.post(move |mt| {
            for m in &release.messages {
                mt.live_midi(&release.session, &release.track, m);
            }
        });
    }
}
