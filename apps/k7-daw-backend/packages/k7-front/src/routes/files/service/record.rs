//! record.rs — `GET /record/envelope`: a desk record's envelope, transients and tempo, read off a workspace audio file
//! as `/media` resolves it, from its sidecar or decoded once (apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/setList.md).
//! What a reader gets wrong: the workspace is the request's header, the file the query's `path`, as every JSON route.

use axum::extract::Request;
use axum::response::Response;
use serde_json::{Value, json};

use super::native::media::direct::{check, envelope_json, module};
use super::{Who, body};
use crate::routes::files::json_answer;

fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

/// The record's envelope: a matching sidecar's, else a decode at the threshold it carried or the default.
pub async fn envelope(who: &Who<'_>, query: &str, req: Request) -> Response {
    let ws = req
        .headers()
        .get("x-studio-workspace")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let path = body::query(query, "path");
    if ws.is_empty() || path.is_empty() {
        return error(400, "workspace and path required");
    }
    if !k7_mediatek::media::is_audio(&path) {
        return error(400, &format!("{path}: not an audio file"));
    }
    let abs = match super::stream::workspace_file(who, &ws, query).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let made = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let checked = check(&abs, None);
        if let Some(env) = checked.get("env") {
            return Ok(env.clone());
        }
        let wanted = checked
            .get("sensitivity")
            .and_then(Value::as_f64)
            .unwrap_or(k7_mediatek::sidecar::DEFAULT_SENSITIVITY);
        // The process's one compiled module, as the accounts' analyses use: compiling it per call costs seconds.
        let wf = module(None)?;
        k7_mediatek::analyse::analyse_file(&wf, &abs, wanted)
            .map(|e| envelope_json(&e))
            .map_err(|e| e.message().to_string())
    })
    .await;
    match made {
        Ok(Ok(v)) => json_answer(200, &v),
        Ok(Err(m)) => error(500, &m),
        Err(e) => error(500, &e.to_string()),
    }
}
