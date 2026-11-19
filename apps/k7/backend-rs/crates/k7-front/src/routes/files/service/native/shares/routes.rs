//! routes.rs — the routes of the shares piece: `/export` and `/export/tracks` (2510 to 2562).
//! What a reader gets wrong: a decline hands the request back WHOLE and before any render; once a render ran, the answer is this side's.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use k7_file_service::libraries::Home;
use k7_file_service::shares::names::{check_timeline, timeline_ask, wants_mp3};
use k7_file_service::space::Space;
use serde_json::json;

use super::export::{Format, export_mix};
use super::picture::Chromium;
use super::ports::{DECLINED, JobsMixdown, RealViews};
use super::tracks;
use super::{Account, Ports, Shares, registry};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::Tag;
use crate::routes::files::service::native::docs::Docs;
use crate::routes::files::service::native::engine::routes::{Engine, declined};
use crate::routes::files::service::native::entries::blocking;
use crate::routes::files::service::{Who, body, workspace};

/// What a route says when the workspace has no open project.
pub const NO_ACTIVE: &str = "no active project — select a project in the workspace first";

/// The tag of one of the routes.
pub fn tag_of(method: &hyper::Method, path: &str) -> Option<Tag> {
    match (method, path) {
        (&hyper::Method::GET, "/export") => Some(Tag::Export),
        (&hyper::Method::GET, "/export/tracks") => Some(Tag::ExportTracks),
        _ => None,
    }
}

/// What a route works from, built once the piece is known to be on and the account to have what it needs.
pub(super) struct Deps {
    pub(super) acct: Arc<Account>,
    pub(super) ports: Arc<Ports>,
    pub(super) shares: Arc<Shares>,
    pub(super) root: String,
    pub(super) home: Home,
}

/// The routes: answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn hook(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(tag) = tag_of(req.method(), path) else {
        return Err(req);
    };
    let Some(engine) = who.held.engine.clone() else {
        return Err(req);
    };
    let (parts, payload) = req.into_parts();
    let Some(bytes) = read(payload).await else {
        return Ok(json_answer(
            413,
            &json!({ "error": "request entity too large" }),
        ));
    };
    let ctype = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let rebuilt = |parts, bytes: Bytes| Request::from_parts(parts, Body::from(bytes));
    // node's router parsed the body before its owner gate: a bad body is its 400.
    if let Err(e) = body::json(ctype, &bytes) {
        return Ok(json_answer(400, &json!({ "error": e })));
    }
    let (_, root) = match workspace(who, &parts.headers) {
        Ok(w) => w,
        Err(r) => return Ok(*r),
    };
    let Some(d) = deps(who, &engine, &root) else {
        declined(who, tag, "no-shares-parts");
        return Err(rebuilt(parts, bytes));
    };
    let query = parts.uri.query().unwrap_or("").to_owned();
    let said = match tag {
        Tag::Export => export(&d, &query).await,
        _ => export_tracks_route(&d, &query).await,
    };
    match said {
        Some(r) => Ok(r),
        None => {
            d.shares.counts.declines.fetch_add(1, Ordering::Relaxed);
            declined(who, tag, "compile-or-body");
            Err(rebuilt(parts, bytes))
        }
    }
}

/// The body as node's router read it: whole, up to the JSON limit, or `None` for one too large.
async fn read(payload: Body) -> Option<Bytes> {
    let whole = http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await;
    whole.ok().map(|b| b.to_bytes())
}

fn deps(who: &Who<'_>, engine: &Arc<Engine>, root: &Path) -> Option<Deps> {
    let jobs = engine.jobs.get()?.clone();
    let code_root = who.code_root?;
    let account = &who.held.account;
    let env = &account.env;
    let home = Home::of(env);
    let docs = Docs::of(&who.held.acct, code_root).with_thawed(jobs.thawed.clone());
    let tmp = ["TMPDIR", "TEMP", "TMP"]
        .iter()
        .find_map(|k| env.get(*k).filter(|v| !v.is_empty()))
        .map_or_else(std::env::temp_dir, PathBuf::from);
    let acct = Arc::new(Account {
        root: root.to_path_buf(),
        tmp,
        env: env.clone(),
    });
    let ports = Arc::new(Ports {
        mixdown: Arc::new(JobsMixdown(jobs.clone())),
        views: Arc::new(RealViews {
            docs,
            home: home.clone(),
            log: who.log.clone(),
        }),
        browser: Arc::new(Chromium),
    });
    Some(Deps {
        acct,
        ports,
        shares: registry::of(&home.studio, who.log),
        root: root.display().to_string(),
        home,
    })
}

/// The open project of the workspace, or the sentence of why there is none; `Err(())` where the front declines to say.
async fn active_of(d: &Deps) -> Result<Option<PathBuf>, ()> {
    let (home, root) = (d.home.clone(), d.root.clone());
    match blocking(move || Space::open(&home, &root).active()).await {
        Some(Ok(active)) => Ok(active.map(PathBuf::from)),
        Some(Err(_)) => Err(()),
        None => Ok(None),
    }
}

fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

/// A sentence the TypeScript spells itself, which this side hands back instead of saying.
fn handed_back(message: &str) -> bool {
    message.starts_with(DECLINED)
}

/// `GET /export`.
async fn export(d: &Deps, query: &str) -> Option<Response> {
    let ask = timeline_ask(query);
    let timeline = match check_timeline(&ask) {
        Ok(t) => t,
        Err(e) => return Some(error(400, &e)),
    };
    let dir = match active_of(d).await {
        Ok(Some(dir)) => dir,
        Ok(None) => return Some(error(400, NO_ACTIVE)),
        Err(()) => return None,
    };
    let format = if wants_mp3(query) {
        Format::Mp3
    } else {
        Format::Wav
    };
    let mixed = match export_mix(&d.acct, &d.ports, &dir, format, timeline).await {
        Ok(m) => m,
        Err(e) if handed_back(&e) => return None,
        Err(e) => return Some(error(400, &e)),
    };
    d.shares.counts.exports.fetch_add(1, Ordering::Relaxed);
    match mixed.into_response().await {
        Ok(r) => Some(r),
        Err(e) => Some(error(400, &e)),
    }
}

/// `GET /export/tracks`: the same gates as `/export`, the project's tracks as a zip.
pub(super) async fn export_tracks_route(d: &Deps, query: &str) -> Option<Response> {
    let token = tracks::signal::token_of(query);
    let said = match export_tracks_answer(d, query).await? {
        Ok(r) => (r, None),
        Err(e) => (error(400, &e), Some(e)),
    };
    Some(match token {
        Some(t) => tracks::signal::mark(said.0, &t, said.1.as_deref()),
        None => said.0,
    })
}

/// The zip, or the sentence a refusal says; `None` where the TypeScript answers.
async fn export_tracks_answer(d: &Deps, query: &str) -> Option<Result<Response, String>> {
    let ask = timeline_ask(query);
    let timeline = match check_timeline(&ask) {
        Ok(t) => t,
        Err(e) => return Some(Err(e)),
    };
    let dir = match active_of(d).await {
        Ok(Some(dir)) => dir,
        Ok(None) => return Some(Err(NO_ACTIVE.to_owned())),
        Err(()) => return None,
    };
    let packed = match tracks::export_tracks(&d.acct, &d.ports, &dir, timeline).await {
        Ok(m) => m,
        Err(e) if handed_back(&e) => return None,
        Err(e) => return Some(Err(e)),
    };
    d.shares.counts.exports.fetch_add(1, Ordering::Relaxed);
    Some(packed.into_response().await)
}
