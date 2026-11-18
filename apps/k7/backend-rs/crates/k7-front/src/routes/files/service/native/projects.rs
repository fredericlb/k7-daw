//! projects.rs — `POST /open`, `/project/rename`, `/project/delete` and `GET /project` answered natively: the crate's
//! plans, the account's lock around the marker and the move, the five steps around them (docs/file-projects.md, 2000 to 2064).
//! What a reader gets wrong: the engine's unload and the journal's gate come BEFORE the account's lock, never inside it.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use k7_file_service::confine::Disk;
use k7_file_service::entries::apply::{self, RETRY, make_folder};
use k7_file_service::journal::Journals;
use k7_file_service::libraries::Home;
use k7_file_service::paths::{dirname, resolve_all};
use k7_file_service::projects::ProjError;
use k7_file_service::projects::info::project_info;
use k7_file_service::projects::life::{MovePlan, kept_answer, plan_recycle, plan_rename};
use k7_file_service::projects::marker;
use k7_file_service::rel::to_rel;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;
use k7_hub::Log;
use serde_json::{Value, json};

use super::entries::{blocking, rename_retrying};
use super::{Ctx, Tag, locks, projects_open, workspaces_memo as memo};
use crate::routes::files::json_answer;
use crate::routes::files::service::Who;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::stream::io_words;

const OPEN: [(&str, Field); 2] = [("path", Field::Text), ("libraries", Field::OptStr)];
const PATH_NAME: [(&str, Field); 2] = [("path", Field::Text), ("name", Field::Text)];
const PATH: [(&str, Field); 1] = [("path", Field::Text)];
/// How long a move waits for the records in flight: None is as long as a record's lease, which abandons a hung one (F1).
const GATE_WAIT: Option<Duration> = None;

/// `GATE_WAIT`, shrunk under test so a restored bound fails a short hold.
fn gate_wait() -> Option<Duration> {
    GATE_WAIT.map(|w| {
        if cfg!(test) {
            w.min(Duration::from_millis(100))
        } else {
            w
        }
    })
}

/// The three routes of this piece that write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
    Rename,
    Recycle,
}

impl Kind {
    /// The route a method and a path name, when it is one of the three.
    pub fn of(method: &hyper::Method, path: &str) -> Option<Kind> {
        match path {
            "/open" if method == hyper::Method::POST => Some(Kind::Open),
            "/project/rename" if method == hyper::Method::POST => Some(Kind::Rename),
            "/project/delete" if method == hyper::Method::POST => Some(Kind::Recycle),
            _ => None,
        }
    }

    pub fn tag(self) -> Tag {
        match self {
            Kind::Open => Tag::Open,
            Kind::Rename => Tag::ProjectRename,
            Kind::Recycle => Tag::ProjectDelete,
        }
    }

    pub(super) fn schema(self) -> &'static [(&'static str, Field)] {
        match self {
            Kind::Open => &OPEN,
            Kind::Rename => &PATH_NAME,
            Kind::Recycle => &PATH,
        }
    }
}

/// What the piece needs of an account: its home, its ring, its engine link and its media.
#[derive(Clone)]
pub struct Proj {
    /// The account's ring, which the Logs panel reads.
    pub runtime: Option<Arc<super::runtime::Runtime>>,
    pub home: Home,
    pub account: String,
    pub log: Log,
    /// The account's `STUDIO_DEMOS` and the code root, which the bundle's demos folder is found from.
    pub demos_from: (Option<String>, String),
    /// The account's engine link, where `engine` is on: the open answers its load from it (2247).
    pub engine: Option<Arc<super::engine::routes::Engine>>,
    /// The account's media, where `media` answers: an open warms its project's waveforms through it.
    pub site: Option<super::media::host::Site>,
    /// The account's native capture, where `capture` answers: the sync gate of an open reads it.
    pub capture: Option<Arc<k7_capture::account::Account>>,
    /// The hub's address and secret, which the libraries are refreshed through.
    pub hub: Option<(String, String)>,
    /// The account's media, whose prewarm a folder's move cancels first.
    pub media: Option<Arc<super::media::Media>>,
    /// A test shrinks the walk's bound to make a `TooMany` small.
    #[cfg(test)]
    pub max_seen: Option<u64>,
}

impl Proj {
    /// The walk's limits: the crate's, or the test's shrunk one.
    pub(super) fn limits(&self) -> Limits {
        #[cfg(test)]
        if let Some(max_seen) = self.max_seen {
            return Limits {
                max_seen,
                ..Limits::default()
            };
        }
        Limits::default()
    }

    /// A line node's service would say in its own process: the front's log, and the Logs panel's.
    pub(super) fn say(&self, level: &str, line: &str) {
        (self.log)(line);
        if let Some(r) = &self.runtime {
            r.say(level, line);
        }
    }

    pub(super) fn declined(&self, tag: Tag, why: &'static str) {
        super::stats::global().declined_for(
            &self.account,
            tag.route(),
            &self.account,
            &self.log,
            why,
        );
    }
}

pub(super) fn raw(status: u16, text: String) -> Response {
    let mut r = Response::new(Body::from(text));
    *r.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

pub(super) fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

/// A refusal or a failure as the routes word it: an `fs` error through `entryFailure`, without the server's path (2095).
pub(super) fn refusal(p: &Proj, e: &ProjError) -> Response {
    if let ProjError::Io { code, .. } = e {
        p.say("warn", &format!("[files] {e}"));
        let (status, msg) = io_words(Some(code));
        return error(status, &msg);
    }
    error(400, &e.to_string())
}

/// A step that was lost, as `wrap` answers a throw no route catches: logged, a generic 500.
pub(super) fn uncaught(p: &Proj, name: &str, kind: Kind) -> Response {
    let what = kind.tag().route();
    p.say(
        "error",
        &format!("[{name}] {what} failed: the step was lost"),
    );
    error(500, "internal error")
}

/// A folder's handles let go before it moves: the prewarm, the module watches of a recycle, the engine serving it.
async fn released(p: &Proj, root: &str, abs: &str, watches: bool) {
    if let Some(m) = &p.media {
        m.prewarms.cancel(root, &m.waves).await;
    }
    if watches {
        super::runtime::release_watches(p.runtime.as_ref(), abs).await;
    }
    if let Some(e) = &p.engine {
        e.link.unload_under(abs).await;
    }
    #[cfg(test)]
    step(
        p,
        root,
        "releaseFolder",
        json!({ "ws": root, "abs": abs, "watches": watches }),
    )
    .await;
}

/// A native act's step, which the tests see in order and may hold; never built in the front.
#[cfg(test)]
pub(super) async fn step(_p: &Proj, root: &str, op: &str, args: Value) {
    super::entries::calls::push_args(root, op, &args);
    super::entries::calls::gate(root, op).await;
}

/// A task of its own: a client that hung up must not strand a half-done move (2034, 2043).
pub(super) async fn finished(
    work: impl Future<Output = Option<Response>> + Send + 'static,
) -> Option<Response> {
    match super::refuse::spawn(work).await {
        Ok(r) => r,
        Err(_) => Some(error(500, "internal error")),
    }
}

/// The hook: one of the three POSTs answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(kind) = Kind::of(req.method(), path) else {
        return Err(req);
    };
    let account = &who.held.account;
    let code = who
        .code_root
        .map_or_else(String::new, |r| r.display().to_string());
    let p = Proj {
        runtime: who.held.runtime.clone(),
        home: Home::of(&account.env, who.admin),
        account: account.slug.clone(),
        log: who.log.clone(),
        demos_from: (account.env.get("STUDIO_DEMOS").cloned(), code),
        engine: who.held.engine.clone(),
        site: super::media::host::site_for(who),
        capture: super::open_prepare::capture_of(who),
        hub: super::listings::hub_of(&account.env),
        media: who.held.media.clone(),
        #[cfg(test)]
        max_seen: None,
    };
    let resolve = |h: &HeaderMap| resolved(who, h);
    hook(&p, kind, resolve, req).await
}

/// `requireRootOwner` then `resolveWorkspace`: the workspace's name and root, or the refusal.
fn resolved(who: &Who<'_>, h: &HeaderMap) -> Result<(String, String), Box<Response>> {
    crate::routes::files::service::workspace(who, h).map(|(n, r)| (n, r.display().to_string()))
}

/// The hook past the account: the body parses, the workspace resolves, the zod fields hold, then the route (2000).
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn hook(
    p: &Proj,
    kind: Kind,
    resolve: impl FnOnce(&HeaderMap) -> Result<(String, String), Box<Response>>,
    req: Request,
) -> Result<Response, Request> {
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
    let (name, root) = match resolve(&parts.headers) {
        Ok(w) => w,
        Err(r) => return Ok(*r),
    };
    if let Err(m) = body::check(&parsed, kind.schema()) {
        return Ok(error(400, &m));
    }
    let done = match kind {
        Kind::Open => projects_open::open(p, &name, &root, &parsed).await,
        Kind::Rename | Kind::Recycle => moved(p, kind, &root, &parsed).await,
    };
    match done {
        Some(r) => Ok(r),
        None => Err(Request::from_parts(parts, Body::from(bytes))),
    }
}

/// The plan of a move on a blocking thread: every guard and stat the TypeScript runs, nothing written (2041, 2042).
pub(super) fn plan_of(
    home: &Home,
    root: &str,
    kind: Kind,
    rel: &str,
    name: &str,
) -> Result<Option<MovePlan>, ProjError> {
    let space = Space::open(home, root);
    match kind {
        Kind::Recycle => plan_recycle(&space, home.admin, rel, &Disk).map(Some),
        _ => plan_rename(&space, home.admin, rel, name, &Disk),
    }
}

/// `GET /project`, answering: no ask and no lock, a read that writes nothing (2071).
pub async fn answer(ctx: &Ctx) -> Option<Response> {
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let done = blocking(move || project_info(&Space::open(&home, &root))).await?;
    match done {
        Ok(text) => Some(raw(200, text)),
        Err(d) => {
            ctx.declined(d.as_str());
            None
        }
    }
}

/// A rename or a recycle: the plan, the unload, the gate alone, the lock, the plan again, the move, the marker (2040).
async fn moved(p: &Proj, kind: Kind, root: &str, body: &Value) -> Option<Response> {
    let (rel, name) = (body::text(body, "path"), body::text(body, "name"));
    let (home, r2, (rel2, name2)) = (
        p.home.clone(),
        root.to_string(),
        (rel.clone(), name.clone()),
    );
    let planned = blocking(move || plan_of(&home, &r2, kind, &rel2, &name2)).await?;
    let plan = match planned {
        Err(ProjError::Declined(d)) => {
            p.declined(kind.tag(), d.as_str());
            return None;
        }
        Err(e) => return Some(refusal(p, &e)),
        Ok(None) => return kept(p, root, &rel).await,
        Ok(Some(plan)) => plan,
    };
    let (p, root) = (p.clone(), root.to_string());
    finished(async move {
        // The handles go first, with no lock and no gate held: the unload may take a while (2044).
        released(&p, &root, &plan.abs, kind == Kind::Recycle).await;
        let journals = Journals::global();
        let gate = resolve_all(&[&root]);
        let alone = match gate_wait() {
            None => journals.exclusive(&gate).await,
            Some(w) => match tokio::time::timeout(w, journals.exclusive(&gate)).await {
                Ok(alone) => alone,
                Err(_) => return Some(error(409, "a command is still running on this workspace")),
            },
        };
        let lock = locks::write(&p.home.studio).await;
        let done = under_lock(&p, kind, &root, (&rel, &name)).await;
        drop((lock, alone));
        done
    })
    .await
}

/// A rename to the name it has: `{path}` and nothing done.
async fn kept(p: &Proj, root: &str, rel: &str) -> Option<Response> {
    let (home, r2, rel2) = (p.home.clone(), root.to_string(), rel.to_string());
    let text = blocking(move || kept_answer(&Space::open(&home, &r2), &rel2, &Disk)).await?;
    match text {
        Ok(t) => Some(raw(200, t)),
        Err(ProjError::Declined(d)) => {
            p.declined(Tag::ProjectRename, d.as_str());
            None
        }
        Err(e) => Some(refusal(p, &e)),
    }
}

/// The plan again, its answer the one that counts; the move, the marker and the memory (2040, 2093).
async fn under_lock(
    p: &Proj,
    kind: Kind,
    root: &str,
    (rel, name): (&str, &str),
) -> Option<Response> {
    let (home, r2, (rel2, name2)) = (
        p.home.clone(),
        root.to_string(),
        (rel.to_string(), name.to_string()),
    );
    let again = blocking(move || plan_of(&home, &r2, kind, &rel2, &name2)).await;
    let plan = match again {
        Some(Ok(Some(plan))) => plan,
        Some(Ok(None)) => return kept(p, root, rel).await,
        Some(Err(ProjError::Declined(d))) => {
            p.declined(kind.tag(), d.as_str());
            p.say(
                "warn",
                &format!("[files] native: declined under the lock: {}", d.as_str()),
            );
            return Some(error(500, "internal error"));
        }
        Some(Err(e)) => return Some(refusal(p, &e)),
        None => {
            p.say("warn", "[files] native: the plan under the lock was lost");
            return Some(error(500, "internal error"));
        }
    };
    if kind == Kind::Recycle {
        let bin = dirname(&plan.dest);
        match blocking(move || make_folder(&bin)).await {
            Some(Ok(())) => {}
            Some(Err(e)) => return Some(refusal(p, &e.into())),
            None => return Some(error(500, "internal error")),
        }
    }
    let real = Arc::new(apply::Disk);
    if let Err(e) = rename_retrying(real, &plan.abs, &plan.dest, RETRY.1).await {
        return Some(refusal(p, &e.into()));
    }
    let marked = marked(p, kind, root, &plan).await;
    super::projects_open::moved_away(root, &plan.abs);
    #[cfg(test)]
    {
        let to = (kind == Kind::Rename).then_some(plan.dest.as_str());
        step(
            p,
            root,
            "moved",
            json!({ "ws": root, "from": plan.abs, "to": to }),
        )
        .await;
    }
    Revisions::global().forget(&plan.abs);
    memo::dropped(&p.account);
    match marked {
        Ok(()) => Some(raw(200, plan.answer)),
        Err(e) => {
            // The folder moved: the person hears so (U4), the failure is logged and counted.
            p.say(
                "warn",
                &format!("[files] native: marker write failed after the move: {e}"),
            );
            super::stats::Stats::bump(&super::stats::global().io);
            Some(raw(200, plan.answer))
        }
    }
}

/// The marker after a move: a rename of the open project (or its ancestor) re-points it, a recycle of it empties it.
async fn marked(p: &Proj, kind: Kind, root: &str, plan: &MovePlan) -> Result<(), ProjError> {
    if !plan.was_active {
        return Ok(());
    }
    let moved_to = plan.active_to.clone().unwrap_or_else(|| plan.dest.clone());
    let (home, r2) = (p.home.clone(), root.to_string());
    let done = blocking(move || match kind {
        Kind::Rename => {
            let rel = to_rel(&Space::open(&home, &r2), &moved_to);
            marker::write(&r2, &rel)
        }
        _ => marker::clear(&r2),
    })
    .await;
    done.unwrap_or_else(|| {
        Err(ProjError::Io {
            code: "EIO",
            syscall: "rename",
            path: String::new(),
        })
    })
}
