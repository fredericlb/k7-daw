//! project.rs — `project`, `tracks` and `flush`: a document parsed and checked on the socket's task, then applied
//! on the message thread unless a newer one overtook it in the queue (`supersede`).
//! What a reader gets wrong: `busy` and `applied` are sent by the apply that ran, never by one that was
//! skipped, since the document that overtook it answers for both.

use super::ctx::Request;
use super::handlers::HandlerTable;
use super::msg::Malformed;
use super::supersede::superseded;
use crate::doc::{Checks, Parsed, parse_project, parse_tracks};
use crate::log::say;
use crate::wire::json;
use serde_json::{Value, json};
use std::time::Instant;

/// The one-line summaries of a long changeset are capped so one apply cannot bury the log.
const MAX_LOGGED_CHANGES: usize = 12;

pub fn add(t: &mut HandlerTable) {
    t.add(&["project"], project);
    t.add(&["tracks"], tracks);
    t.add(&["flush"], flush);
}

/// An `error` to the socket alone, as a parse failure is told.
fn refuse(r: &Request, what: &str, why: &Malformed) {
    r.client
        .send(&json!({"type": "error", "message": format!("{what}: {}", why.0)}));
}

fn project(r: &Request) -> Result<(), Malformed> {
    let checks = Checks::of(&r.ctx.engine);
    let parsed = match parse_project(&r.msg.0, &r.session, &checks) {
        Ok(p) => p,
        Err(e) => {
            refuse(r, "bad project", &e);
            return Ok(());
        }
    };
    let doc = json::dump(&parsed.doc);
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    let Parsed {
        name,
        tempo,
        tracks,
        clips,
        plugins,
        ..
    } = parsed;
    r.ctx.supersede.post_project(&r.session, |ticket| {
        r.ctx.engine.post(move |mt| {
            if superseded(&ticket) {
                say!("[project] '{name}' superseded before it ran — skipped");
                return;
            }
            say!(
                "[project] '{name}' {tempo}bpm {tracks} track(s) {clips} clip(s) {plugins} plugin(s)"
            );
            // A project opened on this engine takes it over: the slot's chain stops, and the supervisor sees it gone.
            if session != crate::doc::SLOT_SESSION {
                mt.release_slot();
            }
            ctx.broadcast_to(&session, json!({"type": "busy", "busy": true}));
            let started = Instant::now();
            let changes = match mt.apply_project(&session, &doc) {
                Ok(changes) => changes,
                Err(why) => {
                    say!("[project] apply failed: {why}");
                    ctx.broadcast_to(
                        &session,
                        json!({"type": "error", "message": format!("apply failed: {why}")}),
                    );
                    ctx.broadcast_to(&session, json!({"type": "busy", "busy": false}));
                    return;
                }
            };
            let standing: Vec<Value> = ctx
                .engine
                .standing_warnings(&session)
                .into_iter()
                .map(|(subject, message)| json!({"subject": subject, "message": message}))
                .collect();
            ctx.broadcast_to(
                &session,
                json!({"type": "applied", "changes": changes, "warnings": standing}),
            );
            log_changes(&changes, started.elapsed().as_secs_f64() * 1000.0);
            ctx.broadcast_to(&session, json!({"type": "busy", "busy": false}));
        });
    });
    Ok(())
}

/// The diff the engine ran, in the log as well as at whichever client is listening.
fn log_changes(changes: &[String], ms: f64) {
    if changes.is_empty() {
        say!("[applied] no changes (project already current)");
        return;
    }
    say!("[applied] {} change(s) in {ms}ms", changes.len());
    for c in changes.iter().take(MAX_LOGGED_CHANGES) {
        say!("    {c}");
    }
    if changes.len() > MAX_LOGGED_CHANGES {
        say!("    … and {} more", changes.len() - MAX_LOGGED_CHANGES);
    }
}

fn tracks(r: &Request) -> Result<(), Malformed> {
    let checks = Checks::of(&r.ctx.engine);
    let parsed = match parse_tracks(&r.msg.0, &checks) {
        Ok(t) => t,
        Err(e) => {
            refuse(r, "bad tracks", &e);
            return Ok(());
        }
    };
    let ids: Vec<String> = parsed.iter().map(track_id).collect();
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.supersede.post_tracks(&r.session, ids, |ticket| {
        r.ctx.engine.post(move |mt| {
            if superseded(&ticket) {
                return;
            }
            // REPLACE these tracks in the document the session holds, then apply as a project does.
            let held = mt.track_ids(&session);
            if held.is_empty() {
                ctx.broadcast_to(
                    &session,
                    json!({"type": "error", "message": "tracks: this session holds no project"}),
                );
                return;
            }
            let (plan, unknown) = replacements(&held, parsed);
            if unknown > 0 {
                ctx.engine.warn(
                    &session,
                    "tracks",
                    &format!(
                        "{unknown} track(s) named in an update this project does not hold — the whole document has to be re-sent"
                    ),
                );
            }
            // No `busy` and no `applied`: this path is incremental, and six announcements a second would drown the changeset.
            if let Err(why) = mt.apply_tracks(&session, &json::dump(&plan)) {
                say!("[tracks] apply failed: {why}");
            }
        });
    });
    Ok(())
}

fn track_id(track: &Value) -> String {
    track["id"].as_str().unwrap_or_default().to_owned()
}

/// Where each delta track goes in the document, `[{index, track}]`, and how many it holds none for.
fn replacements(held: &[String], tracks: Vec<Value>) -> (Value, usize) {
    let mut plan = Vec::new();
    let mut unknown = 0;
    for track in tracks {
        match held.iter().position(|id| *id == track_id(&track)) {
            Some(index) => plan.push(json!({"index": index, "track": track})),
            None => unknown += 1,
        }
    }
    (Value::Array(plan), unknown)
}

fn flush(r: &Request) -> Result<(), Malformed> {
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    // Forget the project when the client switches projects; the engine stays up with its audio device and scan cache.
    r.ctx.engine.post(move |mt| {
        mt.reset_project(&session);
        ctx.broadcast_to(&session, json!({"type": "flushed"}));
    });
    Ok(())
}
