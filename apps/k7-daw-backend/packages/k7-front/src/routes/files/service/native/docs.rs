//! docs.rs — the compile's nine routes answered from the shared song sessions: the active project read from the
//! marker, the roots and the rig made in Rust, the view's text sent as the session holds it (1110 to 1121).
//! What a reader gets wrong: a decline refuses with a sentence (`refuse.rs`); the sessions' lock is taken
//! only inside `stack::on_stack`, never across an await (1510).

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::response::Response;
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use k7_file_service::confine::Disk;
use k7_file_service::rel::to_rel;
use k7_file_service::revisions::Revisions;
use k7_file_service::rig::rig;
use k7_file_service::song_ask::{Input, song_roots};
use k7_file_service::space::Space;
use serde::Deserialize;
use serde_json::Value;
use song::doc::Vsts;
use song::graph::Folder;
use song::session::{Ask, View, engine_doc_of};

use super::answering::BASE_WEIGHT;
use super::docs_failure;
use super::jobs::Thawed;
use super::stack::{Failed, on_stack};
use super::stats::Stats;
use super::{Ctx, Home, Tag};
use crate::routes::files::service::Acct;
use crate::routes::files::service::body;
use crate::routes::files::service::routes::Call;

/// What a failed view's text starts with: the TypeScript's own path answers it.
const FAILURE: &str = "{\"failure\":";

/// What a docs route needs beyond a read's context: the account, the code root and the account's environment.
#[derive(Clone)]
pub struct Docs {
    pub acct: Arc<Acct>,
    pub code_root: String,
    pub env: BTreeMap<String, String>,
    /// The thawed modules the account's jobs hold, which an ask reads natively when `engine-jobs` answers (2310).
    pub thawed: Option<Arc<Thawed>>,
}

impl Docs {
    pub fn of(acct: &Arc<Acct>, code_root: &Path) -> Docs {
        Docs {
            acct: acct.clone(),
            code_root: code_root.display().to_string(),
            env: acct.env.clone(),
            thawed: None,
        }
    }

    /// A testkit's handle: a fresh account over `env`.
    #[cfg(test)]
    pub fn of_env(code_root: &Path, env: &BTreeMap<String, String>) -> Docs {
        Docs::of(&Acct::make(env.clone(), 384 << 20), code_root)
    }

    /// The account's song sessions, one set for every route.
    pub fn songs(&self) -> k7_file_service::songs::Songs {
        self.acct.songs()
    }

    /// The same, with the thawed list read from `thawed` instead of what the runtime says.
    pub fn with_thawed(mut self, thawed: Arc<Thawed>) -> Docs {
        self.thawed = Some(thawed);
        self
    }
}

/// The ask of a compile: the active project, its revision token, and the compile's key (1505).
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Asked {
    pub active: Option<String>,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    thawed: Vec<String>,
    /// A folder other than the open project (a library chain's graph): no automatic freezes are drawn.
    #[serde(skip)]
    other: bool,
}

impl Asked {
    /// The ask of a project the front compiles itself: nothing of the runtime's, the thawed list read from `Docs`.
    pub(super) fn of_project(active: &str, rev: Option<String>) -> Asked {
        Asked {
            active: Some(active.to_owned()),
            rev,
            thawed: Vec::new(),
            other: false,
        }
    }

    /// The ask of a workspace with no open project.
    fn none() -> Asked {
        Asked {
            active: None,
            rev: None,
            thawed: Vec::new(),
            other: false,
        }
    }

    /// The ask of a folder beside the open project, drawn with none of its own freezes.
    pub(super) fn of_other(dir: &str) -> Asked {
        Asked {
            other: true,
            ..Asked::of_project(dir, None)
        }
    }
}

/// The ask of a compile, read natively: the marker's project and the memo's token for it; blocking.
pub(super) fn asked_in(home: &Home, root: &str) -> Result<Asked, &'static str> {
    let space = Space::open(home, root);
    let Some(dir) = space.active().map_err(|d| d.as_str())? else {
        return Ok(Asked::none());
    };
    let rev = Revisions::global().token(home, space.root(), Some(&dir), false);
    Ok(Asked {
        active: Some(dir),
        rev: Some(rev),
        ..Asked::none()
    })
}

/// `asked_in` on the blocking pool.
pub(super) async fn asked_of(home: &Home, root: &str) -> Result<Asked, &'static str> {
    let (home, root) = (home.clone(), root.to_string());
    tokio::task::spawn_blocking(move || asked_in(&home, &root))
        .await
        .map_err(|_| "lost")?
}

/// The view a route asks of the session.
pub fn view_of(tag: Tag) -> Option<View> {
    Some(match tag {
        Tag::ProjectDoc => View::Web,
        Tag::EngineDoc => View::Engine,
        Tag::ProjectErrors => View::Errors,
        Tag::Graph => View::Graph,
        Tag::Hints => View::Hints,
        Tag::Surfaces => View::Surfaces,
        Tag::IoPorts => View::IoPorts,
        Tag::Catalogue => View::Catalogue,
        Tag::BaseModules => View::BaseModules,
        _ => return None,
    })
}

/// The views one compile makes touch the renders it used, as the host's `on` mode does (806).
fn compiles(view: View) -> bool {
    matches!(view, View::Engine | View::Web | View::Errors | View::Graph)
}

/// A session's text, or a slice of it, handed to the body without a copy (1120).
struct Text(Arc<str>, Range<usize>);

impl AsRef<[u8]> for Text {
    fn as_ref(&self) -> &[u8] {
        &self.0.as_bytes()[self.1.clone()]
    }
}

fn json_bytes(text: Text) -> Response {
    let mut r = Response::new(Body::from(Bytes::from_owner(text)));
    *r.status_mut() = StatusCode::OK;
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

/// The text a route sends: the whole view, the engine's `doc` slice, or the catalogue with relative paths (1518, 1502).
pub(super) enum Sent {
    Whole(Arc<str>),
    Slice(Arc<str>, Range<usize>),
    Mapped(String),
}

/// The view of a reply made into what the route sends, or why it declined.
pub(super) fn shape(ctx: &Ctx, view: View, text: Arc<str>) -> Result<Sent, &'static str> {
    if text.starts_with(FAILURE) {
        return Err("failure");
    }
    match view {
        View::Engine => {
            let doc = engine_doc_of(&text).ok_or("engine-shape")?;
            let start = doc.as_ptr() as usize - text.as_ptr() as usize;
            let range = start..start + doc.len();
            Ok(Sent::Slice(text, range))
        }
        View::Catalogue => catalogue(ctx, &text).map(Sent::Mapped).ok_or("catalogue"),
        _ => Ok(Sent::Whole(text)),
    }
}

/// The module catalogue with each row's `path` as the workspace spells it; an empty or absent `path` loses the key (1120).
pub fn catalogue(ctx: &Ctx, text: &str) -> Option<String> {
    let mut rows: Vec<Value> = serde_json::from_str(text).ok()?;
    let space = Space::open(&ctx.home, &ctx.root);
    for row in &mut rows {
        let row = row.as_object_mut()?;
        match row.get("path") {
            Some(Value::String(p)) if !p.is_empty() => {
                let rel = to_rel(&space, p);
                row.insert("path".into(), Value::String(rel));
            }
            Some(Value::Null | Value::String(_)) => {
                row.shift_remove("path");
            }
            None => {}
            Some(_) => return None,
        }
    }
    Some(k7_sync::json::stringify(&Value::Array(rows)))
}

/// What one ask is made of natively: the session's key and the disk it reads, beside the ask itself.
pub(super) struct Prepared {
    pub ask: Ask,
    pub key: String,
    pub rule: song::PathRule,
    pub disk: song::DiskSource,
}

/// The roots and the rig made in Rust for the runtime's ask; blocking, since it reads the disk.
pub(super) fn prepare(ctx: &Ctx, asked: Asked) -> Result<Prepared, &'static str> {
    prepare_with(&ctx.home, ctx.docs.as_ref().ok_or("no-docs")?, asked)
}

/// `prepare` for a caller with no request: the freezer's compile holds a `Home` and a `Docs`, nothing more.
pub(super) fn prepare_with(
    home: &Home,
    docs: &Docs,
    asked: Asked,
) -> Result<Prepared, &'static str> {
    let active = asked.active.ok_or("no-active-project")?;
    let other = asked.other;
    let input = Input {
        home,
        project: &active,
        code_root: &docs.code_root,
        env: &docs.env,
    };
    let mut roots = song_roots(&input, &Disk).map_err(|d| d.as_str())?;
    let rig = rig(home).map_err(|d| d.as_str())?;
    if roots.project.is_empty() {
        roots.project.clone_from(&active);
    }
    let ws = roots.confine.clone().unwrap_or_else(|| active.clone());
    let rule = roots.paths;
    let key = k7_file_service::songs::key_of(&ws, rule).map_err(|_| "workspace")?;
    let (disk, _) = k7_file_service::songs::disk_of(&roots).map_err(|_| "mounts")?;
    let ask = Ask {
        roots,
        rig,
        vsts: Vsts::Scan,
        thawed: docs.thawed.as_ref().map_or(asked.thawed, |t| t.of(&active)),
        rev: asked.rev,
        folder: if other { Folder::Other } else { Folder::Open },
    };
    Ok(Prepared {
        ask,
        key,
        rule,
        disk,
    })
}

/// The text of one view from the shared session; the one place its lock is taken (1112).
pub(super) fn view_text(ctx: &Ctx, p: &Prepared, view: View) -> Result<Arc<str>, &'static str> {
    let docs = ctx.docs.as_ref().ok_or("no-docs")?;
    view_text_with(docs, ctx.touches, p, view)
}

/// `view_text` with what it read of the request carried: the log, and whether the renders it used are marked used.
pub(super) fn view_text_with(
    docs: &Docs,
    touches: bool,
    p: &Prepared,
    view: View,
) -> Result<Arc<str>, &'static str> {
    let songs = docs.songs();
    let made = songs.with(&p.key, p.rule, |s| s.answer(&p.disk, &p.ask, &[view]));
    let reply = made.map_err(|_| "retired")?.map_err(|_| "compile")?;
    if compiles(view) && touches {
        k7_file_service::songs::touch_used(&reply.renders_used);
    }
    reply
        .views
        .into_iter()
        .find(|(v, _)| *v == view)
        .map(|(_, text)| text)
        .ok_or("no-view")
}

/// A node's missing model asked of the shared session, which holds nothing of it (2446); the one lock beside `view_text`'s.
pub(super) fn model_need(
    ctx: &Ctx,
    asked: Asked,
    node: &str,
) -> Result<song::graph::ModelNeed, &'static str> {
    let p = prepare(ctx, asked)?;
    let docs = ctx.docs.as_ref().ok_or("no-docs")?;
    let songs = docs.songs();
    let made = songs.with(&p.key, p.rule, |s| s.model_need(&p.disk, &p.ask, node));
    made.map_err(|_| "retired")?.map_err(|_| "graph")
}

fn compile(ctx: &Ctx, asked: Asked, view: View) -> Result<Arc<str>, &'static str> {
    #[cfg(test)]
    if matches!(ctx.chaos, super::Chaos::Panic) {
        panic!("a compile that dies");
    }
    view_text(ctx, &prepare(ctx, asked)?, view)
}

/// The graph view as JSON for an answer that carries one (3d): `None` where docs cannot answer; the caller leaves the key out.
pub(super) async fn graph_value(ctx: &Ctx) -> Option<Value> {
    ctx.docs.as_ref()?;
    let held = ctx.gate.take(&ctx.account, BASE_WEIGHT).await?;
    let asked = asked_of(&ctx.home, &ctx.root).await.ok()?;
    asked.active.as_ref()?;
    let ctx2 = ctx.clone();
    let made = on_stack(move || compile(&ctx2, asked, View::Graph)).await;
    drop(held);
    serde_json::from_str(&made.ok()?.ok()?).ok()
}

/// A view with no open project: what the route answers then, native (`docs_failure.rs`).
fn no_project(c: &Call, ctx: &Ctx, view: View) -> Response {
    let (status, body) = docs_failure::no_project(view);
    if view == View::Surfaces {
        c.say(
            "error",
            &docs_failure::no_project_line(&ctx.workspace, ctx.tag.route()),
        );
    }
    Stats::bump(&ctx.stats.docs_answered);
    crate::routes::files::json_answer(status, &body)
}

fn decline(ctx: &Ctx, why: &'static str) -> Option<Response> {
    ctx.declined(why);
    None
}

/// One docs route answered natively, or `None` where it declined (1115, 1517).
pub async fn answer(c: &Call, ctx: &Ctx) -> Option<Response> {
    let view = view_of(ctx.tag)?;
    ctx.docs.as_ref()?;
    let at = if ctx.tag == Tag::Graph {
        body::query(&c.query, "at")
    } else {
        String::new()
    };
    let Some(mut held) = ctx.gate.take(&ctx.account, BASE_WEIGHT).await else {
        return decline(ctx, "busy");
    };
    let asked = if at.is_empty() {
        let asked = match asked_of(&ctx.home, &ctx.root).await {
            Ok(a) => a,
            Err(why) => return decline(ctx, why),
        };
        if asked.active.is_none() {
            return Some(no_project(c, ctx, view));
        }
        asked
    } else {
        let space = Space::open(&ctx.home, &ctx.root);
        match space.safe_path(&at, &k7_file_service::confine::Disk) {
            Ok(abs) => Asked::of_other(&abs),
            Err(k7_file_service::space::PathError::Declined(d)) => return decline(ctx, d.as_str()),
            Err(e) => return Some(super::entries::error(400, &e.to_string())),
        }
    };
    let ctx2 = ctx.clone();
    let made = on_stack(move || compile(&ctx2, asked, view)).await;
    let text = match made {
        Ok(Ok(t)) => t,
        Ok(Err(why)) => return decline(ctx, why),
        Err(Failed::Panicked) => {
            Stats::bump(&ctx.stats.panics);
            (ctx.log)(&format!("[native] {} panicked", ctx.tag.route()));
            return decline(ctx, "panic");
        }
        Err(Failed::Lost) => return decline(ctx, "lost"),
    };
    // A project that will not compile is the song crate's findings, worded as the compiler's throw (4-7).
    if text.starts_with(FAILURE) {
        Stats::bump(&ctx.stats.docs_answered);
        return Some(docs_failure::compile_failure(&text));
    }
    // The catalogue is parsed, so it weighs six times its text on top (1516).
    if view == View::Catalogue && !held.grow_waiting(text.len().saturating_mul(6)).await {
        return decline(ctx, "busy");
    }
    let ctx3 = ctx.clone();
    let sent = tokio::task::spawn_blocking(move || shape(&ctx3, view, text))
        .await
        .ok()?;
    let sent = match sent {
        Ok(s) => s,
        Err(why) => return decline(ctx, why),
    };
    Stats::bump(&ctx.stats.docs_answered);
    Some(match sent {
        Sent::Whole(t) => {
            let n = t.len();
            json_bytes(Text(t, 0..n))
        }
        Sent::Slice(t, range) => json_bytes(Text(t, range)),
        Sent::Mapped(s) => {
            let t: Arc<str> = Arc::from(s);
            let n = t.len();
            json_bytes(Text(t, 0..n))
        }
    })
}
