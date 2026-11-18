//! onnx.rs — what the bound machine separates on, read or set there, over its jobs path (3f, M9).
//! What a reader gets wrong: no machine bound answers `{providers: null}`, a machine that cannot be reached is 503 `no-machine`.

use axum::response::Response;
use k7_file_service::verb_host::Error;
use serde_json::{Map, Value, json};

use super::drive::no_machine;
use super::{Checked, failure};
use crate::routes::engine_reach::job::{JobChannel, of_type, request_job};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::engine::EngineLink;
use crate::routes::files::service::native::engine::jobs::WsChannel;

/// The most provider names a set may hold, and the longest name.
const SET_MAX: usize = 32;
const NAME_MAX: usize = 128;

/// How long the machine gets to answer, in all and between two words.
const ASK: std::time::Duration = std::time::Duration::from_secs(30);

/// `POST /verb-host/onnx {set?}`.
pub async fn ask(req: &Checked, link: &EngineLink) -> Response {
    let mut body = Map::from_iter([("type".to_string(), json!("onnx-providers"))]);
    match req.body.get("set") {
        None | Some(Value::Null) => {}
        Some(Value::Array(names)) => {
            let ok = names.len() <= SET_MAX
                && names.iter().all(|n| {
                    n.as_str()
                        .is_some_and(|s| !s.is_empty() && s.len() <= NAME_MAX)
                });
            if !ok {
                return failure(&Error::new(
                    400,
                    "set must be at most 32 names of at most 128 bytes",
                ));
            }
            body.insert("set".into(), Value::Array(names.clone()));
        }
        Some(_) => return failure(&Error::new(400, "set must be a list of names")),
    }
    let Some(url) = link.jobs_url() else {
        return json_answer(200, &json!({ "providers": null }));
    };
    let Ok(mut chan) = WsChannel::open(&url).await else {
        return no_machine("the engine's machine could not be reached");
    };
    let answer = over(&mut chan, body).await;
    chan.close();
    answer
}

/// The machine's `onnx-providers` reply as the answer: its string names, `[]` when it names none; a refusal is 503 `no-machine`.
pub(super) async fn over<C: JobChannel>(chan: &mut C, body: Map<String, Value>) -> Response {
    match request_job(chan, body, of_type("onnx-providers"), |_| {}, ASK, ASK).await {
        Ok(reply) => {
            let providers: Vec<Value> = reply
                .get("providers")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter(|p| p.is_string()).cloned().collect())
                .unwrap_or_default();
            json_answer(200, &json!({ "providers": providers }))
        }
        Err(e) => no_machine(&e.0),
    }
}
