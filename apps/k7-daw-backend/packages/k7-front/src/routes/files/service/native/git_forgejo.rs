//! git_forgejo.rs — a Forgejo server behind the forge seam (`docs/files/git.md`, "Forgejo"): its profile, the token
//! endpoint (an authorization code with PKCE, then single-use rotating refresh tokens) and the topic. What a reader gets
//! wrong: the client is public, so no secret is ever sent, and the scopes are asked once, final, since Forgejo refuses
//! a later sign-in that asks for others.

use serde_json::{Value, json};

use super::git_forge::{Entry, Profile, TOPIC, Tokens};
use super::git_forge_api::{Api, Reply};
use super::git_forge_token::now_s;
use super::git_forgejo_servers::Server;

/// The scopes k7 asks, once: the key upload needs the first, the repositories the second.
pub(super) const SCOPES: [&str; 2] = ["write:user", "write:repository"];
/// The one path Forgejo sends the browser back to; the person registers it without a port, which Forgejo then ignores.
pub(super) const CALLBACK: &str = "/api/files/git/forge/callback";
/// The redirect an app shell catches when its sheet cannot end on a loopback page; it forwards the query to `CALLBACK`.
pub(super) const APP_REDIRECT: &str = "rocks.k7.daw://callback";

/// The two redirect addresses the person registers on the server's OAuth2 application.
pub(super) fn redirects() -> [String; 2] {
    [format!("http://127.0.0.1{CALLBACK}"), APP_REDIRECT.into()]
}

pub(super) fn granted() -> Vec<String> {
    SCOPES.iter().map(|s| s.to_string()).collect()
}

/// Why a refresh gave no token: the server refused it (the person signs in again), or it could not be reached.
#[derive(Debug)]
pub(super) enum Refresh {
    Refused,
    Offline(String),
}

/// One Forgejo server: what the device keeps of it, and its profile.
#[derive(Clone, Debug)]
pub(super) struct Forgejo {
    pub server: Server,
    pub profile: Profile,
}

impl Forgejo {
    pub fn new(server: Server) -> Forgejo {
        let ssh_host = match server.ssh_port {
            22 => server.ssh_host.clone(),
            p => format!("{}:{p}", server.ssh_host),
        };
        let profile = Profile {
            id: server.id.clone(),
            kind: "forgejo",
            name: "Forgejo",
            code: "forgejo",
            ssh_host,
            entry: Entry {
                host: k7_git::auth::forge_token_host(&server.id),
                user: "k7",
            },
            api: api_of(&server.url),
            repos_path: format!("/user/repos?limit={}&page=1", server.page_size),
            pages: 10,
            repo_scope: "write:repository",
        };
        Forgejo { server, profile }
    }

    /// The page that asks the person's consent; the browser opens it, never the front.
    pub fn authorize_url(&self, redirect: &str, state: &str, challenge: &str) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.server.client_id)
            .append_pair("redirect_uri", redirect)
            .append_pair("response_type", "code")
            .append_pair("scope", &SCOPES.join(" "))
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .finish();
        format!("{}/login/oauth/authorize?{query}", self.server.url)
    }

    async fn token(&self, body: Value) -> Result<Reply, String> {
        let url = format!("{}/login/oauth/access_token", self.server.url);
        self.profile.api.call("POST", &url, None, Some(body)).await
    }

    /// The code the browser brought back, exchanged with the verifier that only this front holds.
    pub async fn exchange(
        &self,
        code: &str,
        redirect: &str,
        verifier: &str,
    ) -> Result<Tokens, String> {
        let sent = json!({
            "grant_type": "authorization_code", "client_id": self.server.client_id,
            "code": code, "redirect_uri": redirect, "code_verifier": verifier,
        });
        let r = self.token(sent).await?;
        tokens_of(&r).ok_or_else(|| said(&r))
    }

    /// The refresh token spent for a new pair; the caller keeps the new refresh token before using the access token.
    pub async fn refresh(&self, refresh: &str) -> Result<Tokens, Refresh> {
        let sent = json!({
            "grant_type": "refresh_token", "client_id": self.server.client_id,
            "refresh_token": refresh,
        });
        let r = self.token(sent).await.map_err(Refresh::Offline)?;
        tokens_of(&r).ok_or(Refresh::Refused)
    }

    /// `PUT …/topics/k7-workspace` adds the one topic and keeps the others; a collaborator's 403 is silent.
    pub async fn put_topic(&self, token: &str, full: &str) {
        let path = format!("/repos/{full}/topics/{TOPIC}");
        let _ = self.profile.api.send("PUT", token, &path, json!({})).await;
    }
}

/// The REST API under the server's address.
pub(super) fn api_of(url: &str) -> Api {
    Api {
        base: format!("{url}/api/v1"),
        name: "Forgejo",
        accept: "application/json",
    }
}

fn tokens_of(r: &Reply) -> Option<Tokens> {
    if !r.ok() {
        return None;
    }
    let text = |k: &str| r.body.get(k).and_then(Value::as_str).map(str::to_string);
    let ttl = r.body.get("expires_in").and_then(Value::as_u64);
    Some(Tokens {
        access: text("access_token")?,
        refresh: text("refresh_token"),
        expiry: ttl.map(|t| now_s() + t),
    })
}

/// The token endpoint's own sentence (`error_description`), else its code.
fn said(r: &Reply) -> String {
    let text = |k: &str| r.body.get(k).and_then(Value::as_str);
    text("error_description")
        .or_else(|| text("error"))
        .map(str::to_string)
        .unwrap_or_else(|| r.message())
}
