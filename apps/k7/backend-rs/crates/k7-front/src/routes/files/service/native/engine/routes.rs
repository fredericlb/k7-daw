//! The five routes the link answers (2226, 2227): the poll, the client's document, the unload, the copy's retry and the served project.
//! What a reader gets wrong: the poll starts its prewarm in a task and never awaits it; a decline hands the request back whole.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use k7_file_service::engine::{served_project, session_for};
use k7_file_service::libraries::Home;
use k7_file_service::projects::{info::workspace_info, info_json};
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;
use serde_json::{Value, json};

use super::EngineLink;
use super::host::Bridge;
use super::real::RealHost;
use crate::routes::files::json_answer;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::native::Tag;
use crate::routes::files::service::native::docs::Docs;
use crate::routes::files::service::native::entries::blocking;
use crate::routes::files::service::native::jobs::Jobs;
use crate::routes::files::service::native::jobs::driver::{DriverEnv, real_driver};
use crate::routes::files::service::native::jobs::freezer::FreezerTask;
use crate::routes::files::service::native::jobs::report::ReportTask;
use crate::routes::files::service::native::projects::{error, raw};
use crate::routes::files::service::{Who, claim_root, live, workspace};

/// An account's engine: the link, the bridge that files its takes, and the host that compiles for it.
pub struct Engine {
    pub link: EngineLink,
    pub bridge: Arc<Bridge>,
    pub host: Arc<RealHost>,
    /// The account's jobs: freezes, renders and separations (2306).
    pub jobs: OnceLock<Arc<Jobs>>,
    /// The account's runtime, whose background jobs the process list shows beside the engine's (2630).
    pub runtime: OnceLock<Arc<super::super::runtime::Runtime>>,
    /// The automatic freezer's timer: dropped with the engine, which stops it.
    freezer: Mutex<Option<FreezerTask>>,
    /// The 2362 line's task, dropped with the engine.
    reporter: Mutex<Option<ReportTask>>,
    log: k7_hub::Log,
    files_root: String,
}

impl Engine {
    /// Must be made inside a runtime: the link's consumer, the freezer's timer and the report line run on it.
    pub fn start(
        account: &k7_hub::account::Account,
        acct: &Arc<crate::routes::files::service::Acct>,
        user: &str,
        code_root: &std::path::Path,
        log: k7_hub::Log,
    ) -> Arc<Engine> {
        let files_root = account.home.join("workspace").display().to_string();
        let mediatek = account
            .env
            .get("STUDIO_MEDIATEK")
            .map(std::path::PathBuf::from);
        let roots = k7_hub::binding::roots_for(&account.home, &account.slug, mediatek.as_deref())
            .iter()
            .filter_map(|r| r["remote"].as_str().map(str::to_string))
            .collect();
        let host = RealHost::new(
            Home::of(&account.env, false),
            code_root.display().to_string(),
            acct.clone(),
        );
        let engine = Engine::build(files_root, user, roots, host, log);
        engine.hold_jobs(&account.env, code_root);
        engine
    }

    /// The account's jobs held, the freezer's timer started over the link and the 2362 line (2306).
    pub fn hold_jobs(&self, env: &BTreeMap<String, String>, code_root: &Path) {
        let pairs = env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let driver = real_driver(&self.link, DriverEnv::new(code_root.to_path_buf(), pairs));
        let jobs = Jobs::new(Some(driver));
        self.host.hold_thawed(jobs.thawed.clone());
        let _ = self.jobs.set(jobs.clone());
        let docs = Docs::of(&self.host.acct(), code_root).with_thawed(jobs.thawed.clone());
        let task = jobs.start_freezer(self.link.clone(), docs, &tokio::runtime::Handle::current());
        *self.freezer.lock().unwrap_or_else(|p| p.into_inner()) = Some(task);
        let every = crate::routes::files::service::native::jobs::report::REPORT_EVERY;
        let report = jobs.report(
            &self.link.0.account.email,
            self.log.clone(),
            every,
            &tokio::runtime::Handle::current(),
        );
        *self.reporter.lock().unwrap_or_else(|p| p.into_inner()) = Some(report);
    }

    /// The link, the bridge and the host joined, with no jobs yet.
    pub fn build(
        files_root: String,
        user: &str,
        roots: Vec<String>,
        host: Arc<RealHost>,
        log: k7_hub::Log,
    ) -> Arc<Engine> {
        let say = log.clone();
        let bridge = Bridge::new(host.clone(), Arc::new(move |l: &str| say(l)));
        let link = EngineLink::new(
            super::Account {
                files_root: files_root.clone(),
                email: user.to_string(),
                roots,
            },
            host.clone(),
            bridge.clone(),
        );
        bridge.set_link(link.clone());
        Arc::new(Engine {
            link,
            bridge,
            host,
            jobs: OnceLock::new(),
            runtime: OnceLock::new(),
            freezer: Mutex::new(None),
            reporter: Mutex::new(None),
            log: log.clone(),
            files_root,
        })
    }

    pub fn files_root(&self) -> &str {
        &self.files_root
    }
}

/// The five routes: answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live_hook(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(engine) = who.held.engine.clone() else {
        return Err(req);
    };
    let post = req.method() == hyper::Method::POST;
    let tag = match (req.method(), path) {
        (&hyper::Method::GET, "/workspace") => Tag::Workspace,
        (&hyper::Method::GET, "/engine/served") => Tag::EngineServed,
        _ if post && path == "/engine/project" => Tag::EngineProject,
        _ if post && path == "/engine/unload" => Tag::EngineUnload,
        _ if post && path == "/engine/sync/retry" => Tag::EngineSyncRetry,
        _ => return super::super::jobs::routes::hook(who, &engine, path, req).await,
    };
    engine.host.note_admin(who.admin);
    match tag {
        Tag::Workspace => poll(&engine, who, req).await,
        Tag::EngineServed => Ok(served(&engine, who)),
        _ => act(&engine, who, path, req).await,
    }
}

pub(crate) fn declined(who: &Who<'_>, tag: Tag, why: &'static str) {
    let slug = &who.held.account.slug;
    super::super::stats::global().declined_for(slug, tag.route(), slug, who.log, why);
}

/// `GET /engine/served`: the project the engine holds for this person, by address.
fn served(e: &Engine, who: &Who<'_>) -> Response {
    if !claim_root(&who.held.account.home, who.user) {
        return json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account" }),
        );
    }
    let project = [session_for("k7", Some(who.user)), "k7".to_string()]
        .iter()
        .find_map(|n| e.link.find(n)?.served())
        .and_then(|s| served_project(&e.files_root, &s.root, &s.project))
        .map_or(Value::Null, |(w, p)| json!({ "workspace": w, "path": p }));
    json_answer(200, &json!({ "project": project }))
}

/// `GET /workspace`: the info, the take, the copy, the load, the scan and the processes; the prewarm runs beside it.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
async fn poll(e: &Arc<Engine>, who: &Who<'_>, req: Request) -> Result<Response, Request> {
    let (_, root) = match workspace(who, req.headers()) {
        Ok(w) => w,
        Err(r) => return Ok(*r),
    };
    let root = root.display().to_string();
    let account = &who.held.account;
    let home = Home::of(&account.env, who.admin);
    let code = who
        .code_root
        .map_or_else(String::new, |r| r.display().to_string());
    let demos = account.env.get("STUDIO_DEMOS").cloned();
    let r2 = root.clone();
    let walked = blocking(move || {
        let space = Space::open(&home, &r2);
        let demos = k7_file_service::workspaces::demos::demos_dir(demos.as_deref(), &code);
        let info = workspace_info(&space, demos.as_deref(), &Limits::default())?;
        let active = space.active()?;
        Ok::<_, k7_file_service::space::Declined>((info, active))
    })
    .await;
    let (info, active) = match walked {
        Some(Ok(w)) => w,
        Some(Err(d)) => {
            declined(who, Tag::Workspace, d.as_str());
            return Err(req);
        }
        None => return Ok(error(500, "internal error")),
    };
    let session = e.link.session(Some(&root));
    if let Some(a) = &active {
        session.prewarm_soon(&root, a);
    }
    let part = |v: Option<Value>| v.unwrap_or(Value::Null);
    let tail = format!(
        "\"lastTake\":{},\"engineSync\":{},\"engineLoad\":{},\"engineScan\":{},\"processes\":{}",
        part(e.bridge.last_take(session.name())),
        part(session.sync_state().map(|s| s.to_json())),
        part(
            session
                .engine_load_of(active.as_deref())
                .map(|p| json!(p.as_str()))
        ),
        part(session.scan_view().map(|s| s.to_json())),
        Value::Array(e.runtime.get().map(|r| r.processes()).unwrap_or_default()),
    );
    Ok(live::as_node_answers(raw(200, info_json(&info, &tail))).await)
}

/// The three POSTs: the body parsed, the workspace resolved, the fields held, then the act.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
async fn act(
    e: &Arc<Engine>,
    who: &Who<'_>,
    path: &str,
    req: Request,
) -> Result<Response, Request> {
    let (parts, payload) = req.into_parts();
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
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
        Err(m) => return Ok(error(400, &m)),
    };
    let (_, root) = match workspace(who, &parts.headers) {
        Ok(w) => w,
        Err(r) => return Ok(*r),
    };
    let root = root.display().to_string();
    let session = e.link.session(Some(&root));
    let ok = || json_answer(200, &json!({ "ok": true }));
    match path {
        "/engine/project" => {
            if let Err(m) = body::check(&parsed, &[("doc", Field::Object)]) {
                return Ok(error(400, &m));
            }
            session.apply_client_now(parsed["doc"].clone());
        }
        "/engine/unload" => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            session.enqueue(move |s| async move {
                s.unload().await;
                let _ = tx.send(());
            });
            let _ = rx.await;
        }
        _ => {
            session.forget_failure();
            let home = Home::of(&who.held.account.env, who.admin);
            let r2 = root.clone();
            let active = blocking(move || Space::open(&home, &r2).active().ok().flatten())
                .await
                .flatten();
            if let Some(project) = active {
                // Answered once the copy is under way, as the TypeScript's synchronous retry was: the next poll shows it.
                let (tx, rx) = tokio::sync::oneshot::channel();
                session.enqueue(move |s| async move {
                    s.reload(&root, &project).await;
                    let _ = tx.send(());
                });
                let _ = rx.await;
            }
        }
    }
    Ok(ok())
}
