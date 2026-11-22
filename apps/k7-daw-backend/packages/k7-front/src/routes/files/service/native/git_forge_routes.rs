//! git_forge_routes.rs — the routes every forge shares (`docs/files/git.md`): `/git/forges`, one account's status,
//! the end of a sign-in (the key uploaded, the host trusted), the repository pages, creation, the topic, the sign-out.
//! What a reader gets wrong: the token is read from the keychain on each call and never answered nor logged.

use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use hyper::Method;
use k7_git::auth;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::git::{Ctx, bad, field};
use super::git_forge::{
    Forge, Saved, Step, accounts, drop_token, fail, find, keep, repo_of, saved,
};
use super::git_forge_api::{Reply, next_page};
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
    match find(id) {
        Some(f) => answer(ctx, &f, verb, b).await,
        None => fail(404, "unknown-forge", &format!("no account here is {id}")),
    }
}

/// One verb on one forge: a forge this build cannot sign in to answers its status and its sign-out only.
pub(super) async fn answer(ctx: &Ctx, f: &Forge, verb: Verb, b: &Value) -> Response {
    if !f.available() && !matches!(verb, Verb::Status | Verb::Signout) {
        return gone(f);
    }
    match verb {
        Verb::Status => status(ctx, f),
        Verb::Start => f.start(ctx, b).await,
        Verb::Poll => poll(ctx, f).await,
        Verb::Repos => repos(ctx, f).await,
        Verb::Create => create(ctx, f, b).await,
        Verb::Signout => signout(ctx, f),
    }
}

fn status_of(ctx: &Ctx, f: &Forge) -> Value {
    if !f.available() {
        return json!({ "available": false, "scopes": [] });
    }
    match saved(ctx, f) {
        Some(s) => json!({ "available": true, "login": s.login, "scopes": s.scopes }),
        None => json!({ "available": true, "scopes": [] }),
    }
}

/// `GET /git/forges/<id>`: whether the sign-in is offered and who is signed in; no network call.
pub(super) fn status(ctx: &Ctx, f: &Forge) -> Response {
    json_answer(200, &status_of(ctx, f))
}

/// `GET /git/forges`: every account this build offers, each with its status; no network call.
pub(super) fn list(ctx: &Ctx) -> Response {
    let forges: Vec<Value> = accounts()
        .iter()
        .map(|f| {
            let p = f.profile();
            let mut v = status_of(ctx, f);
            v["id"] = json!(p.id);
            v["kind"] = json!(p.kind);
            v["name"] = json!(p.name);
            v
        })
        .collect();
    json_answer(200, &json!({ "forges": forges }))
}

/// `POST /git/forges/<id>/poll`: one step of the sign-in; a token ends it here.
pub(super) async fn poll(ctx: &Ctx, f: &Forge) -> Response {
    match f.poll(ctx).await {
        Step::Answer(r) => r,
        Step::Token(t) => signed_in(ctx, f, &t).await,
    }
}

/// The token is kept, then the key is uploaded and the host trusted, before the answer.
pub(super) async fn signed_in(ctx: &Ctx, f: &Forge, token: &str) -> Response {
    let p = f.profile();
    let me = match p.api.get(token, "/user").await {
        Ok(r) if r.ok() => r,
        Ok(r) => return fail(502, p.code, &r.message()),
        Err(e) => return fail(502, "offline", &e),
    };
    let login = me
        .body
        .get("login")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let s = Saved {
        token: token.to_string(),
        login: login.clone(),
        scopes: f.granted(&me),
    };
    if let Err(e) = keep(ctx, f, &s) {
        return fail(500, "keychain", &format!("the device's keychain: {e}"));
    }
    let note = upload_key(ctx, f, token).await;
    f.trust_host(ctx).await;
    let mut out = json!({ "state": "ok", "login": login, "scopes": s.scopes });
    if let Some(n) = note {
        out["keyNote"] = json!(n);
    }
    json_answer(200, &out)
}

/// `SHA256:` and the unpadded base64 of the hash of the key's blob, as the SSH client names a key.
pub(super) fn fingerprint(line: &str) -> Option<String> {
    let blob = STANDARD.decode(line.split_whitespace().nth(1)?).ok()?;
    Some(format!(
        "SHA256:{}",
        STANDARD_NO_PAD.encode(Sha256::digest(blob))
    ))
}

/// Puts the device's public key on the account unless it is there; a sentence when it cannot be.
async fn upload_key(ctx: &Ctx, f: &Forge, token: &str) -> Option<String> {
    let p = f.profile();
    let public = auth::public_key(&*ctx.secrets).ok()?;
    let ours = fingerprint(&public)?;
    let listed = match p.api.get(token, &f.keys_path(&ours)).await {
        Ok(r) if r.ok() => r,
        Ok(r) => return Some(r.message()),
        Err(e) => return Some(e),
    };
    let there = listed.body.as_array().is_some_and(|a| {
        a.iter().any(|k| {
            k.get("key")
                .and_then(Value::as_str)
                .and_then(fingerprint)
                .is_some_and(|k| k == ours)
        })
    });
    if there {
        return None;
    }
    let title = if auth::key_is_synced(&*ctx.secrets) {
        "k7".to_string()
    } else {
        format!("k7 — {}", ctx.device)
    };
    let sent = json!({ "title": title, "key": public });
    match p.api.send("POST", token, "/user/keys", sent).await {
        Ok(r) if r.ok() => None,
        Ok(r) if r.status == 422 => Some(format!(
            "this device's key is already used on another {} account, so it was not added to this one",
            p.name
        )),
        Ok(r) => Some(r.message()),
        Err(e) => Some(e),
    }
}

/// A token the forge refuses is gone for good: it is dropped and the page asks to sign in again.
fn refused(ctx: &Ctx, f: &Forge, r: &Reply) -> Response {
    let p = f.profile();
    if r.status == 401 {
        let _ = drop_token(ctx, f);
        return fail(
            401,
            &format!("{}-signout", p.code),
            &format!("{} no longer accepts this sign-in: sign in again", p.name),
        );
    }
    let code = if r.status == 422 { "invalid" } else { p.code };
    let detail = r.body.pointer("/errors/0/message").and_then(Value::as_str);
    let message = match detail {
        Some(d) => format!("{} ({d})", r.message()),
        None => r.message(),
    };
    fail(if r.status == 422 { 422 } else { 502 }, code, &message)
}

/// The token if the person is signed in with the repository scope, else the answer to give.
#[allow(
    clippy::result_large_err,
    reason = "the refusal is the answer to give, built once"
)]
fn with_repo(ctx: &Ctx, f: &Forge) -> Result<Saved, Response> {
    let p = f.profile();
    let Some(s) = saved(ctx, f) else {
        return Err(fail(
            401,
            &format!("{}-signout", p.code),
            &format!("sign in with {} first", p.name),
        ));
    };
    if s.scopes.iter().any(|x| x == p.repo_scope) {
        Ok(s)
    } else {
        Err(fail(
            403,
            "scope",
            &format!("{}'s repositories need one more permission", p.name),
        ))
    }
}

/// `GET /git/forges/<id>/repos`: the account's repositories in the forge's order, with their topics.
pub(super) async fn repos(ctx: &Ctx, f: &Forge) -> Response {
    let s = match with_repo(ctx, f) {
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
            Ok(r) => return refused(ctx, f, &r),
            Err(e) => return fail(502, "offline", &e),
        };
        list.extend(r.body.as_array().into_iter().flatten().filter_map(row));
        next = r.link.as_deref().and_then(next_page);
    }
    json_answer(200, &json!({ "repos": list }))
}

fn row(r: &Value) -> Option<Value> {
    let topics: Vec<&str> = r
        .get("topics")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    Some(json!({
        "fullName": r.get("full_name")?, "sshUrl": r.get("ssh_url")?,
        "private": r.get("private").and_then(Value::as_bool).unwrap_or(false),
        "pushedAt": r.get("pushed_at"), "k7": topics.contains(&super::git_forge::TOPIC),
    }))
}

/// `POST /git/forges/<id>/repos { name, private }`: a new repository, marked as a k7 workspace.
pub(super) async fn create(ctx: &Ctx, f: &Forge, b: &Value) -> Response {
    let s = match with_repo(ctx, f) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(name) = field(b, "name") else {
        return bad("a name is required");
    };
    let private = b.get("private").and_then(Value::as_bool).unwrap_or(true);
    let sent = json!({ "name": name, "private": private });
    let made = match f
        .profile()
        .api
        .send("POST", &s.token, "/user/repos", sent)
        .await
    {
        Ok(r) if r.ok() => r,
        Ok(r) => return refused(ctx, f, &r),
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
    match drop_token(ctx, f) {
        Ok(()) => json_answer(200, &json!({ "ok": true })),
        Err(e) => fail(500, "keychain", &format!("the device's keychain: {e}")),
    }
}

/// Connecting an existing repository of the forge marks it as a k7 workspace; best effort, a failure (no admin) is silent.
pub(super) async fn tag_topic(ctx: &Ctx, f: &Forge, url: &str) {
    let (Some(repo), Ok(s)) = (repo_of(url, &f.profile().ssh_host), with_repo(ctx, f)) else {
        return;
    };
    f.tag_existing(&s.token, &repo).await;
}

/// After a connect: the topic on the account whose forge the address names, on a task of its own.
pub(super) fn tag_later(ctx: &Ctx, url: &str) {
    for f in accounts().into_iter().filter(Forge::available) {
        if repo_of(url, &f.profile().ssh_host).is_none() {
            continue;
        }
        let (c, url) = (ctx.clone(), url.to_string());
        tokio::spawn(async move { tag_topic(&c, &f, &url).await });
    }
}
