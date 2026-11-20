//! routes.rs — the hook for the waveform routes: `/waveform`, `/waveforms/status`, `/waveforms/prewarm` and
//! `/waveforms/cancel` answered from the account's `Media`, a decline handed back and refused (2406, 2420 to 2432).
//! What a reader gets wrong: node's router parsed a POST's body and resolved the workspace before its handler, so does this.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::{CONTENT_TYPE, HeaderMap};
use k7_file_service::media::{clamp_buckets, clamp_sensitivity, is_whole};
use k7_hub::Log;
use serde_json::{Value, json};
use song::graph::ModelNeed;

use super::Media;
use super::locate::Session;
use super::waveform::{Locator, Request as Asked, Why};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::{Ctx, Req, Tag, stats};
use crate::routes::files::service::{Who, body, claim_root, workspace};

/// The routes of this hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Waveform,
    Status,
    Prewarm,
    Cancel,
    Model,
    Download,
}

impl Kind {
    pub fn of(method: &hyper::Method, path: &str) -> Option<Kind> {
        let (get, post) = (method == hyper::Method::GET, method == hyper::Method::POST);
        match path {
            "/waveform" if get => Some(Kind::Waveform),
            "/waveforms/status" if get => Some(Kind::Status),
            "/waveforms/prewarm" if post => Some(Kind::Prewarm),
            "/waveforms/cancel" if post => Some(Kind::Cancel),
            "/models/download" if post => Some(Kind::Download),
            p if get
                && p.strip_prefix("/account/models/")
                    .is_some_and(|id| !id.is_empty() && !id.contains('/')) =>
            {
                Some(Kind::Model)
            }
            _ => None,
        }
    }

    fn tag(self) -> Tag {
        match self {
            Kind::Waveform => Tag::Waveform,
            Kind::Status => Tag::WaveformsStatus,
            Kind::Prewarm => Tag::WaveformsPrewarm,
            Kind::Cancel => Tag::WaveformsCancel,
            Kind::Model => Tag::AccountModel,
            Kind::Download => Tag::ModelsDownload,
        }
    }
}

/// `req.query[name]` as `String` makes it: absent is none, a repeated key its values joined by commas.
pub fn query_opt(q: &str, name: &str) -> Option<String> {
    let got: Vec<String> = url::form_urlencoded::parse(q.as_bytes())
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
        .collect();
    (!got.is_empty()).then(|| got.join(","))
}

fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

type Resolve<'a> = &'a (dyn Fn(&HeaderMap) -> Result<(String, String), Box<Response>> + Sync);

/// What the routes need of an account, so a test can stand in for the front's `Who`: the media, the workspace gate, the
/// owner's gate, and the locator of a workspace's project.
pub struct Hook<'a> {
    pub media: Arc<Media>,
    pub account: String,
    pub log: &'a Log,
    pub resolve: Resolve<'a>,
    pub owner: &'a (dyn Fn() -> bool + Sync),
    pub locator: &'a (dyn Fn(&str, &str, &str) -> Arc<dyn Locator> + Sync),
}

/// The hook: one of the routes answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live_hook(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(kind) = Kind::of(req.method(), path) else {
        return Err(req);
    };
    let Some(media) = who.held.media.clone() else {
        say_declined(&who.held.account.slug, kind, who.log, "media");
        return Err(req);
    };
    let account = &who.held.account;
    let locator = |name: &str, root: &str, query: &str| -> Arc<dyn Locator> {
        let rq = Req {
            env: &account.env,
            slug: &account.slug,
            workspace: name,
            root,
            query,
            body: &Value::Null,
            log: who.log,
            docs: who.code_root.map(|r| (&who.held.acct, r)),
            thawed: None,
        };
        let mut ctx = Ctx::of(Tag::ProjectDoc, &rq);
        ctx.touches = false;
        Arc::new(Session::new(ctx, media.lites.clone()))
    };
    let resolve = |h: &HeaderMap| workspace(who, h).map(|(n, r)| (n, r.display().to_string()));
    let owner = || claim_root(&who.held.account.home, who.user);
    let hook = Hook {
        media: media.clone(),
        account: account.slug.clone(),
        log: who.log,
        resolve: &resolve,
        owner: &owner,
        locator: &locator,
    };
    let done = answer(&hook, kind, path, req).await;
    media.say_if_due(who.log);
    done
}

fn say_declined(account: &str, kind: Kind, log: &Log, why: &'static str) {
    stats::global().declined_for(account, kind.tag().route(), account, log, why);
}

/// The route past the account: the body parses, the workspace resolves, then the route's own work.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn answer(
    h: &Hook<'_>,
    kind: Kind,
    path: &str,
    req: Request,
) -> Result<Response, Request> {
    let media = &h.media;
    if kind == Kind::Model {
        return model(h, path).ok_or(req);
    }
    let (parts, payload) = req.into_parts();
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => return Ok(error(413, "request entity too large")),
    };
    let ct = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    if let Err(e) = body::json(ct, &bytes) {
        return Ok(error(400, &e));
    }
    let (name, root) = match (h.resolve)(&parts.headers) {
        Ok(w) => w,
        Err(r) => return Ok(*r),
    };
    let query = parts.uri.query().unwrap_or("").to_string();
    let session = (h.locator)(&name, &root, &query);
    let hand_back = || Request::from_parts(parts, Body::from(bytes.clone()));
    match kind {
        Kind::Model => unreachable!("answered above"),
        Kind::Download => download(h, &*session, bytes.clone(), hand_back).await,
        Kind::Status => Ok(json_answer(200, &media.prewarms.status(&root).to_json())),
        Kind::Cancel => {
            let s = media.prewarms.cancel(&root, &media.waves).await;
            Ok(json_answer(200, &s.to_json()))
        }
        Kind::Prewarm => {
            let s = media
                .prewarms
                .start(&root, media.waves.clone(), session, media.counts.clone())
                .await;
            Ok(json_answer(200, &s.to_json()))
        }
        Kind::Waveform => {
            let ask = match asked(&query) {
                Ok(a) => a,
                Err(r) => return Ok(*r),
            };
            match waveform(media, &*session, ask).await {
                Ok(r) => Ok(r),
                Err(why) => {
                    media
                        .counts
                        .declines
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    say_declined(&h.account, kind, h.log, why);
                    Err(hand_back())
                }
            }
        }
    }
}

/// The route's own checks and clamps: `clip required`, then buckets, `whole` and the sensitivity.
fn asked(query: &str) -> Result<Asked, Box<Response>> {
    let clip = query_opt(query, "clip").unwrap_or_default();
    if clip.is_empty() {
        return Err(Box::new(error(400, "clip required")));
    }
    Ok(Asked {
        clip,
        buckets: clamp_buckets(query_opt(query, "buckets").as_deref()),
        whole: is_whole(query_opt(query, "whole").as_deref()),
        sensitivity: clamp_sensitivity(query_opt(query, "sensitivity").as_deref()),
    })
}

/// The answer, a refusal as the route words it, or why it declined (a `&'static str` for the log).
pub async fn waveform(
    media: &Media,
    locator: &dyn Locator,
    ask: Asked,
) -> Result<Response, &'static str> {
    let (buckets, sens) = (ask.buckets, ask.sensitivity);
    let located = match locator.locate(ask).await {
        Ok(l) => l,
        Err(Why::Decline(why)) => return Err(why),
        Err(Why::Refuse(code, m)) => return Ok(error(code, &m)),
    };
    Ok(match media.waves.answer(located, buckets, sens).await {
        Ok(text) => raw_json(200, text),
        Err((code, m)) => error(code, &m),
    })
}

fn raw_json(status: u16, text: Arc<str>) -> Response {
    let mut r = Response::new(Body::from(text.as_bytes().to_vec()));
    *r.status_mut() = hyper::StatusCode::from_u16(status).unwrap_or(hyper::StatusCode::OK);
    r.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

/// `GET /account/models/:id`: one download as it stands, behind the root owner's gate; a job it does not hold is a 404 (2440, 2446).
fn model(h: &Hook<'_>, path: &str) -> Option<Response> {
    if !(h.owner)() {
        return Some(error(403, "these workspaces belong to another account"));
    }
    let id = path.strip_prefix("/account/models/").unwrap_or("");
    // No job held is the TypeScript's own 404: nothing else starts a download now.
    Some(match h.media.downloads.job(id) {
        Some(job) => json_answer(200, &job.to_json()),
        None => error(404, "no such download"),
    })
}

/// `POST /models/download`: the node's model read off the project's graph, its download started or the TypeScript's refusal (2446).
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
async fn download(
    h: &Hook<'_>,
    session: &dyn Locator,
    bytes: axum::body::Bytes,
    hand_back: impl FnOnce() -> Request,
) -> Result<Response, Request> {
    let node = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v.get("node").and_then(Value::as_str).map(str::to_owned))
        .filter(|n| !n.is_empty());
    // A body the route's schema would refuse is worded as zod words it.
    let Some(node) = node else {
        let said = body::check(
            &serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null),
            &[("node", body::Field::Text)],
        );
        return Ok(error(
            400,
            &said.err().unwrap_or_else(|| "invalid body".into()),
        ));
    };
    let need = match session.need(node.clone()).await {
        Ok(n) => n,
        Err(Why::Refuse(code, m)) => return Ok(error(code, &m)),
        Err(Why::Decline(why)) => {
            h.media
                .counts
                .declines
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            say_declined(&h.account, Kind::Download, h.log, why);
            return Err(hand_back());
        }
    };
    Ok(match need {
        ModelNeed::NoNode => error(404, &format!("no node '{node}' in this project")),
        ModelNeed::Have => json_answer(200, &json!({ "job": null })),
        ModelNeed::Needs { file, url: None } => error(
            400,
            &format!(
                "'{node}' needs the model '{file}' and its module gives no model_url to fetch it from"
            ),
        ),
        ModelNeed::Needs {
            file,
            url: Some(url),
        } => json_answer(
            200,
            &json!({ "job": h.media.downloads.start(&file, &url).to_json() }),
        ),
    })
}
