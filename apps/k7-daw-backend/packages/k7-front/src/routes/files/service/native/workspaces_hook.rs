//! workspaces_hook.rs — the hook for the home page's six routes: the body parses, the owner
//! is checked, then `workspaces::serve` answers or the request goes back to be refused (1605, 1606).
//! What a reader gets wrong: node's router parsed the body BEFORE its owner gate, so a bad body is answered first.

use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use k7_file_service::libraries::Home;

use super::workspaces::{Acct, Kind, error, serve};
use crate::routes::files::service::body;
use crate::routes::files::service::{Who, claim_root};

/// The hook: one of the six routes answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(kind) = Kind::of(req.method(), path) else {
        return Err(req);
    };
    let a = acct(who);
    let owner = || claim_root(&who.held.account.home, who.user);
    let done = hook(&a, kind, owner, req).await;
    // A workspace made, renamed or removed brings the roaming list up to this device.
    let wrote = !matches!(kind, Kind::List | Kind::Projects | Kind::Surfaces);
    if wrote && done.as_ref().is_ok_and(|r| r.status().is_success()) {
        let workspaces = k7_file_service::workspaces::names::workspace_dir(&a.home);
        super::git_roam::sweep_later(workspaces, a.home.account_home());
    }
    done
}

/// The hook past the account: the body, the owner, then the route (1606).
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn hook(
    a: &Acct,
    kind: Kind,
    owner: impl FnOnce() -> bool,
    req: Request,
) -> Result<Response, Request> {
    // node's router parsed the body before its owner gate, so a body that does not parse is answered first.
    let (parts, body) = req.into_parts();
    let bytes = match http_body_util::Limited::new(body, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => return Ok(error(413, "request entity too large")),
    };
    let content_type = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let parsed = match body::json(content_type, &bytes) {
        Ok(b) => b,
        Err(e) => return Ok(error(400, &e)),
    };
    if !owner() {
        return Ok(error(403, "these workspaces belong to another account"));
    }
    let acts = !matches!(kind, Kind::List | Kind::Projects | Kind::Surfaces);
    if !acts {
        return match serve(a, kind, &parsed).await {
            Some(r) => Ok(r),
            None => Err(Request::from_parts(parts, Body::from(bytes))),
        };
    }
    // The act runs in a task of its own: a client that hung up leaves it to finish.
    let (own, ask) = (a.clone(), parsed.clone());
    let task = super::refuse::spawn(async move { serve(&own, kind, &ask).await });
    match task.await {
        Ok(Some(r)) => Ok(r),
        Ok(None) => Err(Request::from_parts(parts, Body::from(bytes))),
        Err(_) => Ok(error(500, "internal error")),
    }
}

/// What the piece needs of the request's account.
fn acct(who: &Who<'_>) -> Acct {
    let account = &who.held.account;
    let code = who
        .code_root
        .map_or_else(String::new, |r| r.display().to_string());
    Acct {
        runtime: who.held.runtime.clone(),
        home: Home::of(&account.env),
        account: account.slug.clone(),
        log: who.log.clone(),
        demos_from: (account.env.get("STUDIO_DEMOS").cloned(), code),
        gate: super::answering::global(),
        engine: who.held.engine.clone(),
    }
}
