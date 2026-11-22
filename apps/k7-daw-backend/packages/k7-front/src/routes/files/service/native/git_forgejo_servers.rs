//! git_forgejo_servers.rs — the Forgejo servers this device signs in to (`docs/files/git.md`, "Forgejo"): the list in
//! the account's own folder (`.k7-forges.json`, never synced), the probe, the add and the removal. What a reader gets
//! wrong: the probe needs no token, and a server is kept only once its API answered as Forgejo's.

use std::path::Path;

use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::git::{Ctx, bad, field};
use super::git_forge::{Forge, drop_token, fail};
use super::git_forge_routes::id_ok;
use super::git_forgejo::{SCOPES, api_of, redirects};
use crate::routes::files::json_answer;

const FILE: &str = ".k7-forges.json";

/// One server as the device keeps it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Server {
    /// `host`, or `host:port` when the port is not the scheme's: the route's id.
    pub id: String,
    /// The address the person gave, without a trailing slash: the web pages and, under `/api/v1`, the API.
    pub url: String,
    /// The OAuth2 application the person registered on the server.
    pub client_id: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    /// The server's `max_response_items`: Forgejo clamps a larger page without saying so.
    pub page_size: u32,
}

#[derive(Default, Serialize, Deserialize)]
struct Kept {
    servers: Vec<Server>,
}

pub(super) fn list(ctx: &Ctx) -> Vec<Server> {
    let at = Path::new(&ctx.account_home).join(FILE);
    let text = std::fs::read_to_string(at).unwrap_or_default();
    serde_json::from_str::<Kept>(&text)
        .map(|k| k.servers)
        .unwrap_or_default()
}

fn store(ctx: &Ctx, servers: Vec<Server>) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(&Kept { servers }).map_err(std::io::Error::other)?;
    crate::atomic::write_atomic(Path::new(&ctx.account_home), FILE, &text)
}

/// The base address and the id of what the person typed: http or https, a host, no credentials, query nor fragment.
pub(super) fn address(typed: &str) -> Result<(String, String, String), String> {
    let t = typed.trim();
    let with = if t.contains("://") {
        t.to_string()
    } else {
        format!("https://{t}")
    };
    let no = || format!("{t} is not a server's address");
    let u = url::Url::parse(&with).map_err(|_| no())?;
    let plain = u.username().is_empty() && u.password().is_none();
    if !matches!(u.scheme(), "http" | "https")
        || !plain
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err(no());
    }
    let host = u.host_str().ok_or_else(no)?.to_ascii_lowercase();
    let id = match u.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.clone(),
    };
    if !id_ok(&id) {
        return Err(no());
    }
    let base = format!(
        "{}{}",
        u.origin().ascii_serialization(),
        u.path().trim_end_matches('/')
    );
    Ok((base, id, host))
}

/// What the server says of itself without a token: Forgejo's version, its page size, LFS, and its own address.
#[allow(
    clippy::result_large_err,
    reason = "the refusal is the answer to give, built once"
)]
async fn probe(base: &str, id: &str) -> Result<Value, Response> {
    let api = api_of(base);
    let version = match api.get_open("/version").await {
        Ok(r) if r.ok() && r.body.get("version").is_some() => r.body["version"].clone(),
        Ok(_) => {
            return Err(fail(
                502,
                "not-forgejo",
                &format!("{id} does not answer as a Forgejo server"),
            ));
        }
        Err(e) => return Err(fail(502, "offline", &e)),
    };
    let read = |path: &'static str| {
        let api = api.clone();
        async move {
            api.get_open(path)
                .await
                .ok()
                .filter(|r| r.ok())
                .map(|r| r.body)
        }
    };
    let page = read("/settings/api").await;
    let page_size = page
        .as_ref()
        .and_then(|b| b.get("max_response_items"))
        .and_then(Value::as_u64)
        .unwrap_or(50);
    let repo = read("/settings/repository").await;
    let lfs_off = repo
        .as_ref()
        .and_then(|b| b.get("lfs_disabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let found = format!("{base}/.well-known/openid-configuration");
    let issuer = match api.call("GET", &found, None, None).await {
        Ok(r) if r.ok() => r
            .body
            .get("issuer")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    };
    let root = issuer.map_or(base.to_string(), |i| i.trim_end_matches('/').to_string());
    // LFS goes over HTTP(S) to the server's own ROOT_URL, which may not be the address the person typed.
    let root_answers =
        root == base || matches!(api_of(&root).get_open("/version").await, Ok(r) if r.ok());
    Ok(json!({
        "id": id, "url": base, "version": version, "pageSize": page_size, "lfs": !lfs_off,
        "rootUrl": root, "rootAnswers": root_answers, "redirects": redirects(), "scopes": SCOPES,
    }))
}

/// `POST /git/forges/probe { url }`: what the server is and the two redirects to register; nothing is kept.
pub(super) async fn probe_route(b: &Value) -> Response {
    let Some(typed) = field(b, "url") else {
        return bad("an address is required");
    };
    match address(typed) {
        Ok((base, id, _)) => match probe(&base, &id).await {
            Ok(v) => json_answer(200, &v),
            Err(r) => r,
        },
        Err(e) => fail(400, "bad-address", &e),
    }
}

fn host_ok(h: &str) -> bool {
    !h.is_empty()
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

/// `POST /git/forges { url, clientId, sshHost?, sshPort? }`: the server probed, then kept; a new client id signs out.
pub(super) async fn add(ctx: &Ctx, b: &Value) -> Response {
    let (Some(typed), Some(client_id)) = (field(b, "url"), field(b, "clientId")) else {
        return bad("an address and a client id are required");
    };
    if !client_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return bad("the client id is the OAuth2 application's, as Forgejo shows it");
    }
    let (base, id, host) = match address(typed) {
        Ok(a) => a,
        Err(e) => return fail(400, "bad-address", &e),
    };
    let ssh_host = field(b, "sshHost").unwrap_or(&host).to_ascii_lowercase();
    let ssh_port = b.get("sshPort").and_then(Value::as_u64).unwrap_or(22);
    if !host_ok(&ssh_host) || !(1..=65535).contains(&ssh_port) {
        return bad("the ssh host or port is not valid");
    }
    let mut v = match probe(&base, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let server = Server {
        id: id.clone(),
        url: base,
        client_id: client_id.to_string(),
        ssh_host,
        ssh_port: ssh_port as u16,
        page_size: v["pageSize"].as_u64().unwrap_or(50) as u32,
    };
    let mut servers = list(ctx);
    let before = servers.iter().position(|s| s.id == id);
    if let Some(i) = before {
        let old = servers.remove(i);
        if old.client_id != server.client_id {
            let _ = drop_token(ctx, &Forge::Forgejo(super::git_forgejo::Forgejo::new(old)));
        }
    }
    servers.push(server);
    if let Err(e) = store(ctx, servers) {
        return fail(500, "io", &format!("the server list was not kept: {e}"));
    }
    v["added"] = json!(true);
    json_answer(200, &v)
}

/// `POST /git/forges/<id>/remove`: the token and the server go; the uploaded key stays on the server.
pub(super) fn remove(ctx: &Ctx, f: &Forge) -> Response {
    let Forge::Forgejo(g) = f else {
        return fail(400, "built-in", "GitHub is built in: sign out instead");
    };
    if let Err(e) = drop_token(ctx, f) {
        return fail(500, "keychain", &format!("the device's keychain: {e}"));
    }
    let servers = list(ctx)
        .into_iter()
        .filter(|s| s.id != g.server.id)
        .collect();
    match store(ctx, servers) {
        Ok(()) => json_answer(200, &json!({ "ok": true })),
        Err(e) => fail(500, "io", &format!("the server list was not kept: {e}")),
    }
}
