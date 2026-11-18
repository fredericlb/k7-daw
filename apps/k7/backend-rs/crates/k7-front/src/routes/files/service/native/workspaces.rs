//! workspaces.rs — the home page's three listings and the acts on a whole workspace (make, remove, copy a project),
//! answered natively: the crate's plan and disk, the steps around them, the account's lock (docs/file-workspaces.md).
//! What a reader gets wrong: a removal's `letGo` and a room's join hold NO lock; the
//! lock covers the disk and the memos alone, and every act runs to its end in a task of its own once its plan passed.

use std::future::Future;
use std::sync::Arc;

use axum::body::Body;
use axum::response::Response;
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use k7_file_service::libraries::Home;
use k7_file_service::pins;
use k7_file_service::space::Declined;
use k7_file_service::tree::Limits;
use k7_file_service::workspaces::copy::{CopyAsk, answer_of, apply_copy, plan_copy};
use k7_file_service::workspaces::demos::demos_dir;
use k7_file_service::workspaces::life::{make, plan_make, plan_remove, remove};
use k7_file_service::workspaces::listing::{rows, summaries};
use k7_file_service::workspaces::names::{name_ok, on_disk, workspace_dir};
use k7_file_service::workspaces::{WsError, ms, rows_json, summaries_json, surfaced_json};
use k7_hub::Log;
use serde_json::{Value, json};

use super::answering::{Gate, TREE_WEIGHT};
use super::entries::told;
use super::{Tag, locks, workspaces_memo as memo};
use crate::routes::files::json_answer;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::stream::io_words;

const NAME: [(&str, Field); 1] = [("name", Field::Text)];
const COPY: [(&str, Field); 4] = [
    ("from", Field::Text),
    ("path", Field::Text),
    ("to", Field::Text),
    ("name", Field::Text),
];

/// The six routes of this piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    List,
    Make,
    Remove,
    Copy,
    Projects,
    Surfaces,
}

impl Kind {
    /// The route a method and a path name, when it is one of the six.
    pub fn of(method: &hyper::Method, path: &str) -> Option<Kind> {
        let (get, post) = (method == hyper::Method::GET, method == hyper::Method::POST);
        match path {
            "/workspaces" if get => Some(Kind::List),
            "/workspaces" if post => Some(Kind::Make),
            "/workspaces/remove" if post => Some(Kind::Remove),
            "/workspaces/copy-project" if post => Some(Kind::Copy),
            "/workspaces/projects" if get => Some(Kind::Projects),
            "/workspaces/surfaces" if get => Some(Kind::Surfaces),
            _ => None,
        }
    }

    pub fn tag(self) -> Tag {
        match self {
            Kind::List => Tag::Workspaces,
            Kind::Make => Tag::WorkspaceMake,
            Kind::Remove => Tag::WorkspaceRemove,
            Kind::Copy => Tag::CopyProject,
            Kind::Projects => Tag::WsProjects,
            Kind::Surfaces => Tag::WsSurfaces,
        }
    }

    pub(super) fn schema(self) -> &'static [(&'static str, Field)] {
        match self {
            Kind::Make | Kind::Remove => &NAME,
            Kind::Copy => &COPY,
            _ => &[],
        }
    }
}

/// What the piece needs of an account: its home, its ring, and the process's gate.
#[derive(Clone)]
pub struct Acct {
    /// The account's ring, which the Logs panel reads.
    pub runtime: Option<Arc<super::runtime::Runtime>>,
    pub home: Home,
    pub account: String,
    pub log: Log,
    /// The account's `STUDIO_DEMOS` and the code root, which the bundle's demos folder is found from.
    pub demos_from: (Option<String>, String),
    pub gate: &'static Gate,
    /// The account's engine link, which a removed workspace's served project is let go from.
    pub engine: Option<Arc<super::engine::routes::Engine>>,
}

impl Acct {
    /// A line node's service would say in its own process: the front's log, and the Logs panel's.
    fn say(&self, level: &str, line: &str) {
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

fn raw(status: u16, text: String) -> Response {
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

/// A refusal or a failure as the routes word it: an `fs` error through `entryFailure`, without the server's path (1691).
fn refusal(a: &Acct, e: &WsError) -> Response {
    if let WsError::Io { code, .. } = e {
        a.say("warn", &format!("[files] {e}"));
        let (status, msg) = io_words(Some(code));
        return error(status, &msg);
    }
    error(
        if matches!(e, WsError::Unknown { .. }) {
            404
        } else {
            400
        },
        &e.to_string(),
    )
}

/// `body` against the route's zod fields, or its 400.
fn invalid(kind: Kind, body: &Value) -> Option<Response> {
    body::check(body, kind.schema())
        .err()
        .map(|m| error(400, &m))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

/// An act past its plan runs to its end in a task of its own: a client that hung up must not strand it (1661).
async fn finished(work: impl Future<Output = Response> + Send + 'static) -> Response {
    super::refuse::spawn(work)
        .await
        .unwrap_or_else(|_| error(500, "internal error"))
}

/// A native act's step, which the tests see in order and may hold; never built in the front.
#[cfg(test)]
async fn step(_a: &Acct, root: &str, op: &str, _args: Value) {
    super::entries::calls::push(root, op);
    super::entries::calls::gate(root, op).await;
}

/// The route answered natively, or `None` where it declined (a decline, before anything is written).
pub async fn serve(a: &Acct, kind: Kind, body: &Value) -> Option<Response> {
    if let Some(r) = invalid(kind, body) {
        return Some(r);
    }
    match kind {
        Kind::List => list(a).await,
        Kind::Projects | Kind::Surfaces => listing(a, kind).await,
        Kind::Make => make_one(a, &body::text(body, "name")).await,
        Kind::Remove => remove_one(a, &body::text(body, "name")).await,
        Kind::Copy => copy_one(a, body).await,
    }
}

/// `GET /workspaces`: no memo, as the TypeScript has none (1637).
async fn list(a: &Acct) -> Option<Response> {
    let held = a.gate.take(&a.account, TREE_WEIGHT).await;
    let Some(held) = held else {
        a.declined(Tag::Workspaces, Declined::Busy.as_str());
        return None;
    };
    let home = a.home.clone();
    let walked = blocking(move || {
        let _held = held;
        summaries(&home, &Limits::default())
    })
    .await?;
    match walked {
        Ok(rows) => Some(raw(200, summaries_json(&rows))),
        Err(d) => {
            a.declined(Tag::Workspaces, d.as_str());
            None
        }
    }
}

/// `/workspaces/projects` and `/surfaces`: one walk feeds both, kept 5 seconds as text (1670, 1671).
async fn listing(a: &Acct, kind: Kind) -> Option<Response> {
    let pick = |e: &memo::Entry| match kind {
        Kind::Projects => e.projects.to_string(),
        _ => e.surfaced.to_string(),
    };
    let dir = workspace_dir(&a.home);
    let mtime = blocking(move || ms(std::fs::metadata(&dir).and_then(|m| m.modified()))).await?;
    if let Some(e) = memo::fresh(&a.account, mtime) {
        return Some(raw(200, pick(&e)));
    }
    let acts = memo::acts(&a.account);
    let Some(held) = a.gate.take(&a.account, TREE_WEIGHT).await else {
        a.declined(kind.tag(), Declined::Busy.as_str());
        return None;
    };
    let (home, (env_demos, code)) = (a.home.clone(), a.demos_from.clone());
    let walked = blocking(move || {
        let _held = held;
        let demos = demos_dir(env_demos.as_deref(), &code);
        rows(&home, demos.as_deref(), &Limits::default())
    })
    .await?;
    let (rows, stamps) = match walked {
        Ok(w) => w,
        Err(d) => {
            a.declined(kind.tag(), d.as_str());
            return None;
        }
    };
    let entry = memo::Entry {
        at: std::time::Instant::now(),
        root_mtime: stamps.root_mtime,
        projects: rows_json(&rows).into(),
        surfaced: surfaced_json(&rows).into(),
        acts,
    };
    let text = pick(&entry);
    memo::keep(&a.account, entry);
    Some(raw(200, text))
}

/// `POST /workspaces`: the folder made under the lock, its pins made inside the write (1662, 1622).
async fn make_one(a: &Acct, name: &str) -> Option<Response> {
    let lock = locks::write(&a.home.studio).await;
    let (home, n) = (a.home.clone(), name.to_string());
    let planned = blocking(move || plan_make(&home, home.admin, &n)).await?;
    match planned {
        Err(WsError::Declined(d)) => {
            a.declined(Tag::WorkspaceMake, d.as_str());
            return None;
        }
        Err(e) => return Some(refusal(a, &e)),
        Ok(false) => return Some(json_answer(200, &json!({ "created": false }))),
        Ok(true) => {}
    }
    let (a, name) = (a.clone(), name.to_string());
    Some(
        finished(async move {
            let _lock = lock;
            let (home, n) = (a.home.clone(), name.clone());
            let made = blocking(move || make(&home, &n)).await;
            let created = match made {
                Some(Ok(c)) => c,
                Some(Err(e)) => return refusal(&a, &e),
                None => return error(500, "internal error"),
            };
            if created {
                let root = format!("{}/{name}", workspace_dir(&a.home));
                let (home, r2) = (a.home.clone(), root.clone());
                match blocking(move || pins::ensure_made(&home, &r2)).await {
                    Some(Ok(_)) => {}
                    Some(Err(d)) => {
                        a.say("warn", &format!("[workspaces] pins: {}", d.as_str()));
                        return error(500, "internal error");
                    }
                    None => return error(500, "internal error"),
                }
                #[cfg(test)]
                step(
                    &a,
                    &root,
                    "pinsMade",
                    json!({ "ws": root, "admin": a.home.admin }),
                )
                .await;
            }
            memo::dropped(&a.account);
            json_answer(200, &json!({ "created": created }))
        })
        .await,
    )
}

/// `POST /workspaces/remove`: the handles let go with no lock held, then the folder and its marker under it (1621, 1660).
async fn remove_one(a: &Acct, name: &str) -> Option<Response> {
    let (home, n) = (a.home.clone(), name.to_string());
    let planned = blocking(move || plan_remove(&home, &n)).await?;
    let root = match planned {
        Err(WsError::Declined(d)) => {
            a.declined(Tag::WorkspaceRemove, d.as_str());
            return None;
        }
        Err(e) => return Some(refusal(a, &e)),
        Ok(root) => root,
    };
    let a = a.clone();
    Some(
        finished(async move {
            let_go(&a, &root).await;
            let _lock = locks::write(&a.home.studio).await;
            let (home, r) = (a.home.clone(), root.clone());
            match blocking(move || remove(&home, &r)).await {
                Some(Ok(())) => {}
                Some(Err(e)) => return refusal(&a, &e),
                None => return error(500, "internal error"),
            }
            k7_file_service::revisions::Revisions::global().forget(&root);
            super::projects_open::opened(&root, None);
            #[cfg(test)]
            step(&a, &root, "forget", json!({ "ws": root })).await;
            memo::dropped(&a.account);
            json_answer(200, &json!({ "ok": true }))
        })
        .await,
    )
}

/// The handles let go, then the room ended: the module watches, the engine serving it, the registry's account (2157).
async fn let_go(a: &Acct, root: &str) {
    super::runtime::release_watches(a.runtime.as_ref(), root).await;
    if let Some(e) = &a.engine {
        e.link.unload_under(root).await;
    }
    let held = k7_capture::Captures::global().at(root);
    #[cfg(test)]
    step(
        a,
        root,
        "letGo",
        json!({ "ws": root, "native": held.is_some() }),
    )
    .await;
    if let Some((acct, _)) = held {
        acct.end_workspace(root).await;
    }
}

/// `getWorkspace` on each end of a copy, in order: a name that is invalid or
/// missing stops the walk and the plan answers it (2654); the spellings to join otherwise.
pub(super) fn copy_ends(home: &Home, from: &str, to: &str) -> Vec<String> {
    let dir = workspace_dir(home);
    [from, to]
        .iter()
        .filter(|e| name_ok(e))
        .map(|e| on_disk(&dir, e).unwrap_or_else(|| e.to_string()))
        .collect()
}

/// `POST /workspaces/copy-project`: both ends joined, the plan under the lock and the weight, the writes in a task (1623).
async fn copy_one(a: &Acct, body: &Value) -> Option<Response> {
    let t = |k: &str| body::text(body, k);
    let (from, path, to, name) = (t("from"), t("path"), t("to"), t("name"));
    let home = a.home.clone();
    let (f, t) = (from.clone(), to.clone());
    // The folders' own spellings are read off the async threads.
    let spelled = blocking(move || copy_ends(&home, &f, &t)).await?;
    #[cfg(test)]
    let spelled_twin = spelled.clone();
    if let Some(acct) = k7_capture::Captures::global().account(&a.home.studio) {
        for name in spelled {
            // `getWorkspace` joins a workspace that is there; a missing end is refused below, never made.
            let root = format!("{}/{name}", workspace_dir(&a.home));
            if tokio::fs::metadata(&root).await.is_ok_and(|m| m.is_dir()) {
                acct.attach_named(&name).await;
            }
        }
    }
    #[cfg(test)]
    for name in spelled_twin {
        step(a, "", "join", json!({ "name": name })).await;
    }
    let lock = locks::write(&a.home.studio).await;
    let Some(held) = a.gate.take(&a.account, TREE_WEIGHT).await else {
        a.declined(Tag::CopyProject, Declined::Busy.as_str());
        return None;
    };
    let home = a.home.clone();
    let planned = blocking(move || {
        let _held = held;
        let ask = CopyAsk {
            from: &from,
            path: &path,
            to: &to,
            name: &name,
        };
        plan_copy(&home, home.admin, &ask, &Limits::default())
    })
    .await?;
    let plan = match planned {
        Err(WsError::Declined(d)) => {
            a.declined(Tag::CopyProject, d.as_str());
            return None;
        }
        Err(e) => return Some(refusal(a, &e)),
        Ok(p) => p,
    };
    let a = a.clone();
    Some(
        finished(async move {
            let _lock = lock;
            let p = plan.clone();
            match blocking(move || apply_copy(&p)).await {
                Some(Ok(())) => {}
                Some(Err(e)) => return refusal(&a, &e),
                None => return error(500, "internal error"),
            }
            told(&plan.dest, "listed").await;
            memo::dropped(&a.account);
            raw(200, answer_of(&plan))
        })
        .await,
    )
}
