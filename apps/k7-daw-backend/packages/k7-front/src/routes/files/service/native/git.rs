//! git.rs — the routes that put a workspace under git, connect it to a remote, commit, push, pull and settle a merge
//! (`docs/files/git.md`): the body, the owner and the workspace checked, then `k7-git` answers on a blocking thread.
//! What a reader gets wrong: a refusal is `{ error, code }` and a token or key is read from the body once, kept in the device's
//! keychain and never answered or logged; while a merge waits every write to the workspace is refused (423).

use std::ops::Not;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::Method;
use hyper::header::CONTENT_TYPE;
use k7_capture::Captures;
use k7_capture::landing::Land;
use k7_file_service::libraries::Home;
use k7_file_service::paths::resolve;
use k7_file_service::workspaces::names::workspace_dir;
use k7_git::auth::{self, KnownHosts, Secrets};
use k7_git::{Error, IdentityStore, Net, Origin, Repo};
use serde_json::{Value, json};

use super::git_connect;
use super::git_fetch::Fetchers;
use super::git_forge_routes::{self, Verb};
use super::git_forgejo_flow;
use super::git_forgejo_servers;
use super::git_github;
use super::git_keys;
use super::git_lock;
use super::git_roam;
use super::git_sync;
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::{Who, claim_root, workspace_name_ok};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    State,
    Status,
    Log,
    Connect,
    Clone,
    Commit,
    Push,
    Pull,
    Merge,
    Resolve,
    Abort,
    Key,
    ForgetHost,
    Identity,
    SetIdentity,
    Focus,
    Roam,
    Forges,
    ForgeAdd,
    ForgeProbe,
    /// Where a forge sends the browser back after a sign-in, cookie-less (`git_forgejo_flow.rs`).
    ForgeCallback,
    /// One account's route: its id (`github.com`) and what is asked of it.
    Forge(String, Verb),
}

impl Kind {
    pub fn of(method: &Method, path: &str) -> Option<Kind> {
        let (get, post) = (method == Method::GET, method == Method::POST);
        match path {
            "/git/state" if get => Some(Kind::State),
            "/git/status" if get => Some(Kind::Status),
            "/git/log" if get => Some(Kind::Log),
            "/git/connect" if post => Some(Kind::Connect),
            "/git/clone" if post => Some(Kind::Clone),
            "/git/commit" if post => Some(Kind::Commit),
            "/git/push" if post => Some(Kind::Push),
            "/git/pull" if post => Some(Kind::Pull),
            "/git/merge" if get => Some(Kind::Merge),
            "/git/merge/resolve" if post => Some(Kind::Resolve),
            "/git/merge/abort" if post => Some(Kind::Abort),
            "/git/key" if get => Some(Kind::Key),
            "/git/forget-host" if post => Some(Kind::ForgetHost),
            "/git/identity" if get => Some(Kind::Identity),
            "/git/identity" if post => Some(Kind::SetIdentity),
            "/git/focus" if post => Some(Kind::Focus),
            "/git/roam" if get => Some(Kind::Roam),
            "/git/forges" if get => Some(Kind::Forges),
            "/git/forges" if post => Some(Kind::ForgeAdd),
            "/git/forges/probe" if post => Some(Kind::ForgeProbe),
            "/git/forge/callback" if get => Some(Kind::ForgeCallback),
            _ => Kind::forge(method, path),
        }
    }

    /// `/git/forges/<id>/...`, and `/git/github/...`, which is GitHub's under its first name.
    fn forge(method: &Method, path: &str) -> Option<Kind> {
        if let Some(rest) = path.strip_prefix("/git/github") {
            let verb = Verb::of(method, rest)?;
            return Some(Kind::Forge(git_github::ID.into(), verb));
        }
        let after = path.strip_prefix("/git/forges/")?;
        let (id, rest) = after.find('/').map_or((after, ""), |i| after.split_at(i));
        let verb = Verb::of(method, rest)?;
        git_forge_routes::id_ok(id).then(|| Kind::Forge(id.into(), verb))
    }
}

/// What the routes need of the account: the transports' context, the device's secrets, the identity file, the capture's landing.
#[derive(Clone)]
pub struct Ctx {
    pub net: Arc<Net>,
    pub secrets: Arc<dyn Secrets>,
    pub hosts: Arc<KnownHosts>,
    pub identity: IdentityStore,
    pub workspaces: String,
    /// The account's own folder, where the device keeps what is not a workspace's (`.k7-roam.json`).
    pub account_home: String,
    pub device: String,
    /// The front's own port, which a browser sign-in's loopback redirect names.
    pub port: u16,
    pub land: Option<Land>,
    pub fetchers: Arc<Fetchers>,
    /// The account's capture, to start one on a workspace a clone made.
    pub account: Option<Arc<k7_capture::account::Account>>,
}

pub(super) fn answer_error(e: &Error) -> Response {
    let mut b = json!({ "error": e.to_string(), "code": e.code() });
    let extra = match e {
        Error::UnknownHost { fingerprint } => json!({ "fingerprint": fingerprint }),
        Error::HostKeyChanged { expected, got } => {
            json!({ "expected": expected, "fingerprint": got })
        }
        Error::Unparsable { path, .. } => json!({ "path": path }),
        Error::Unresolved { n } => json!({ "unresolved": n }),
        _ => Value::Null,
    };
    if let (Some(m), Some(x)) = (b.as_object_mut(), extra.as_object()) {
        m.extend(x.clone());
    }
    json_answer(e.status(), &b)
}

pub(super) fn bad(message: &str) -> Response {
    json_answer(400, &json!({ "error": message, "code": "bad-request" }))
}

pub(super) fn field<'a>(b: &'a Value, name: &str) -> Option<&'a str> {
    b.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// The merge waiting for choices refuses a write to its workspace: `None` when the workspace may be written.
pub fn merge_refusal(root: &str) -> Option<Response> {
    Path::new(root).join(".git").join("k7-merge.json").is_file().then(|| {
        json_answer(423, &json!({
            "error": "a merge is waiting for your choices: finish or abort it before editing",
            "code": "merge-pending",
        }))
    })
}

/// A commit, or a pull that commits, while a merge waits: 409, so `HEAD` never moves under it.
pub(super) fn merge_waits(root: &Path) -> Option<Response> {
    Path::new(root).join(".git").join("k7-merge.json").is_file().then(|| {
        json_answer(409, &json!({
            "error": "a merge is waiting for your choices: finish or abort it before committing",
            "code": "merge-pending",
        }))
    })
}

static PORT: AtomicU16 = AtomicU16::new(0);

/// The port the front listens on, told once at its start.
pub fn listening_on(port: u16) {
    PORT.store(port, Ordering::Relaxed);
}

fn device_name() -> String {
    ["HOSTNAME", "COMPUTERNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "this device".to_string())
}

fn nets() -> &'static Mutex<std::collections::HashMap<String, Arc<Net>>> {
    static N: OnceLock<Mutex<std::collections::HashMap<String, Arc<Net>>>> = OnceLock::new();
    N.get_or_init(Default::default)
}

fn fetchers() -> Arc<Fetchers> {
    static F: OnceLock<Arc<Fetchers>> = OnceLock::new();
    F.get_or_init(Default::default).clone()
}

/// The routes' context of one account: its keychain, its trusted servers, its capture.
fn ctx_of(who: &Who<'_>, home: &Home) -> Ctx {
    let account_home = home.account_home();
    let private = PathBuf::from(&account_home).join(".k7-git-secrets");
    let secrets = auth::device_store(private, &who.held.account.slug);
    let hosts = Arc::new(KnownHosts::in_folder(Path::new(&account_home)));
    let net = nets()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .entry(home.studio.clone())
        .or_insert_with(|| {
            Net::new(
                tokio::runtime::Handle::current(),
                crate::trust::client_config(),
                secrets.clone(),
                hosts.clone(),
            )
        })
        .clone();
    let account = Captures::global().account(&home.studio);
    Ctx {
        net,
        secrets,
        hosts,
        identity: IdentityStore::in_folder(Path::new(&account_home)),
        workspaces: workspace_dir(home),
        account_home: home.account_home(),
        device: device_name(),
        port: PORT.load(Ordering::Relaxed),
        land: account.as_ref().map(|a| a.landing()),
        fetchers: fetchers(),
        account,
    }
}

/// A front started with `K7_GIT_ALLOW_FILE` is a test bed (the e2e beds run a server-shaped front), never a deployment.
fn test_bed() -> bool {
    std::env::var_os("K7_GIT_ALLOW_FILE").is_some_and(|v| !v.is_empty())
}

/// The 403 a server front answers: a device's own front is the only one that holds a person's tokens and keys.
pub(super) fn server_refusal(device: bool) -> Option<Response> {
    device.not().then(|| {
        json_answer(
            403,
            &json!({
                "error": "git is for a device's own k7: this server keeps no tokens or keys and calls no address for you",
                "code": "device-only",
            }),
        )
    })
}

/// The hook: one of the git routes answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn live(who: &Who<'_>, path: &str, req: Request) -> Result<Response, Request> {
    let Some(kind) = Kind::of(req.method(), path) else {
        return Err(req);
    };
    if let Some(refused) = server_refusal(who.device || test_bed()) {
        return Ok(refused);
    }
    let home = Home::of(&who.held.account.env);
    if !claim_root(&who.held.account.home, who.user) {
        return Ok(json_answer(
            403,
            &json!({ "error": "these workspaces belong to another account", "code": "forbidden" }),
        ));
    }
    Ok(hook(&ctx_of(who, &home), kind, req).await)
}

pub async fn hook(ctx: &Ctx, kind: Kind, req: Request) -> Response {
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
    let b = match body::json(ctype, &bytes) {
        Ok(b) => b,
        Err(e) => return bad(&e),
    };
    let query = parts.uri.query().unwrap_or("").to_string();
    // Its own task: a client that hangs up never drops a held lock under a running libgit2 call.
    let ctx = ctx.clone();
    match tokio::spawn(async move { dispatch(&ctx, kind, b, query).await }).await {
        Ok(r) => r,
        Err(e) => answer_error(&Error::Io(format!("the git worker stopped: {e}"))),
    }
}

/// Whether the kind reads or writes the repository, so it takes the workspace's lock.
fn locks(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Connect | Kind::Commit | Kind::Push | Kind::Pull | Kind::Resolve | Kind::Abort
    )
}

/// Whether the kind fetches or pushes, so it also takes the remote's lock the background fetch holds.
fn moves_remote_refs(kind: &Kind) -> bool {
    matches!(kind, Kind::Connect | Kind::Push | Kind::Pull)
}

fn busy() -> Response {
    answer_error(&Error::Busy)
}

async fn dispatch(ctx: &Ctx, kind: Kind, b: Value, query: String) -> Response {
    match kind {
        Kind::Identity => return git_keys::identity(ctx),
        Kind::SetIdentity => return git_keys::set_identity(ctx, &b),
        Kind::Key => return git_keys::key(ctx),
        Kind::ForgetHost => return git_keys::forget_host(ctx, &b),
        Kind::Clone => return clone(ctx, &b).await,
        Kind::Roam => return git_roam::list(ctx).await,
        Kind::Forges => return git_forge_routes::list(ctx),
        Kind::ForgeAdd => return git_forgejo_servers::add(ctx, &b).await,
        Kind::ForgeProbe => return git_forgejo_servers::probe_route(&b).await,
        Kind::ForgeCallback => return git_forgejo_flow::callback(ctx, &query).await,
        Kind::Forge(id, verb) => return git_forge_routes::route(ctx, &id, verb, &b).await,
        _ => {}
    }
    let name = if matches!(kind, Kind::State | Kind::Status | Kind::Log | Kind::Merge) {
        body::query(&query, "workspace")
    } else {
        field(&b, "workspace").unwrap_or("").to_string()
    };
    if name.is_empty() || !workspace_name_ok(&name) {
        return bad("a valid workspace is required");
    }
    let root = PathBuf::from(resolve(&format!("{}/{name}", ctx.workspaces), ""));
    if !root.is_dir() {
        return json_answer(
            404,
            &json!({ "error": format!("unknown workspace: {name}"), "code": "unknown-workspace" }),
        );
    }
    let key = root.display().to_string();
    let _held = if locks(&kind) {
        match git_lock::try_take(&key) {
            Some(g) => Some(g),
            None => return busy(),
        }
    } else {
        None
    };
    let _remote = if moves_remote_refs(&kind) {
        match git_lock::try_take_remote(&key) {
            Some(g) => Some(g),
            None => return busy(),
        }
    } else {
        None
    };
    let done = match kind {
        Kind::State => state(ctx, &root).await,
        Kind::Status => status(ctx, &root).await,
        Kind::Log => log(ctx, &root, &query).await,
        Kind::Connect => git_connect::connect(ctx, &root, &b).await,
        Kind::Commit => git_sync::commit(ctx, &root, &b).await,
        Kind::Push => git_sync::push(ctx, &root).await,
        Kind::Pull => git_sync::pull(ctx, &root, &b).await,
        Kind::Merge => git_sync::merge_view(ctx, &root).await,
        Kind::Resolve => git_sync::resolve_merge(ctx, &root, &b).await,
        Kind::Abort => {
            run(ctx, &root, |r| {
                r.merge_abort().map(|_| json!({ "ok": true }))
            })
            .await
        }
        Kind::Focus => git_keys::focus(ctx, &root, &b).await,
        _ => unreachable!("answered above"),
    };
    if matches!(kind, Kind::Connect) && done.is_ok() {
        git_roam::sweep_later(ctx.workspaces.clone(), ctx.account_home.clone());
    }
    done.unwrap_or_else(|e| answer_error(&e))
}

pub(super) type Answer = Result<Response, Error>;

pub(super) fn open(root: &Path) -> Result<Repo, Error> {
    Repo::open(root, &root.join(".git"))
}

/// A libgit2 call on a blocking thread over the workspace's repository, its answer as JSON.
pub(super) async fn run(
    ctx: &Ctx,
    root: &Path,
    f: impl FnOnce(&Repo) -> Result<Value, Error> + Send + 'static,
) -> Answer {
    let root = root.to_path_buf();
    let v = ctx.net.blocking(move || f(&open(&root)?)).await?;
    Ok(json_answer(200, &v))
}

async fn state(ctx: &Ctx, root: &Path) -> Answer {
    if !root.join(".git").exists() {
        return Ok(json_answer(200, &json!({ "mode": "none" })));
    }
    let last = ctx
        .fetchers
        .of(&root.display().to_string())
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .last
        .clone();
    run(ctx, root, move |r| {
        let origin = r.origin_config();
        let tracked = r.ahead_behind()?;
        let fetched = last.as_ref().is_some_and(|o| o.error.is_none());
        let remote_empty = origin.is_some() && fetched && tracked.is_none();
        let (ahead, behind) = match tracked {
            Some(ab) => ab,
            None => (r.head_commits()?, 0),
        };
        let pending = r.merge_pending()?;
        let mut v = json!({
            "mode": if origin.is_some() { "connected" } else { "local" },
            "branch": r.branch(),
            "remote": origin.as_ref().map(|o| o.url.clone()),
            "user": origin.and_then(|o| o.user),
            "ahead": ahead, "behind": behind,
            "changed": r.status()?.iter().filter(|e| !e.too_large).count(),
            "merge": pending.as_ref().map(|p| p.conflicts.iter().filter(|c| c.choice.is_none()).count()),
            "lastFetch": last.map(|o| json!({ "at": o.at.saturating_mul(1000), "error": o.error.map(|(c, m)| json!({ "code": c, "error": m })) })),
        });
        if remote_empty {
            v["remoteEmpty"] = json!(true);
        }
        Ok(v)
    })
    .await
}

async fn status(ctx: &Ctx, root: &Path) -> Answer {
    run(ctx, root, |r| Ok(json!({ "entries": r.status()? }))).await
}

async fn log(ctx: &Ctx, root: &Path, query: &str) -> Answer {
    let limit = body::query(query, "limit")
        .parse()
        .unwrap_or(50usize)
        .min(500);
    let before = Some(body::query(query, "before")).filter(|b| !b.is_empty());
    run(ctx, root, move |r| {
        Ok(json!({ "commits": r.log(limit, before.as_deref())? }))
    })
    .await
}

pub(super) fn origin_of(b: &Value) -> Result<Origin, Error> {
    let url = field(b, "url").ok_or_else(|| Error::BadUrl("an address is required".into()))?;
    Ok(Origin {
        url: url.to_string(),
        user: field(b, "user").map(str::to_string),
    })
}

async fn clone(ctx: &Ctx, b: &Value) -> Response {
    let made = async {
        let origin = origin_of(b)?;
        let name = field(b, "name").ok_or_else(|| Error::BadPath("a name is required".into()))?;
        if !workspace_name_ok(name) {
            return Err(Error::BadPath(format!("invalid workspace name: {name}")));
        }
        let target = PathBuf::from(resolve(&format!("{}/{name}", ctx.workspaces), ""));
        let _held = git_lock::try_take(&target.display().to_string()).ok_or(Error::Busy)?;
        let staged = git_connect::keep_trust(ctx, &origin, b)?;
        if let Err(e) = ctx.net.clone_into(origin, target.clone()).await {
            staged.restore(ctx);
            return Err(e);
        }
        if let Some(a) = &ctx.account
            && let Err(e) = a.attach(&target.display().to_string(), name).await
        {
            return Err(Error::Io(e));
        }
        Ok(json!({ "ok": true, "name": name }))
    };
    match made.await {
        Ok(v) => {
            git_roam::sweep_later(ctx.workspaces.clone(), ctx.account_home.clone());
            json_answer(200, &v)
        }
        Err(e) => answer_error(&e),
    }
}
