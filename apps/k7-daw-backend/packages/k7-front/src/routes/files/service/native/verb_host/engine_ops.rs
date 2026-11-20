//! engine_ops.rs — `POST /verb-host/engine {op}`: transport, arm, the takes view's arm, a timeline's audition, a plugin's window (3f-1, M9).
//! What a reader gets wrong: `takeArm` and `timelinePlay` only set the session's state and have the served document sent again; the engine is not asked.

use std::time::Duration;

use axum::response::Response;
use k7_file_service::engine::docs::TakeArm;
use k7_file_service::verb_host::Error;
use serde_json::{Value, json};

use super::account::{QUERY_WAIT, ready};
use super::{Checked, failure};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::engine::EngineLink;
use crate::routes::files::service::native::jobs::waiters::Want;

const NO_ENGINE: &str = "engine offline — is the C++ engine running on this machine?";

/// How long an arm waits for a link still being dialled.
const LINK_OPEN: Duration = Duration::from_secs(5);

/// A name the wire carries: a non-empty string of a bounded length.
fn name<'a>(body: &'a Value, key: &str) -> Result<&'a str, Box<Response>> {
    match body.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s.len() <= 256 => Ok(s),
        _ => Err(Box::new(failure(&Error::new(
            400,
            format!("{key} must be a string of 1 to 256 bytes"),
        )))),
    }
}

fn bad(why: &str) -> Response {
    failure(&Error::new(400, why))
}

fn finite(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|n| n.is_finite())
}

const SHAPE: &str = "arm must be null or {track, eventBeats, punch: {from, to?}}";

/// The takes view's arm, checked; `Ok(None)` disarms.
fn take_arm(v: &Value) -> Result<Option<TakeArm>, &'static str> {
    if v.is_null() {
        return Ok(None);
    }
    let (Some(track), Some(beats), Some(punch)) = (
        v.get("track")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty() && t.len() <= 256),
        finite(v.get("eventBeats")),
        v.get("punch").filter(|p| p.is_object()),
    ) else {
        return Err(SHAPE);
    };
    let Some(from) = finite(punch.get("from")) else {
        return Err(SHAPE);
    };
    let to = match punch.get("to") {
        None | Some(Value::Null) => None,
        other => Some(finite(other).ok_or(SHAPE)?),
    };
    Ok(Some(TakeArm {
        track: track.to_string(),
        event_beats: beats,
        from,
        to,
    }))
}

pub async fn run(req: &Checked, link: &EngineLink) -> Response {
    let op = match name(&req.body, "op") {
        Ok(o) => o,
        Err(r) => return *r,
    };
    let root = req.ctx.root.as_str();
    match op {
        "transport" => {
            let action = match name(&req.body, "action") {
                Ok(a) if a.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') => a,
                _ => return bad("action must be a short word"),
            };
            let Some(s) = ready(link, root, QUERY_WAIT).await else {
                return json_answer(200, &json!({ "sent": false }));
            };
            let sent = s.frame(json!({ "type": "transport", "action": action }));
            json_answer(200, &json!({ "sent": sent }))
        }
        "arm" => {
            // A take recorded while a merge waits would be filed into files the merge is about to write.
            if req.body.get("armed").and_then(Value::as_bool) != Some(false)
                && let Some(refused) =
                    crate::routes::files::service::native::git::merge_refusal(root)
            {
                return refused;
            }
            arm(req, link).await
        }
        "takeArm" => {
            let arm = match take_arm(req.body.get("arm").unwrap_or(&Value::Null)) {
                Ok(a) => a,
                Err(why) => return bad(why),
            };
            if arm.is_some()
                && let Some(refused) =
                    crate::routes::files::service::native::git::merge_refusal(root)
            {
                return refused;
            }
            let s = link.session(Some(root));
            s.set_take_arm(arm);
            s.enqueue(|s| async move { s.resend_served().await });
            json_answer(200, &json!({ "ok": true }))
        }
        "timelinePlay" => {
            let id = match req.body.get("id") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if !s.is_empty() && s.len() <= 256 => Some(s.clone()),
                _ => return bad("id must be null or a string of 1 to 256 bytes"),
            };
            let s = link.session(Some(root));
            s.set_audition(id);
            s.enqueue(|s| async move { s.resend_served().await });
            json_answer(200, &json!({ "ok": true }))
        }
        "showUi" => {
            let id = match name(&req.body, "id") {
                Ok(i) => i,
                Err(r) => return *r,
            };
            let Some(s) = ready(link, root, QUERY_WAIT).await else {
                return json_answer(200, &json!({ "lines": [NO_ENGINE] }));
            };
            if !s.frame(json!({ "type": "show-native-plugin-ui", "id": id })) {
                return json_answer(200, &json!({ "lines": [NO_ENGINE] }));
            }
            json_answer(200, &json!({ "lines": [] }))
        }
        _ => bad("op must be transport, arm, takeArm, timelinePlay or showUi"),
    }
}

/// Arm or disarm a lane, waiting for a link being dialled and for the engine's `armed` frame of that track.
async fn arm(req: &Checked, link: &EngineLink) -> Response {
    let track = match name(&req.body, "track") {
        Ok(t) => t,
        Err(r) => return *r,
    };
    let armed = match req.body.get("armed") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => return bad("armed must be a boolean"),
    };
    let said = |lines: Vec<String>, armed: Option<bool>| {
        json_answer(200, &json!({ "armed": armed, "lines": lines }))
    };
    if link.binding().url.is_none() {
        return said(vec![NO_ENGINE.into()], None);
    }
    let Some(s) = ready(link, &req.ctx.root, LINK_OPEN).await else {
        return said(
            vec![format!(
                "the engine link is still opening after {} s — \"{track}\" was not armed; arm it again in a moment",
                LINK_OPEN.as_secs()
            )],
            None,
        );
    };
    let waiting = link.0.waiters.wait(Want::Armed, track);
    let refused = link.0.waiters.wait(Want::EngineError, "");
    let mut frame = json!({ "type": "arm", "track": track });
    if let Some(a) = armed {
        frame["armed"] = json!(a);
    }
    if !s.frame(frame) {
        return said(vec![NO_ENGINE.into()], None);
    }
    // The engine's own sentence ends the wait, as the TypeScript's `engineError` did.
    let answer = tokio::select! {
        a = waiting.answer() => a.map(Ok),
        e = refused.answer() => e.map(Err),
    };
    match answer {
        Some(Ok(f)) => said(vec![], f.get("armed").and_then(Value::as_bool)),
        Some(Err(e)) => said(
            vec![
                e["message"]
                    .as_str()
                    .unwrap_or("the engine refused the arm")
                    .to_owned(),
            ],
            None,
        ),
        None => said(
            vec![if s.is_ready() {
                format!(
                    "engine did not answer the arm of \"{track}\" in 2000ms — the link is up, so check the engine log for a [rec] line"
                )
            } else {
                format!("the engine link dropped while arming \"{track}\"")
            }],
            None,
        ),
    }
}
