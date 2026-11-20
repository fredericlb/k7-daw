//! devices.rs — the audio interfaces the engine sees, and a pair of inputs grouped as one stereo device (3f, M9).
//! What a reader gets wrong: `heard` says a frame was queued to a live engine, never that the engine applied it.

use axum::response::Response;
use k7_file_service::verb_host::Error;
use serde_json::{Value, json};

use super::account::{QUERY_WAIT, ready};
use super::{Checked, failure};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::engine::EngineLink;
use crate::routes::files::service::native::jobs::waiters::Want;

/// `GET /verb-host/devices`: the interfaces, `[]` with no engine or none answering.
pub async fn list(req: &Checked, link: &EngineLink) -> Response {
    let Some(session) = ready(link, &req.ctx.root, QUERY_WAIT).await else {
        return json_answer(200, &json!({ "devices": [] }));
    };
    let waiting = link.0.waiters.wait(Want::Devices, "");
    if !session.frame(json!({ "type": "list-audio-devices" })) {
        return json_answer(200, &json!({ "devices": [] }));
    }
    let devices = match waiting
        .answer()
        .await
        .and_then(|f| f.get("devices").cloned())
    {
        Some(d @ Value::Array(_)) => d,
        _ => json!([]),
    };
    json_answer(200, &json!({ "devices": devices }))
}

/// `POST /verb-host/devices/stereo {channel, stereo}`: the engine makes the pair now.
pub async fn stereo(req: &Checked, link: &EngineLink) -> Response {
    let channel = req.body.get("channel").and_then(Value::as_u64);
    let on = req.body.get("stereo").and_then(Value::as_bool);
    let (Some(channel), Some(on)) = (channel.filter(|c| (1..=1024).contains(c)), on) else {
        return failure(&Error::new(
            400,
            "channel must be a number from 1 to 1024 and stereo a boolean",
        ));
    };
    let Some(session) = ready(link, &req.ctx.root, QUERY_WAIT).await else {
        return json_answer(200, &json!({ "heard": false }));
    };
    let heard =
        session.frame(json!({ "type": "set-input-stereo", "channel": channel, "stereo": on }));
    json_answer(200, &json!({ "heard": heard }))
}
