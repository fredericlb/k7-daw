//! cold_set.rs — `PUT /account/cold-chains/:which`: the machine's stems or instrumental chain chosen, answered natively (F2).
//! What a reader gets wrong: the owner gate stands after the body is read, and a ref that is no string is refused.

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use serde_json::{Value, json};

use super::{What, answer_what};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::Ctx;
use crate::routes::files::service::native::reads::{self, Json};
use crate::routes::files::service::native::refuse::refused;
use crate::routes::files::service::{Who, body, claim_root};

pub(super) async fn answer(who: &Who<'_>, path: &str, req: Request, ctx: Ctx) -> Response {
    let which = path.rsplit('/').next().unwrap_or("").to_owned();
    let (parts, payload) = req.into_parts();
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => return json_answer(413, &json!({ "error": "request entity too large" })),
    };
    let content_type = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let parsed = match body::json(content_type, &bytes) {
        Ok(b) => b,
        Err(e) => return json_answer(400, &json!({ "error": e })),
    };
    if !claim_root(&who.held.account.home, who.user) {
        return json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account" }),
        );
    }
    if which != "stems" && which != "instrumental" {
        return json_answer(
            404,
            &json!({ "error": format!("no '{which}' chain setting") }),
        );
    }
    let chain_ref = match parsed.get("ref") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => k7_file_service::space::js_trim(s).to_owned(),
        Some(_) => {
            super::note_decline(&ctx, "body");
            return refused();
        }
    };
    let what = What::Set { which, chain_ref };
    match answer_what(&ctx, what).await {
        Some(d) => reads::response(d.status, Json::Value(d.body)),
        None => refused(),
    }
}
