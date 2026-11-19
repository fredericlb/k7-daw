//! history.rs — the routes that read a workspace's own history on the device and restore from it (`docs/files/history.md`):
//! the body or the query, the owner and the workspace checked, then the account's room answers.
//! What a reader gets wrong: a relay-synced workspace answers 409 `relay`, and the panel then asks the server's routes.

use std::path::Path;
use std::sync::Arc;

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::Method;
use hyper::header::CONTENT_TYPE;
use k7_capture::Captures;
use k7_capture::account::{Account, Ask};
use k7_capture::failure::Failure;
use k7_file_service::libraries::Home;
use k7_file_service::paths::resolve;
use k7_file_service::workspaces::names::workspace_dir;
use serde_json::Value;

use super::git::bad;
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::{Who, claim_root, workspace_name_ok};

fn refuse(f: &Failure) -> Response {
    json_answer(f.status, &f.body())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Page,
    Checkpoint,
    Diff,
    Restore,
}

impl Kind {
    pub fn of(method: &Method, path: &str) -> Option<Kind> {
        let (get, post) = (method == Method::GET, method == Method::POST);
        match path {
            "/history/page" if get => Some(Kind::Page),
            "/history/checkpoint" if post => Some(Kind::Checkpoint),
            "/history/diff" if get => Some(Kind::Diff),
            "/history/restore" if post => Some(Kind::Restore),
            _ => None,
        }
    }
}

/// The hook: one of the four routes answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(kind) = Kind::of(req.method(), path) else {
        return Err(req);
    };
    let home = Home::of(&who.held.account.env);
    let account = Captures::global().account(&home.studio);
    let owner = || claim_root(&who.held.account.home, who.user);
    Ok(hook(account.as_ref(), who.user, &home, kind, owner, req).await)
}

/// The hook past the account: the body, then the owner, then the account's capture, then the room.
pub async fn hook(
    account: Option<&Arc<Account>>,
    user: &str,
    home: &Home,
    kind: Kind,
    owner: impl FnOnce() -> bool,
    req: Request,
) -> Response {
    let (parts, payload) = req.into_parts();
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => return bad("the request is too large"),
    };
    let ctype = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let parsed = match body::json(ctype, &bytes) {
        Ok(b) => b,
        Err(e) => return bad(&e),
    };
    if !owner() {
        return refuse(&Failure::new(
            403,
            "forbidden",
            "these workspaces belong to another account",
        ));
    }
    let Some(acct) = account else {
        return refuse(&Failure::new(
            501,
            "not-built",
            "history needs the live capture, which is off on this host",
        ));
    };
    let query = parts.uri.query().unwrap_or("");
    let q = |k: &str| {
        let v = body::query(query, k);
        (!v.is_empty()).then(|| Value::String(v))
    };
    let name = match kind {
        Kind::Page | Kind::Diff => body::query(query, "workspace"),
        _ => parsed
            .get("workspace")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    };
    if name.is_empty() {
        return bad("workspace is required");
    }
    if !workspace_name_ok(&name) {
        return bad(&format!("invalid workspace name: {name}"));
    }
    let root = resolve(&format!("{}/{name}", workspace_dir(home)), "");
    if !Path::new(&root).is_dir() {
        return refuse(&Failure::new(
            404,
            "unknown-workspace",
            format!("unknown workspace: {name}"),
        ));
    }
    if let Err(e) = acct.attach(&root, &name).await {
        return refuse(&Failure::new(500, "internal", e));
    }
    let ask = match kind {
        Kind::Page => Ask::Page {
            before: q("before"),
            limit: q("limit"),
        },
        Kind::Diff => Ask::Diff {
            from: q("from"),
            to: q("to"),
        },
        Kind::Checkpoint => Ask::Checkpoint {
            body: parsed,
            by: user.to_string(),
        },
        Kind::Restore => Ask::Restore { body: parsed },
    };
    match acct.history_ask(&root, ask).await {
        Ok(v) => json_answer(200, &v),
        Err(f) => refuse(&f),
    }
}
