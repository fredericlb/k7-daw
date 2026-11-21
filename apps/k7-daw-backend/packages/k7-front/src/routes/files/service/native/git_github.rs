//! git_github.rs — "Sign in with GitHub" (`docs/files/git.md`): the device flow, the token in the device's keychain, the
//! device's key uploaded and GitHub's host keys trusted, then the repository list and creation. What a reader gets wrong:
//! the token never syncs and is never answered, and git itself stays SSH; the token only calls the REST API.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use k7_git::auth;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::git::{Ctx, bad, field};
use super::git_github_api::{Hub, Reply, next_page};
use crate::routes::files::json_answer;

/// The topic that marks a repository as a k7 workspace.
pub(super) const TOPIC: &str = "k7-workspace";
/// The keychain entry of the token: its host is not the SSH key's, so it is never the synced one.
const TOKEN_HOST: &str = "github-oauth";
const TOKEN_USER: &str = "k7";
/// Ten pages of a hundred: the picker is not a mirror of the account.
const PAGES: usize = 10;

#[derive(Serialize, Deserialize)]
struct Saved {
    token: String,
    login: String,
    scopes: Vec<String>,
}

struct Pending {
    device_code: String,
    interval: u64,
}

fn pending() -> &'static Mutex<HashMap<String, Pending>> {
    static P: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

fn fail(status: u16, code: &str, message: &str) -> Response {
    json_answer(status, &json!({ "error": message, "code": code }))
}

fn gone() -> Response {
    fail(
        404,
        "github-unavailable",
        "this build of k7 has no GitHub sign-in",
    )
}

fn saved(ctx: &Ctx) -> Option<Saved> {
    let text = ctx.secrets.get(TOKEN_HOST, TOKEN_USER).ok()??;
    serde_json::from_str(&text).ok()
}

fn keep(ctx: &Ctx, s: &Saved) -> std::io::Result<()> {
    ctx.secrets
        .set(TOKEN_HOST, TOKEN_USER, &json!(s).to_string())
}

/// `GET /git/github`: whether the path is offered and who is signed in; no network call.
pub(super) fn status(ctx: &Ctx, hub: Option<&Hub>) -> Response {
    if hub.is_none() {
        return json_answer(200, &json!({ "available": false, "scopes": [] }));
    }
    match saved(ctx) {
        Some(s) => json_answer(
            200,
            &json!({ "available": true, "login": s.login, "scopes": s.scopes }),
        ),
        None => json_answer(200, &json!({ "available": true, "scopes": [] })),
    }
}

/// `POST /git/github/start { scopes }`: the code the person types on github.com; the device code stays here.
pub(super) async fn start(ctx: &Ctx, hub: Option<&Hub>, b: &Value) -> Response {
    let Some(hub) = hub else { return gone() };
    let scopes: Vec<&str> = b
        .get("scopes")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if scopes.is_empty()
        || scopes.iter().any(|s| {
            !s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '_')
        })
    {
        return bad("scopes are required");
    }
    let sent = json!({ "client_id": hub.client_id, "scope": scopes.join(" ") });
    let r = match hub.login("/login/device/code", sent).await {
        Ok(r) if r.ok() && r.body.get("device_code").is_some() => r,
        Ok(r) => return fail(502, "github", &r.message()),
        Err(e) => return fail(502, "offline", &e),
    };
    let text = |k: &str| {
        r.body
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let num = |k: &str, d: u64| r.body.get(k).and_then(Value::as_u64).unwrap_or(d);
    let interval = num("interval", 5);
    pending().lock().unwrap_or_else(|p| p.into_inner()).insert(
        ctx.account_home.clone(),
        Pending {
            device_code: text("device_code"),
            interval,
        },
    );
    json_answer(
        200,
        &json!({
            "userCode": text("user_code"), "verificationUri": text("verification_uri"),
            "interval": interval, "expiresIn": num("expires_in", 900),
        }),
    )
}

/// `POST /git/github/poll`: one ask of GitHub; on `ok` the token is kept, the key uploaded and the host keys trusted first.
pub(super) async fn poll(ctx: &Ctx, hub: Option<&Hub>) -> Response {
    let Some(hub) = hub else { return gone() };
    let (device_code, interval) = {
        let map = pending().lock().unwrap_or_else(|p| p.into_inner());
        match map.get(&ctx.account_home) {
            Some(p) => (p.device_code.clone(), p.interval),
            None => return json_answer(200, &json!({ "state": "expired" })),
        }
    };
    let sent = json!({
        "client_id": hub.client_id, "device_code": device_code,
        "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
    });
    let r = match hub.login("/login/oauth/access_token", sent).await {
        Ok(r) => r,
        Err(e) => return fail(502, "offline", &e),
    };
    if let Some(token) = r.body.get("access_token").and_then(Value::as_str) {
        pending()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&ctx.account_home);
        return signed_in(ctx, hub, token).await;
    }
    let state =
        |s: &str, interval: u64| json_answer(200, &json!({ "state": s, "interval": interval }));
    match r.body.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => state("pending", interval),
        Some("slow_down") => {
            let mut map = pending().lock().unwrap_or_else(|p| p.into_inner());
            let more = interval + 5;
            if let Some(p) = map.get_mut(&ctx.account_home) {
                p.interval = more;
            }
            state("slow_down", more)
        }
        Some(done @ ("expired_token" | "access_denied")) => {
            pending()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&ctx.account_home);
            state(
                if done == "expired_token" {
                    "expired"
                } else {
                    "denied"
                },
                interval,
            )
        }
        _ => fail(502, "github", &r.message()),
    }
}

/// The scopes GitHub granted, from `X-OAuth-Scopes`.
fn scopes_of(r: &Reply) -> Vec<String> {
    let line = r.scopes.as_deref().unwrap_or("");
    line.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

async fn signed_in(ctx: &Ctx, hub: &Hub, token: &str) -> Response {
    let me = match hub.get(token, "/user").await {
        Ok(r) if r.ok() => r,
        Ok(r) => return fail(502, "github", &r.message()),
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
        scopes: scopes_of(&me),
    };
    if let Err(e) = keep(ctx, &s) {
        return fail(500, "keychain", &format!("the device's keychain: {e}"));
    }
    let note = upload_key(ctx, hub, token).await;
    pin_host_keys(ctx, hub).await;
    let mut out = json!({ "state": "ok", "login": login, "scopes": s.scopes });
    if let Some(n) = note {
        out["keyNote"] = json!(n);
    }
    json_answer(200, &out)
}

/// The type and the base64 body of a public key line: what GitHub compares, without the comment.
fn key_body(line: &str) -> String {
    line.split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Puts the device's public key on the account unless it is there; a sentence when it cannot be.
async fn upload_key(ctx: &Ctx, hub: &Hub, token: &str) -> Option<String> {
    let public = auth::public_key(&*ctx.secrets).ok()?;
    let ours = key_body(&public);
    let listed = match hub.get(token, "/user/keys?per_page=100").await {
        Ok(r) if r.ok() => r,
        Ok(r) => return Some(r.message()),
        Err(e) => return Some(e),
    };
    let there = listed.body.as_array().is_some_and(|a| {
        a.iter().any(|k| {
            k.get("key")
                .and_then(Value::as_str)
                .is_some_and(|k| key_body(k) == ours)
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
    match hub.send("POST", token, "/user/keys", json!({ "title": title, "key": public })).await {
        Ok(r) if r.ok() => None,
        Ok(r) if r.status == 422 => Some("this device's key is already used on another GitHub account, so it was not added to this one".into()),
        Ok(r) => Some(r.message()),
        Err(e) => Some(e),
    }
}

/// `SHA256:` and the unpadded base64 of the hash of the key's blob, as the SSH client names a key.
fn fingerprint(line: &str) -> Option<String> {
    let blob = STANDARD.decode(line.split_whitespace().nth(1)?).ok()?;
    Some(format!(
        "SHA256:{}",
        STANDARD_NO_PAD.encode(Sha256::digest(blob))
    ))
}

/// Trusts github.com's ed25519 host key, the one the SSH client meets first; a different listed key is never replaced.
async fn pin_host_keys(ctx: &Ctx, hub: &Hub) {
    let Ok(r) = hub.meta().await else { return };
    let keys = r
        .body
        .get("ssh_keys")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let line = keys
        .iter()
        .filter_map(Value::as_str)
        .find(|k| k.starts_with("ssh-ed25519"));
    if let Some(fp) = line.and_then(fingerprint) {
        let _ = ctx.hosts.remember("github.com", 22, &fp);
    }
}

/// A token GitHub refuses is gone for good: it is dropped and the page asks to sign in again.
fn refused(ctx: &Ctx, r: &Reply) -> Response {
    if r.status == 401 {
        let _ = ctx.secrets.delete(TOKEN_HOST, TOKEN_USER);
        return fail(
            401,
            "github-signout",
            "GitHub no longer accepts this sign-in: sign in again",
        );
    }
    let code = if r.status == 422 { "invalid" } else { "github" };
    let detail = r.body.pointer("/errors/0/message").and_then(Value::as_str);
    let message = match detail {
        Some(d) => format!("{} ({d})", r.message()),
        None => r.message(),
    };
    fail(if r.status == 422 { 422 } else { 502 }, code, &message)
}

/// The token if the person is signed in with `repo`, else the answer to give.
#[allow(
    clippy::result_large_err,
    reason = "the refusal is the answer to give, built once"
)]
fn with_repo(ctx: &Ctx) -> Result<Saved, Response> {
    let Some(s) = saved(ctx) else {
        return Err(fail(401, "github-signout", "sign in with GitHub first"));
    };
    if s.scopes.iter().any(|x| x == "repo") {
        Ok(s)
    } else {
        Err(fail(
            403,
            "scope",
            "GitHub's repositories need one more permission",
        ))
    }
}

/// `GET /git/github/repos`: the account's repositories, most recently pushed first, with their topics.
pub(super) async fn repos(ctx: &Ctx, hub: Option<&Hub>) -> Response {
    let Some(hub) = hub else { return gone() };
    let s = match with_repo(ctx) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let mut list = Vec::new();
    let mut next = Some("/user/repos?sort=pushed&per_page=100".to_string());
    for _ in 0..PAGES {
        let Some(at) = next.take() else { break };
        let got = if at.starts_with('/') {
            hub.get(&s.token, &at).await
        } else {
            hub.get_url(&s.token, &at).await
        };
        let r = match got {
            Ok(r) if r.ok() => r,
            Ok(r) => return refused(ctx, &r),
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
        "pushedAt": r.get("pushed_at"), "k7": topics.contains(&TOPIC),
    }))
}

/// `POST /git/github/repos { name, private }`: a new repository, marked as a k7 workspace.
pub(super) async fn create(ctx: &Ctx, hub: Option<&Hub>, b: &Value) -> Response {
    let Some(hub) = hub else { return gone() };
    let s = match with_repo(ctx) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(name) = field(b, "name") else {
        return bad("a name is required");
    };
    let private = b.get("private").and_then(Value::as_bool).unwrap_or(true);
    let made = match hub
        .send(
            "POST",
            &s.token,
            "/user/repos",
            json!({ "name": name, "private": private }),
        )
        .await
    {
        Ok(r) if r.ok() => r,
        Ok(r) => return refused(ctx, &r),
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
    let _ = hub
        .send(
            "PUT",
            &s.token,
            &format!("/repos/{full}/topics"),
            json!({ "names": [TOPIC] }),
        )
        .await;
    json_answer(200, &json!({ "sshUrl": text("ssh_url"), "fullName": full }))
}

/// `POST /git/github/signout`: the token goes; the uploaded key stays, it is the device's git access.
pub(super) fn signout(ctx: &Ctx) -> Response {
    match ctx.secrets.delete(TOKEN_HOST, TOKEN_USER) {
        Ok(()) => json_answer(200, &json!({ "ok": true })),
        Err(e) => fail(500, "keychain", &format!("the device's keychain: {e}")),
    }
}

/// `owner/repo` of a github.com ssh address, `git@github.com:o/r.git` or `ssh://git@github.com/o/r`.
pub(super) fn repo_of(url: &str) -> Option<String> {
    let path = match url.strip_prefix("ssh://") {
        Some(rest) => {
            rest.split_once('/')
                .filter(|(a, _)| a.rsplit('@').next() == Some("github.com"))?
                .1
        }
        None => {
            url.split_once(':')
                .filter(|(a, _)| a.rsplit('@').next() == Some("github.com"))?
                .1
        }
    };
    let path = path.trim_matches('/').trim_end_matches(".git");
    (path.matches('/').count() == 1).then(|| path.to_string())
}

/// Connecting an existing repository marks it as a k7 workspace; best effort, a failure (no admin) is silent.
pub(super) async fn tag_topic(ctx: &Ctx, hub: &Hub, url: &str) {
    let (Some(repo), Ok(s)) = (repo_of(url), with_repo(ctx)) else {
        return;
    };
    let Ok(r) = hub.get(&s.token, &format!("/repos/{repo}/topics")).await else {
        return;
    };
    if !r.ok() {
        return;
    }
    let mut names: Vec<String> = r
        .body
        .get("names")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| n.as_str().map(str::to_string))
        .collect();
    if names.iter().any(|n| n == TOPIC) {
        return;
    }
    names.push(TOPIC.to_string());
    let _ = hub
        .send(
            "PUT",
            &s.token,
            &format!("/repos/{repo}/topics"),
            json!({ "names": names }),
        )
        .await;
}
