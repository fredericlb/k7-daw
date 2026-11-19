//! service — every account's file routes answered natively in the front: the compile, the files, the verbs, the
//! journal, the capture, the engine link, the media, the renders, exports and model downloads, with no node service
//! started, ever: a route the table lacks gets node's 404, a piece that declines its refusal (`routes.rs`).
//! What a reader gets wrong: without a code root the account is refused, not handed to node — docs/service.md.

mod accounts;
pub mod body;
pub mod internal;
pub mod libraries;
mod live;
mod machine;
pub mod native;
mod owner;
mod record;
mod routes;
mod services;
mod stream;
mod switch;
#[cfg(test)]
mod tests_owner;
#[cfg(test)]
mod tests_switches;
mod window;

use std::path::Path;
use std::sync::Arc;

use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use hyper::StatusCode;
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use k7_hub::account::{Account, Names};
use serde_json::json;

pub use self::accounts::Account as Acct;
use self::routes::{Call, Route};
pub use self::services::{FRONT_CAPTURE, Held, Services};
pub use self::switch::code_root;
pub use self::window::Window;
use super::json_answer;
pub(crate) use owner::claim_root;

/// A request the front does not take (no code root), for the hub's refusal.
pub struct Pass;

/// What a route reads: the account, the hub's stamp.
pub struct Who<'a> {
    pub held: &'a Held,
    pub user: &'a str,
    /// Whether the front is a device's own (`STUDIO_LOCAL_USER`), which the git routes need.
    pub device: bool,
    pub log: &'a k7_hub::Log,
    /// Where `packages/` and `demos/` are read, which the bundle's demos folder is looked for from.
    pub code_root: Option<&'a Path>,
}

impl Who<'_> {
    fn account(&self) -> &Account {
        &self.held.account
    }
}

/// Whether a client's path names the node service's `/front/*`, in any case and any percent-encoding, as Express matches it.
pub fn names_front(path: &str) -> bool {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(h), Some(l)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    let decoded = String::from_utf8_lossy(&out).to_ascii_lowercase();
    let mut squeezed = String::with_capacity(decoded.len());
    for c in decoded.chars() {
        if !(c == '/' && squeezed.ends_with('/')) {
            squeezed.push(c);
        }
    }
    under(&squeezed, "/front")
}

fn under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// `assertWorkspaceName`: one ordinary path segment, by allow-list.
fn workspace_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ' ' | '-'))
        && !name.chars().all(|c| c == '.')
}

fn text(status: StatusCode, body: &str) -> Response {
    (status, body.to_string()).into_response()
}

/// Whether the front takes the account's routes: a code root is there.
pub fn takes(files: &super::Files, user: &str) -> bool {
    files.service.covers(user)
}

/// `requireRootOwner` then `resolveWorkspace`: the workspace a request's header names, and its root, or the refusal.
fn workspace(
    who: &Who<'_>,
    headers: &hyper::HeaderMap,
) -> Result<(String, std::path::PathBuf), Box<Response>> {
    if !claim_root(&who.account().home, who.user) {
        return Err(Box::new(json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account" }),
        )));
    }
    let Some(name) = headers
        .get("x-studio-workspace")
        .and_then(|v| v.to_str().ok())
        .filter(|n| !n.is_empty())
    else {
        return Err(Box::new(json_answer(
            400,
            &json!({ "error": "missing workspace header" }),
        )));
    };
    if !workspace_name_ok(name) {
        return Err(Box::new(json_answer(
            400,
            &json!({ "error": format!("invalid workspace name: {name}") }),
        )));
    }
    let root = who.account().home.join("workspace").join(name);
    if !root.is_dir() {
        return Err(Box::new(json_answer(
            404,
            &json!({ "error": format!("unknown workspace: {name}") }),
        )));
    }
    Ok((name.to_string(), root))
}

/// A route the table does not name, as node's file service answered it: its owner and workspace checks, then Express's 404.
fn unnamed(who: &Who<'_>, req: &Request, path: &str) -> Response {
    // node's liveness, mounted before its gates, answered whoever asks.
    if req.method() == hyper::Method::GET && path == "/health" {
        return json_answer(200, &json!({ "ok": true }));
    }
    match workspace(who, req.headers()) {
        Err(r) => *r,
        Ok(_) => crate::routes::last::express::not_found(req.method(), path, false),
    }
}

/// A hooked request no hook answered: refused where a piece owns the route and declined, else node's 404 (4-7).
fn fallen_through(who: &Who<'_>, req: &Request, path: &str) -> Response {
    if native::refuse::owned(req.method(), path) {
        return native::refuse::refused();
    }
    unnamed(who, req, path)
}

/// The routes a link reaches, which carry no header: `/export/tracks` names its workspace in the query (docs/tracks-export.md).
fn lend_workspace(req: &mut Request, path: &str, query: &str) {
    if path != "/export/tracks" || req.headers().contains_key("x-studio-workspace") {
        return;
    }
    // A page on another site can put the link in an image, so only the app's own navigation lends it.
    let site = req
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok());
    if site.is_some_and(|s| s != "same-origin" && s != "none") {
        return;
    }
    let name = body::query(query, "workspace");
    if let Some(v) = Some(name)
        .filter(|n| !n.is_empty())
        .and_then(|n| hyper::header::HeaderValue::from_str(&n).ok())
    {
        req.headers_mut().insert("x-studio-workspace", v);
    }
}

/// The account's request answered natively, or handed back where the front does not take it.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn answer(
    files: &super::Files,
    names: &Arc<Names>,
    user: &str,
    target: &str,
    req: Request,
) -> Result<Response, Pass> {
    // The place a native decline notes its reason, for the refusal that follows (4-7).
    native::refuse::scoped(answer_scoped(files, names, user, target, req)).await
}

/// `getWorkspace`'s test: a name with no folder is not a workspace, and naming it never makes one.
async fn is_workspace(home: &k7_file_service::libraries::Home, name: &str) -> bool {
    let root = format!(
        "{}/{name}",
        k7_file_service::workspaces::names::workspace_dir(home)
    );
    tokio::fs::metadata(&root).await.is_ok_and(|m| m.is_dir())
}

#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
async fn answer_scoped(
    files: &super::Files,
    names: &Arc<Names>,
    user: &str,
    target: &str,
    mut req: Request,
) -> Result<Response, Pass> {
    let svc = &files.service;
    if !takes(files, user) {
        return Err(Pass);
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    lend_workspace(&mut req, path, query);
    // The node service's half of the bargain is the hub's alone to ask.
    if names_front(path) {
        return Ok(text(StatusCode::NOT_FOUND, "Not Found"));
    }
    // The account is made at its first request: its capture starts then.
    let held = match svc.held(user, names).await {
        Ok(h) => h,
        Err(e) => {
            return Ok(json_answer(
                503,
                &json!({ "error": format!("file service: {e}") }),
            ));
        }
    };
    let who = Who {
        held: &held,
        user,
        device: files.hub.is_device(),
        log: &svc.log,
        code_root: svc.code_root.as_deref(),
    };
    let ws = req
        .headers()
        .get("x-studio-workspace")
        .and_then(|v| v.to_str().ok())
        .filter(|w| workspace_name_ok(w))
        .map(str::to_string);
    // The account's binding (or none) is told to its engine link first.
    svc.bind(
        &held,
        files.hub.bindings.get(user).as_deref(),
        &files.hub.seam,
    )
    .await;
    // A workspace a request names is captured from then on, whoever answers it, as node's resolveWorkspace does.
    if let Some(w) = &ws {
        let key = (who.account().slug.clone(), w.clone());
        let first = svc
            .joined
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(key.clone());
        if first {
            let home = k7_file_service::libraries::Home::of(&held.account.env);
            let there = is_workspace(&home, w).await;
            match k7_capture::Captures::global()
                .account(&home.studio)
                .filter(|_| there)
            {
                Some(acct) => acct.attach_named(w).await,
                None => {
                    svc.joined
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .remove(&key);
                }
            }
        }
    }
    let route = routes::route(req.method(), path);
    let handler = match route {
        Route::Hooked => {
            // A base snapshot files its copy inside the workspace: a reader's refuses it like any write.
            if req.method() == hyper::Method::POST
                && path == "/module-base/snapshot"
                && let Ok((_, root)) = workspace(&who, req.headers())
                && let Some(refused) = native::git::merge_refusal(&root.display().to_string())
            {
                return Ok(refused);
            }
            // A removed workspace made again under its name must be joined again.
            if path == "/workspaces/remove" {
                // Windows holds a watched folder open (1323).
                let workspaces = who.account().home.join("workspace");
                k7_file_service::revisions::Revisions::global()
                    .forget_under(&workspaces.display().to_string());
                svc.joined
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .retain(|(slug, _)| slug != &who.account().slug);
            }
            // The bench made and a chain compiled into a slot, where `chains` answers (3j M5).
            let req = match native::bench::live(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The preferences, where `entries` answers (F2).
            let req = match native::prefs_routes::live(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The account's chain lists (1p-D6).
            if let Some(c) = native::chains::live_ctx(&who, req.method(), path, req.headers()) {
                return Ok(native::chains::live(&who, path, req, c).await);
            }
            if let Some(done) =
                native::runtime::logs::live(&who, req.method(), req.headers(), path, query).await
            {
                return Ok(done);
            }
            if path == "/module-libraries"
                && req.method() == hyper::Method::GET
                && let Some(c) = native::listings::live_ctx(&who, req.headers(), query)
            {
                return Ok(native::listings::live(&who, c).await);
            }
            // The poll and the engine's four routes (2226).
            let req = match native::engine::routes::live_hook(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The mixdown and the tracks export (2510).
            let req = match native::shares::routes::hook(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The home page's six routes (1606).
            let req = match native::workspaces_hook::live(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The routes that put a workspace under git.
            let req = match native::git::live(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The routes that read a workspace's own history on this device.
            let req = match native::history::live(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The three POSTs that move the open project (2000).
            let req = match native::projects::live(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            // The waveform routes (2406).
            let req = match native::media::routes::live_hook(&who, path, req).await {
                Ok(r) => return Ok(r),
                Err(req) => req,
            };
            return Ok(fallen_through(&who, &req, path));
        }
        Route::Stream(k) => return Ok(stream::answer(k, &who, query, req).await),
        Route::Handler(h) => h,
    };
    let (parts, body) = req.into_parts();
    let declared = parts
        .headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    let limit = routes::body_limit(handler);
    let too_large = || json_answer(413, &json!({ "error": "request entity too large" }));
    if declared.is_some_and(|n| n > limit) {
        drain(body).await;
        return Ok(too_large());
    }
    let Some(bytes) = read_capped(body, limit).await else {
        return Ok(too_large());
    };
    let content_type = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let body = match body::json(content_type, &bytes) {
        Ok(b) => b,
        Err(e) => return Ok(json_answer(400, &json!({ "error": e }))),
    };
    let (name, root) = match workspace(&who, &parts.headers) {
        Ok(w) => w,
        Err(r) => return Ok(*r),
    };
    let root = root.display().to_string();
    if routes::writes(handler)
        && let Some(refused) = native::git::merge_refusal(&root)
    {
        return Ok(refused);
    }
    let req = native::Req {
        env: &who.account().env,
        slug: &who.account().slug,
        workspace: &name,
        root: &root,
        query,
        body: &body,
        log: &svc.log,
        docs: svc.code_root().map(|r| (&who.held.acct, r)),
        thawed: who
            .held
            .engine
            .as_ref()
            .and_then(|e| e.jobs.get())
            .map(|j| &j.thawed),
    };
    let mut native = native::ctx_for(native::Tag::of(handler), &req);
    if let Some(ctx) = native
        .as_mut()
        .filter(|c| matches!(c.tag, native::Tag::VerbHost(_)))
    {
        let jobs = who.held.engine.as_ref().and_then(|e| e.jobs.get()).cloned();
        let runtime = who
            .held
            .engine
            .as_ref()
            .and_then(|e| e.runtime.get())
            .cloned();
        ctx.verbs = native::verb_host::production_in(jobs, runtime);
        let link = who.held.engine.as_ref().map(|e| e.link.clone());
        ctx.verbs = native::verb_host::account::with(ctx.verbs.take(), link);
    }
    let call = Call {
        runtime: held.runtime.clone(),
        workspace: name,
        root,
        query: query.to_string(),
        body,
        log: svc.log.clone(),
        native,
        media: held.media.clone(),
        engine: held.engine.clone(),
    };
    let mut answer = routes::dispatch(handler, &call).await;
    if routes::reports_compiler(handler) {
        answer.headers_mut().insert(
            "x-k7-compiler",
            HeaderValue::from_static(k7_file_service::stamp::SONG_SOURCES),
        );
    }
    Ok(answer)
}

#[cfg(test)]
#[path = "tests_mod.rs"]
mod tests;

/// How much of a refused body is read and dropped, so the 413 reaches a client still sending (docs/rust-backend-plan.md).
const DRAIN_CAP: usize = 32 << 20;

/// Reads and drops what is left of a refused body up to `DRAIN_CAP`: closing on unread data resets the connection on Windows.
async fn drain(mut body: axum::body::Body) {
    let mut seen = 0usize;
    while let Some(Ok(frame)) = body.frame().await {
        seen += frame.data_ref().map_or(0, |d| d.len());
        if seen > DRAIN_CAP {
            break;
        }
    }
}

/// The whole body when it fits `limit`; else `None`, the rest drained.
async fn read_capped(mut body: axum::body::Body, limit: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    while let Some(Ok(frame)) = body.frame().await {
        if let Some(d) = frame.data_ref() {
            if out.len() + d.len() > limit {
                drain(body).await;
                return None;
            }
            out.extend_from_slice(d);
        }
    }
    Some(out)
}

#[cfg(test)]
mod capped_tests {
    use super::read_capped;
    use axum::body::Body;

    #[tokio::test]
    async fn a_body_past_the_limit_is_read_to_the_end_and_refused() {
        assert_eq!(
            read_capped(Body::from(vec![1u8; 10]), 10)
                .await
                .map(|v| v.len()),
            Some(10)
        );
        assert!(read_capped(Body::from(vec![1u8; 11]), 10).await.is_none());
    }
}
