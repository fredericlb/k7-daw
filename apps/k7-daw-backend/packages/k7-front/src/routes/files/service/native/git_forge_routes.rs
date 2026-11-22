//! git_forge_routes.rs — the routes every forge shares (`docs/files/git.md`): `/git/forges`, one account's status,
//! the end of a device-flow sign-in, the repository pages, creation, the topic, the sign-out, a server's removal.
//! What a reader gets wrong: the token is read from the keychain on each call, refreshed there when it expires, and
//! never answered nor logged.

use axum::response::Response;
use hyper::Method;
use serde_json::{Value, json};

use super::git::{Ctx, bad, field};
use super::git_forge::{
    Forge, Saved, Step, Tokens, accounts, drop_token, fail, find, repo_of, saved, sign_in_again,
};
use super::git_forge_api::{Reply, next_page};
use super::git_forge_token::{ensure_key, ensure_keys_later, finish, fresh, unmark};
use super::git_forgejo_servers;
use crate::routes::files::json_answer;

/// What a route asks of one account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Status,
    Start,
    Poll,
    Repos,
    Create,
    Signout,
    Remove,
}

impl Verb {
    /// The verb of the path after the account's id (`""`, `/start`, ...).
    pub fn of(method: &Method, rest: &str) -> Option<Verb> {
        let (get, post) = (method == Method::GET, method == Method::POST);
        match rest {
            "" if get => Some(Verb::Status),
            "/start" if post => Some(Verb::Start),
            "/poll" if post => Some(Verb::Poll),
            "/repos" if get => Some(Verb::Repos),
            "/repos" if post => Some(Verb::Create),
            "/signout" if post => Some(Verb::Signout),
            "/remove" if post => Some(Verb::Remove),
            _ => None,
        }
    }
}

/// An account's id as a path segment: a host name, nothing that could leave the segment.
pub fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
}

fn gone(f: &Forge) -> Response {
    let p = f.profile();
    fail(
        404,
        &format!("{}-unavailable", p.code),
        &format!("this build of k7 has no {} sign-in", p.name),
    )
}

pub(super) async fn route(ctx: &Ctx, id: &str, verb: Verb, b: &Value) -> Response {
    match find(ctx, id) {
        Some(f) => answer(ctx, &f, verb, b).await,
        None => fail(404, "unknown-forge", &format!("no account here is {id}")),
    }
}

/// One verb on one forge: a forge this build cannot sign in to answers its status, its sign-out and its removal only.
pub(super) async fn answer(ctx: &Ctx, f: &Forge, verb: Verb, b: &Value) -> Response {
    if !f.available() && !matches!(verb, Verb::Status | Verb::Signout | Verb::Remove) {
        return gone(f);
    }
    match verb {
        Verb::Status => status(ctx, f),
        Verb::Start => f.start(ctx, b).await,
        Verb::Poll => poll(ctx, f).await,
        Verb::Repos => repos(ctx, f).await,
        Verb::Create => create(ctx, f, b).await,
        Verb::Signout => signout(ctx, f),
        Verb::Remove => git_forgejo_servers::remove(ctx, f),
    }
}

/// Whether signing out, or removing the server, reaches the person's other Apple devices too.
fn synced(ctx: &Ctx, f: &Forge) -> bool {
    let e = &f.profile().entry;
    let list = matches!(f, Forge::Forgejo(_)) && ctx.roam.available();
    list || ctx.secrets.is_synced(&e.host, e.user)
}

fn status_of(ctx: &Ctx, f: &Forge) -> Value {
    let synced = synced(ctx, f);
    if !f.available() {
        return json!({ "available": false, "scopes": [], "synced": synced });
    }
    match saved(ctx, f) {
        Some(s) => {
            json!({ "available": true, "login": s.login, "scopes": s.scopes, "synced": synced })
        }
        None => json!({ "available": true, "scopes": [], "synced": synced }),
    }
}

/// `GET /git/forges/<id>`: whether the sign-in is offered and who is signed in; no network call.
pub(super) fn status(ctx: &Ctx, f: &Forge) -> Response {
    json_answer(200, &status_of(ctx, f))
}

/// `GET /git/forges`: every account this device offers, each with its status; no network call is waited on.
pub(super) fn list(ctx: &Ctx) -> Response {
    let all = accounts(ctx);
    ensure_keys_later(ctx, &all);
    super::git_author::refresh_later(ctx, &all);
    let forges: Vec<Value> = all
        .iter()
        .map(|f| {
            let p = f.profile();
            let mut v = status_of(ctx, f);
            v["id"] = json!(p.id);
            v["kind"] = json!(p.kind);
            v["name"] = json!(p.name);
            if let (Some(m), Some(x)) = (v.as_object_mut(), f.extra().as_object()) {
                m.extend(x.clone());
            }
            v
        })
        .collect();
    json_answer(200, &json!({ "forges": forges }))
}

/// `POST /git/forges/<id>/poll`: one step of the sign-in; a token ends it here.
pub(super) async fn poll(ctx: &Ctx, f: &Forge) -> Response {
    match f.poll(ctx).await {
        Step::Answer(r) => r,
        Step::Token(t) => signed_in(ctx, f, t).await,
    }
}

/// The token is kept, then the key is uploaded and the host trusted, before the answer.
async fn signed_in(ctx: &Ctx, f: &Forge, t: Tokens) -> Response {
    match finish(ctx, f, t).await {
        Ok(d) => {
            let mut out = json!({ "state": "ok", "login": d.login, "scopes": d.scopes });
            if let Some(n) = d.note {
                out["keyNote"] = json!(n);
            }
            json_answer(200, &out)
        }
        Err((status, code, said)) => fail(status, &code, &said),
    }
}

/// A token the forge refuses is gone for good: it is dropped and the page asks to sign in again.
fn refused(ctx: &Ctx, f: &Forge, r: &Reply, name: Option<&str>) -> Response {
    let p = f.profile();
    if r.status == 401 {
        let _ = drop_token(ctx, f);
        return sign_in_again(f);
    }
    if r.status == 409 {
        let said = format!(
            "{} is already taken on {}",
            name.unwrap_or("this name"),
            p.id
        );
        return fail(409, "taken", &said);
    }
    let code = if r.status == 422 { "invalid" } else { p.code };
    let detail = r.body.pointer("/errors/0/message").and_then(Value::as_str);
    let message = match detail {
        Some(d) => format!("{} ({d})", r.message()),
        None => r.message(),
    };
    fail(if r.status == 422 { 422 } else { 502 }, code, &message)
}

/// The token if the person is signed in with the repository scope, refreshed when it expires, else the answer to give.
#[allow(
    clippy::result_large_err,
    reason = "the refusal is the answer to give, built once"
)]
async fn with_repo(ctx: &Ctx, f: &Forge) -> Result<Saved, Response> {
    let p = f.profile();
    let Some(s) = saved(ctx, f) else {
        return Err(fail(
            401,
            &format!("{}-signout", p.code),
            &format!("sign in with {} first", p.name),
        ));
    };
    if !s.scopes.iter().any(|x| x == p.repo_scope) {
        return Err(fail(
            403,
            "scope",
            &format!("{}'s repositories need one more permission", p.name),
        ));
    }
    let s = fresh(ctx, f, s).await?;
    ensure_key(ctx, f).await;
    Ok(s)
}

/// `GET /git/forges/<id>/repos`: the account's repositories in the forge's order, with their topics.
pub(super) async fn repos(ctx: &Ctx, f: &Forge) -> Response {
    let s = match with_repo(ctx, f).await {
        Ok(s) => s,
        Err(r) => return r,
    };
    let api = &f.profile().api;
    let mut list = Vec::new();
    let mut next = Some(f.profile().repos_path.clone());
    for _ in 0..f.profile().pages {
        let Some(at) = next.take() else { break };
        let got = if at.starts_with('/') {
            api.get(&s.token, &at).await
        } else {
            api.get_url(&s.token, &at).await
        };
        let r = match got {
            Ok(r) if r.ok() => r,
            Ok(r) => return refused(ctx, f, &r, None),
            Err(e) => return fail(502, "offline", &e),
        };
        list.extend(r.body.as_array().into_iter().flatten().filter_map(row));
        next = r.link.as_deref().and_then(next_page);
    }
    json_answer(200, &json!({ "repos": list }))
}

fn row(r: &Value) -> Option<Value> {
    let topics: Vec<&str> = r
        .get("topics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let pushed = r.get("pushed_at").or_else(|| r.get("updated_at"));
    Some(json!({
        "fullName": r.get("full_name")?, "sshUrl": r.get("ssh_url")?,
        "private": r.get("private").and_then(Value::as_bool).unwrap_or(false),
        "pushedAt": pushed, "k7": topics.contains(&super::git_forge::TOPIC),
    }))
}

/// `POST /git/forges/<id>/repos { name, private }`: a new repository, marked as a k7 workspace.
pub(super) async fn create(ctx: &Ctx, f: &Forge, b: &Value) -> Response {
    let s = match with_repo(ctx, f).await {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(name) = field(b, "name") else {
        return bad("a name is required");
    };
    let private = b.get("private").and_then(Value::as_bool).unwrap_or(true);
    let sent = match f {
        Forge::GitHub(_) => json!({ "name": name, "private": private }),
        Forge::Forgejo(_) => json!({
            "name": name, "private": private, "auto_init": false,
            "default_branch": "main", "description": "A k7 workspace",
        }),
    };
    let api = &f.profile().api;
    let made = match api.send("POST", &s.token, "/user/repos", sent).await {
        Ok(r) if r.ok() => r,
        Ok(r) => return refused(ctx, f, &r, Some(name)),
        Err(e) => return fail(502, "offline", &e),
    };
    let text = |k: &str| {
        made.body
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let full = text("full_name");
    f.tag_new(&s.token, &full).await;
    json_answer(200, &json!({ "sshUrl": text("ssh_url"), "fullName": full }))
}

/// `POST /git/forges/<id>/signout`: the token goes; the uploaded key stays, it is the device's git access.
pub(super) fn signout(ctx: &Ctx, f: &Forge) -> Response {
    unmark(ctx, f);
    match drop_token(ctx, f) {
        Ok(()) => json_answer(200, &json!({ "ok": true })),
        Err(e) => fail(500, "keychain", &format!("the device's keychain: {e}")),
    }
}

/// Connecting an existing repository of the forge marks it as a k7 workspace; best effort, a failure (no admin) is silent.
pub(super) async fn tag_topic(ctx: &Ctx, f: &Forge, url: &str) {
    let Some(repo) = repo_of(url, &f.profile().ssh_host) else {
        return;
    };
    if let Ok(s) = with_repo(ctx, f).await {
        f.tag_existing(&s.token, &repo).await;
    }
}

/// After a connect: the topic on the account whose forge the address names, on a task of its own.
pub(super) fn tag_later(ctx: &Ctx, url: &str) {
    for f in accounts(ctx).into_iter().filter(Forge::available) {
        if repo_of(url, &f.profile().ssh_host).is_none() {
            continue;
        }
        let (c, url) = (ctx.clone(), url.to_string());
        tokio::spawn(async move { tag_topic(&c, &f, &url).await });
    }
}
