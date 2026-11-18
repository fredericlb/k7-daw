//! prefs_routes.rs — `GET /preferences` and `PUT /preferences/vim` answered natively where `entries` answers (F2).
//! What a reader gets wrong: a file that does not read is a 500 for the write, as node's `wrap` words it, and the owner gate stands after the body is read.

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::Method;
use hyper::header::CONTENT_TYPE;
use k7_file_service::libraries::Home;
use serde_json::json;

use crate::routes::files::json_answer;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::{Who, claim_root};

#[derive(PartialEq)]
enum Which {
    Get,
    Vim,
}

/// Whether the route is one this file answers.
pub(super) fn owns(method: &Method, path: &str) -> bool {
    which(method, path).is_some()
}

fn which(method: &Method, path: &str) -> Option<Which> {
    match (method, path) {
        (&Method::GET, "/preferences") => Some(Which::Get),
        (&Method::PUT, "/preferences/vim") => Some(Which::Vim),
        _ => None,
    }
}

/// The hook for the two routes: answered here, or the request handed back whole.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(which) = which(req.method(), path) else {
        return Err(req);
    };
    let account = &who.held.account;
    let home = Home::of(&account.env, who.admin);
    if which == Which::Get {
        let view =
            tokio::task::spawn_blocking(move || k7_file_service::preferences::view(&home)).await;
        return match view {
            Ok(v) => Ok(json_answer(200, &v)),
            Err(_) => Err(req),
        };
    }
    let (parts, payload) = req.into_parts();
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return Ok(json_answer(
                413,
                &json!({ "error": "request entity too large" }),
            ));
        }
    };
    let content_type = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let parsed = match body::json(content_type, &bytes) {
        Ok(b) => b,
        Err(e) => return Ok(json_answer(400, &json!({ "error": e }))),
    };
    if !claim_root(&account.home, who.user) {
        return Ok(json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account" }),
        ));
    }
    if let Err(m) = body::check(&parsed, &[("vim", Field::Bool)]) {
        return Ok(json_answer(400, &json!({ "error": m })));
    }
    let on = parsed["vim"].as_bool().unwrap_or(true);
    let log = who.log.clone();
    let done = tokio::task::spawn_blocking(move || {
        k7_file_service::preferences::update(&home, |p| p.editor_vim = on)
    })
    .await;
    Ok(match done {
        Ok(Ok(p)) => json_answer(200, &json!({ "vim": p.editor_vim })),
        Ok(Err(e)) => {
            log(&format!("PUT /preferences/vim failed: {e}"));
            json_answer(500, &json!({ "error": "internal error" }))
        }
        Err(_) => json_answer(500, &json!({ "error": "internal error" })),
    })
}
