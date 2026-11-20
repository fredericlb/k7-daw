//! download.rs — `POST /account/cold-chains/download`: the cold chain found natively, each model started in 1l's registry (1p-D16).
//! What a reader gets wrong: a download twice is two, and answered only where chains and media both answer.

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use serde_json::{Value, json};

use super::super::media::Media;
use super::super::refuse::refused;
use super::{Ctx, What, answer_what};
use crate::routes::files::json_answer;
use crate::routes::files::service::body;

/// `String(body.ref ?? "").trim()`: JavaScript's white space is Rust's less U+0085, plus U+FEFF.
fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}

/// The ref the body names, or `None` where it is no string, which is refused.
fn ref_of(body: &Value) -> Option<String> {
    match body.get("ref") {
        None | Some(Value::Null) => Some(String::new()),
        Some(Value::String(s)) => Some(js_trim(s).to_owned()),
        Some(_) => None,
    }
}

/// The chain's downloads as `[(file, url)]` in its order, or the TypeScript's refusal.
pub(super) fn jobs_of(view: &Value, chain: &str) -> Result<Vec<(String, String)>, (u16, String)> {
    let found = view["chains"]
        .as_array()
        .and_then(|rows| rows.iter().find(|c| c["ref"].as_str() == Some(chain)));
    let Some(found) = found else {
        return Err((404, format!("'{chain}' is not a cold chain of the library")));
    };
    let list: Vec<(String, String)> = found["downloads"]
        .as_array()
        .map(|d| {
            d.iter()
                .filter_map(|x| {
                    Some((
                        x["file"].as_str()?.to_owned(),
                        x["url"].as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    if list.is_empty() {
        return Err((
            400,
            format!("'{chain}' needs no model this machine can download"),
        ));
    }
    Ok(list)
}

/// The context of a download, where the account's `media` is there to start it; else the decline is counted.
pub(super) fn download_ctx(ctx: Ctx, media_answers: bool) -> Option<Ctx> {
    if !media_answers {
        super::note_decline(&ctx, "media");
        return None;
    }
    Some(ctx)
}

/// The route: the body parses and the owner passes as node's middleware does, then the chain is found and its models start.
pub(super) async fn answer(
    ctx: Ctx,
    media: &Media,
    owner: &(dyn Fn() -> bool + Sync),
    req: Request,
) -> Response {
    let (parts, payload) = req.into_parts();
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => return json_answer(413, &json!({ "error": "request entity too large" })),
    };
    let ct = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let parsed = match body::json(ct, &bytes) {
        Ok(v) => v,
        Err(e) => return json_answer(400, &json!({ "error": e })),
    };
    if !owner() {
        return json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account" }),
        );
    }
    let Some(chain) = ref_of(&parsed) else {
        super::note_decline(&ctx, "body");
        return refused();
    };
    let Some(done) = answer_what(&ctx, What::Download).await else {
        return refused();
    };
    match jobs_of(&done.body, &chain) {
        Err((status, error)) => json_answer(status, &json!({ "error": error })),
        Ok(list) => {
            let jobs: Vec<Value> = list
                .iter()
                .map(|(file, url)| media.downloads.start(file, url).to_json())
                .collect();
            json_answer(200, &json!({ "jobs": jobs }))
        }
    }
}
