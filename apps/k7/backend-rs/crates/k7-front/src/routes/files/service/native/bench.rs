//! bench.rs — `POST /workspaces/bench` and `POST /account/slot` answered natively: the live rig made from a library chain, a chain compiled into a slot.
//! What a reader gets wrong: only an `answer` mode takes them (they write), and a request this side cannot judge is a 503: no other side answers.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use k7_file_service::bench::{
    BenchError, Libs, bench_chain_of, copy_frozen_chain, delete_frozen_chain, ensure_bench, opened,
    save_bench_chain, slot_of, workspace_root,
};
use k7_file_service::journal::Journals;
use k7_file_service::libraries::Home;
use k7_file_service::paths::resolve_all;
use k7_file_service::revisions::Revisions;
use k7_file_service::rig::rig;
use serde_json::{Value, json};
use song::{DiskSource, Roots};

use super::answering::{TREE_WEIGHT, global as gate};
use super::locks;
use super::stack::{Failed, on_stack};
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::{Acct, Who, claim_root};

const NO_BENCH: &str = "@banc opens one live rig, and nothing else. A shared MODULE or a CHAIN is edited from a project you already have open — its Libraries tab.";

/// Why a read gave no value: the sentence the route sends, or a fault after the rig was rewritten.
enum Fault {
    Refused(String),
    Broken,
}

/// What a bench or a slot needs of the account: its folders, its runtime and the line it logs to.
#[derive(Clone)]
pub struct Run {
    pub home: Home,
    pub code_root: String,
    pub env: BTreeMap<String, String>,
    pub acct: Arc<Acct>,
    pub slug: String,
    pub log: k7_hub::Log,
}

impl Run {
    pub fn of(
        acct: &Arc<Acct>,
        env: &BTreeMap<String, String>,
        slug: &str,
        admin: bool,
        code_root: &Path,
        log: &k7_hub::Log,
    ) -> Run {
        Run {
            home: Home::of(env, admin),
            code_root: code_root.display().to_string(),
            env: env.clone(),
            acct: acct.clone(),
            slug: slug.to_owned(),
            log: log.clone(),
        }
    }

    fn declined(&self, route: &str, why: &str) {
        super::refuse::note(why);
        (self.log)(&format!("[native] {route} declined: {why}"));
    }

    /// `work` with the libraries read through the account's own compile session, on the compiler's stack and under the gate.
    async fn read<R: Send + 'static>(
        &self,
        route: &str,
        wrote: Option<&AtomicBool>,
        work: impl FnOnce(&Libs<'_>) -> Result<R, BenchError> + Send + 'static,
    ) -> Option<Result<R, Fault>> {
        let Some(_held) = gate().take(&self.slug, TREE_WEIGHT).await else {
            self.declined(route, "busy");
            return None;
        };
        let me = self.clone();
        let made = on_stack(move || {
            let songs = me.acct.songs();
            let vocabulary = |roots: &Roots, disk: &DiskSource| {
                let at = roots
                    .confine
                    .clone()
                    .unwrap_or_else(|| roots.project.clone());
                let key = k7_file_service::songs::key_of(&at, roots.paths).ok()?;
                songs
                    .with(&key, roots.paths, |s| s.vocabulary(disk, roots))
                    .ok()?
                    .ok()
            };
            let libs = Libs {
                home: &me.home,
                code_root: &me.code_root,
                env: &me.env,
                vocabulary: &vocabulary,
            };
            work(&libs)
        })
        .await;
        match made {
            Ok(Ok(v)) => Some(Ok(v)),
            Ok(Err(BenchError::Refused(m))) => Some(Err(Fault::Refused(m))),
            Ok(Err(BenchError::Declined(d))) => {
                self.declined(route, d.as_str());
                None
            }
            // Once the rig was rewritten nothing is declined: the half-made rig would pass for an edit.
            Err(Failed::Panicked) => {
                (self.log)(&format!("[native] {route} panicked"));
                wrote
                    .is_some_and(|w| w.load(Ordering::SeqCst))
                    .then_some(Err(Fault::Broken))
            }
            Err(Failed::Lost) => wrote
                .is_some_and(|w| w.load(Ordering::SeqCst))
                .then_some(Err(Fault::Broken)),
        }
    }

    /// `POST /workspaces/bench` for `project`: the status and body, or `None` where it declined.
    pub async fn bench(&self, project: &str) -> Option<(u16, Value)> {
        if bench_chain_of(project).is_none() {
            return Some((404, json!({ "error": NO_BENCH })));
        }
        let (me, name) = (self.clone(), project.to_owned());
        // The gate, then the lock, held by a task of its own: a client that leaves cannot cut the rig's rewrite short.
        let task = super::refuse::spawn(async move {
            let root = workspace_root(&me.home);
            let _gate = Journals::global().exclusive(&resolve_all(&[&root])).await;
            let _lock = locks::write(&me.home.studio).await;
            let wrote = Arc::new(AtomicBool::new(false));
            let flag = wrote.clone();
            let done = me
                .read("POST /workspaces/bench", Some(&wrote), move |libs| {
                    ensure_bench(libs, &name, &flag)
                })
                .await;
            // An editor holding the bench must reload once the rig was rewritten, whatever the answer.
            if wrote.load(Ordering::SeqCst) {
                Revisions::global().bump(&root);
            }
            done
        });
        match task.await.ok()?? {
            Ok(()) => Some((200, opened())),
            // A library that will not mount is the USER's problem to read, so it is a 409 and not a fault.
            Err(Fault::Refused(m)) => Some((409, json!({ "error": m }))),
            Err(Fault::Broken) => Some((500, json!({ "error": "internal error" }))),
        }
    }

    /// `POST /account/slot` for the chain `r`: the status and body, or `None` where it declined.
    pub async fn slot(&self, r: &str) -> Option<(u16, Value)> {
        let rig = match rig(&self.home) {
            Ok(v) => v,
            Err(d) => {
                self.declined("POST /account/slot", d.as_str());
                return None;
            }
        };
        let saved = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let mut raw = [0u8; 6];
        let _ = getrandom::fill(&mut raw);
        let scratch: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let name = r.to_owned();
        let done = self
            .read("POST /account/slot", None, move |libs| {
                slot_of(libs, &rig, &name, &saved, &scratch)
            })
            .await?;
        Some(match done {
            Ok(v) => (200, v),
            Err(Fault::Refused(m)) => (400, json!({ "error": m })),
            Err(Fault::Broken) => (500, json!({ "error": "internal error" })),
        })
    }
}

/// The ids a chain's steps are given: random, as `randomUUID` makes them.
fn fresh_id() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    k7_file_service::media::models::uuid_v4(b)
}

impl Run {
    /// An account write read under the gate, then the lock, held by a task of its own so a client that leaves cannot cut it short.
    /// The error is the sentence and whether the chain was already written.
    async fn account_write<R: Send + 'static>(
        &self,
        route: &'static str,
        work: impl FnOnce(&Libs<'_>, &AtomicBool) -> Result<R, BenchError> + Send + 'static,
    ) -> Option<Result<R, (String, bool)>> {
        let me = self.clone();
        let task = super::refuse::spawn(async move {
            let root = workspace_root(&me.home);
            let _gate = Journals::global().exclusive(&resolve_all(&[&root])).await;
            let _lock = locks::write(&me.home.studio).await;
            let wrote = Arc::new(AtomicBool::new(false));
            let flag = wrote.clone();
            let done = me
                .read(route, Some(&wrote), move |libs| work(libs, &flag))
                .await;
            // An open editor must reload once a chain was written, whatever the answer.
            if wrote.load(Ordering::SeqCst) {
                Revisions::global().bump(&me.home.account_home());
                Revisions::global().bump(&root);
            }
            (done, wrote.load(Ordering::SeqCst))
        });
        let (done, wrote) = task.await.ok()?;
        match done? {
            Ok(v) => Some(Ok(v)),
            Err(Fault::Refused(m)) => Some(Err((m, wrote))),
            Err(Fault::Broken) => Some(Err(("internal error".into(), true))),
        }
    }

    /// The route's answer for a refusal: 400 before a write, 409 once the chain landed, 500 for a fault.
    fn refusal((m, wrote): (String, bool)) -> (u16, Value) {
        let status = match (wrote, m.as_str()) {
            (true, "internal error") => 500,
            (true, _) => 409,
            _ => 400,
        };
        (status, json!({ "error": m }))
    }

    /// `POST /bench/save` for the folder `dir` a verb runs in: the bench's run written as the account's chain.
    pub async fn save(&self, dir: &str, name: Option<&str>) -> Option<(u16, Value)> {
        let (dir, name) = (dir.to_owned(), name.map(str::to_owned));
        let done = self
            .account_write("POST /bench/save", move |libs, wrote| {
                save_bench_chain(libs, &dir, name.as_deref(), &mut fresh_id, wrote)
            })
            .await?;
        Some(match done {
            Ok(s) => (
                200,
                json!({ "ref": s.reference, "steps": s.steps, "outside": s.outside }),
            ),
            Err(e) => Self::refusal(e),
        })
    }

    /// `POST /bench/save` from a Worker, which names its project (`at`, under the bench's workspace) and never a real folder.
    pub async fn save_at(&self, at: &str, name: Option<&str>) -> Option<(u16, Value)> {
        let dir = k7_file_service::paths::join(&[&workspace_root(&self.home), at]);
        self.save(&dir, name).await
    }

    /// `POST /account/frozen-chains/:name/copy`: a library chain as the account's chain `name`.
    pub async fn copy_chain(&self, name: &str, from: &str) -> Option<(u16, Value)> {
        let (name, from) = (name.to_owned(), from.to_owned());
        let done = self
            .account_write("POST /account/frozen-chains/copy", move |libs, wrote| {
                copy_frozen_chain(libs, &from, &name, &mut fresh_id, wrote)
            })
            .await?;
        Some(match done {
            Ok(row) => (200, row),
            Err(e) => Self::refusal(e),
        })
    }

    /// `DELETE /account/frozen-chains/:name`: one of the account's chains removed.
    pub async fn remove_chain(&self, name: &str) -> (u16, Value) {
        let (home, name) = (self.home.clone(), name.to_owned());
        let task = super::refuse::spawn(async move {
            let root = workspace_root(&home);
            let _gate = Journals::global().exclusive(&resolve_all(&[&root])).await;
            let _lock = locks::write(&home.studio).await;
            let at = home.account_home();
            let done = tokio::task::spawn_blocking(move || delete_frozen_chain(&home, &name)).await;
            if matches!(done, Ok(Ok(()))) {
                Revisions::global().bump(&at);
            }
            done
        });
        match task
            .await
            .unwrap_or_else(|_| Ok(Err("internal error".into())))
        {
            Ok(Ok(())) => (200, json!({ "deleted": true })),
            Ok(Err(m)) => (404, json!({ "error": m })),
            Err(_) => (500, json!({ "error": "internal error" })),
        }
    }
}

/// A path segment as Express hands it over: its percent-escapes undone.
fn unescaped(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (
                b.get(i + 1).and_then(|c| hex(*c)),
                b.get(i + 2).and_then(|c| hex(*c)),
            )
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Which of the routes this file answers a request is.
enum Which {
    Bench,
    Slot,
    Save,
    Copy(String),
    Remove(String),
}

/// Whether the route is one this file answers.
pub(super) fn owns(method: &hyper::Method, path: &str) -> bool {
    which(method, path).is_some()
}

fn which(method: &hyper::Method, path: &str) -> Option<Which> {
    let post = method == hyper::Method::POST;
    match path {
        "/workspaces/bench" if post => return Some(Which::Bench),
        "/account/slot" if post => return Some(Which::Slot),
        "/bench/save" if post => return Some(Which::Save),
        _ => {}
    }
    let rest = path.strip_prefix("/account/frozen-chains/")?;
    match rest.strip_suffix("/copy") {
        Some(name) if post && !name.is_empty() && !name.contains('/') => {
            Some(Which::Copy(unescaped(name)))
        }
        None if method == hyper::Method::DELETE && !rest.contains('/') => {
            Some(Which::Remove(unescaped(rest)))
        }
        _ => None,
    }
}

/// `POST /account/slot` as the hub asks it: no request, so no body to hand back; `None` where the compile could not run.
pub async fn slot_for_hub(run: Run, r: &str) -> Option<(u16, Value)> {
    run.slot(r).await
}

/// The hook for the two routes: answered here, or the request handed back whole to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(which) = which(req.method(), path) else {
        return Err(req);
    };
    let Some(code_root) = who.code_root else {
        return Ok(unanswered());
    };
    let account = &who.held.account;
    let run = Run::of(
        &who.held.acct,
        &account.env,
        &account.slug,
        who.admin,
        code_root,
        who.log,
    );
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
    // A delete carries no body; the others are read as JSON.
    let parsed = if matches!(which, Which::Remove(_)) {
        json!({})
    } else {
        match body::json(content_type, &bytes) {
            Ok(b) => b,
            Err(e) => return Ok(json_answer(400, &json!({ "error": e }))),
        }
    };
    // The owner gate stands before every route, after the body is read.
    if !claim_root(&account.home, who.user) {
        return Ok(json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account" }),
        ));
    }
    let field = |k: &str| parsed.get(k).and_then(Value::as_str).map(str::to_owned);
    let done = match which {
        Which::Bench => {
            let project = match parsed.get("project") {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            };
            run.bench(&project).await
        }
        Which::Slot => {
            if let Err(m) = body::check(&parsed, &[("ref", body::Field::Text)]) {
                return Ok(json_answer(400, &json!({ "error": m })));
            }
            match field("ref") {
                Some(r) => run.slot(&r).await,
                None => None,
            }
        }
        Which::Save => {
            let name = match parsed.get("name") {
                None | Some(Value::Null) => None,
                Some(Value::String(n)) => Some(n.as_str()),
                Some(_) => {
                    return Ok(json_answer(
                        400,
                        &json!({ "error": "dir is required, and name is a string" }),
                    ));
                }
            };
            match (
                field("dir").filter(|d| !d.is_empty()),
                field("at").filter(|a| !a.is_empty()),
            ) {
                (Some(dir), _) => run.save(&dir, name).await,
                (None, Some(at)) => run.save_at(&at, name).await,
                _ => Some((
                    400,
                    json!({ "error": "dir is required, and name is a string" }),
                )),
            }
        }
        Which::Copy(name) => {
            if let Err(m) = body::check(&parsed, &[("from", body::Field::Text)]) {
                return Ok(json_answer(400, &json!({ "error": m })));
            }
            match field("from") {
                Some(from) => run.copy_chain(&name, &from).await,
                None => None,
            }
        }
        Which::Remove(name) => Some(run.remove_chain(&name).await),
    };
    Ok(match done {
        Some((status, v)) => json_answer(status, &v),
        None => unanswered(),
    })
}

/// What these routes answer where the native `chains` piece is off or declines: there is no other side (Z2-8).
fn unanswered() -> Response {
    json_answer(
        503,
        &json!({ "error": "the bench and the account's chains need the code root (K7_CODE_ROOT), which this host does not have" }),
    )
}
