//! git_forge.rs — the forge seam (`docs/files/git.md`, "Forges"): what differs per forge is a `Profile` of data and the
//! few `Forge` methods matched per variant (the sign-in, the refresh, the host-key policy, the key lookup, the topic);
//! every route over it is shared (`git_forge_routes.rs`). What a reader gets wrong: a forge is one account's server,
//! GitHub built in and each Forgejo server from the device's own list, and its token sits in its own keychain entry,
//! never the synced one, never answered.

use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::git::Ctx;
use super::git_forge_api::{Api, Reply};
use super::git_forgejo::{Forgejo, Refresh};
use super::git_forgejo_flow;
use super::git_forgejo_servers;
use super::git_github::GitHub;
use crate::routes::files::json_answer;

/// The topic that marks a repository as a k7 workspace.
pub(super) const TOPIC: &str = "k7-workspace";

/// The keychain entry of one account's token: its host is never the SSH key's, so it is never the synced one.
#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub host: String,
    pub user: &'static str,
}

/// What differs per forge as plain data.
#[derive(Clone, Debug)]
pub(super) struct Profile {
    /// The route's id (`/git/forges/<id>`): the forge's host, with its port when it is not the scheme's.
    pub id: String,
    /// The kind the page draws (`github`, `forgejo`).
    pub kind: &'static str,
    /// The forge's name in sentences.
    pub name: &'static str,
    /// The prefix of its refusal codes (`<code>-signout`, `<code>-unavailable`).
    pub code: &'static str,
    /// The host its repositories' ssh addresses name, `host:port` when the port is not 22.
    pub ssh_host: String,
    pub entry: Entry,
    pub api: Api,
    /// The repository list's first page, its page size included.
    pub repos_path: String,
    /// How many pages the picker reads at most: it is not a mirror of the account.
    pub pages: usize,
    /// The scope the repository routes need.
    pub repo_scope: &'static str,
}

/// One account's forge; a new forge is a new variant here, its own file beside `git_github.rs` and `git_forgejo.rs`.
#[derive(Clone, Debug)]
pub(super) enum Forge {
    GitHub(GitHub),
    Forgejo(Forgejo),
}

/// What a sign-in or a refresh gave: the access token and, where the forge rotates it, the refresh token and the expiry.
pub(super) struct Tokens {
    pub access: String,
    pub refresh: Option<String>,
    /// Seconds since the epoch.
    pub expiry: Option<u64>,
}

/// What one step of a sign-in gave: an answer for the page, or the token that ends it.
pub(super) enum Step {
    Answer(Response),
    Token(Tokens),
}

/// The accounts this device offers, signed in or not: GitHub, then the Forgejo servers it was given.
pub(super) fn accounts(ctx: &Ctx) -> Vec<Forge> {
    let servers = git_forgejo_servers::list(ctx).into_iter();
    std::iter::once(Forge::GitHub(GitHub::from_env()))
        .chain(servers.map(|s| Forge::Forgejo(Forgejo::new(s))))
        .collect()
}

pub(super) fn find(ctx: &Ctx, id: &str) -> Option<Forge> {
    accounts(ctx).into_iter().find(|f| f.profile().id == id)
}

impl Forge {
    pub fn profile(&self) -> &Profile {
        match self {
            Forge::GitHub(g) => &g.profile,
            Forge::Forgejo(g) => &g.profile,
        }
    }

    /// Whether this build can sign in to it at all; signing out works either way.
    pub fn available(&self) -> bool {
        match self {
            Forge::GitHub(g) => g.client_id.is_some(),
            Forge::Forgejo(g) => !g.server.client_id.is_empty(),
        }
    }

    pub async fn start(&self, ctx: &Ctx, b: &Value) -> Response {
        match self {
            Forge::GitHub(g) => g.start(ctx, b).await,
            Forge::Forgejo(g) => git_forgejo_flow::start(ctx, g, b),
        }
    }

    pub async fn poll(&self, ctx: &Ctx) -> Step {
        match self {
            Forge::GitHub(g) => g.poll(ctx).await,
            Forge::Forgejo(g) => Step::Answer(git_forgejo_flow::poll(ctx, g)),
        }
    }

    /// The scopes the token carries, read from the account's `/user` answer.
    pub fn granted(&self, me: &Reply) -> Vec<String> {
        match self {
            Forge::GitHub(_) => super::git_github::scopes_of(me),
            Forge::Forgejo(_) => super::git_forgejo::granted(),
        }
    }

    /// A new access token from a refresh token, for a forge whose tokens expire.
    pub async fn refresh(&self, refresh: &str) -> Result<Tokens, Refresh> {
        match self {
            Forge::GitHub(_) => Err(Refresh::Refused),
            Forge::Forgejo(g) => g.refresh(refresh).await,
        }
    }

    /// The sentence when the forge refuses the device's key because another account holds it.
    pub fn key_elsewhere(&self) -> String {
        match self {
            Forge::GitHub(_) => "this device's key is already used on another GitHub account, so it was not added to this one".into(),
            Forge::Forgejo(g) => format!("This key belongs to another account on {}", g.server.id),
        }
    }

    /// What the account list shows beside the status: a Forgejo server's address.
    pub fn extra(&self) -> Value {
        match self {
            Forge::GitHub(_) => json!({}),
            Forge::Forgejo(g) => json!({ "url": g.server.url }),
        }
    }

    /// The host-key policy once signed in: a forge that publishes its keys has them trusted here, else the first connect asks.
    pub async fn trust_host(&self, ctx: &Ctx) {
        match self {
            Forge::GitHub(g) => g.pin_host_keys(ctx).await,
            Forge::Forgejo(_) => {}
        }
    }

    /// The address that lists the account's keys, narrowed to this fingerprint where the forge can.
    pub fn keys_path(&self, fingerprint: &str) -> String {
        match self {
            Forge::GitHub(_) => "/user/keys?per_page=100".into(),
            Forge::Forgejo(_) => {
                let fp: String =
                    url::form_urlencoded::byte_serialize(fingerprint.as_bytes()).collect();
                format!("/user/keys?fingerprint={fp}")
            }
        }
    }

    /// Marks a repository this route just made.
    pub async fn tag_new(&self, token: &str, full: &str) {
        match self {
            Forge::GitHub(g) => g.tag_new(token, full).await,
            Forge::Forgejo(g) => g.put_topic(token, full).await,
        }
    }

    /// Marks an existing repository, keeping its other topics; best effort.
    pub async fn tag_existing(&self, token: &str, full: &str) {
        match self {
            Forge::GitHub(g) => g.tag_existing(token, full).await,
            Forge::Forgejo(g) => g.put_topic(token, full).await,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct Saved {
    pub token: String,
    pub login: String,
    pub scopes: Vec<String>,
    /// A rotating forge's refresh token; GitHub's entries have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<String>,
    /// When the access token expires, in seconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
}

pub(super) fn saved(ctx: &Ctx, f: &Forge) -> Option<Saved> {
    let e = &f.profile().entry;
    let text = ctx.secrets.get(&e.host, e.user).ok()??;
    serde_json::from_str(&text).ok()
}

pub(super) fn keep(ctx: &Ctx, f: &Forge, s: &Saved) -> std::io::Result<()> {
    let e = &f.profile().entry;
    ctx.secrets.set(&e.host, e.user, &json!(s).to_string())
}

pub(super) fn drop_token(ctx: &Ctx, f: &Forge) -> std::io::Result<()> {
    let e = &f.profile().entry;
    ctx.secrets.delete(&e.host, e.user)
}

pub(super) fn fail(status: u16, code: &str, message: &str) -> Response {
    json_answer(status, &json!({ "error": message, "code": code }))
}

/// The answer once the forge no longer takes the token: the page shows "Sign in again".
pub(super) fn sign_in_again(f: &Forge) -> Response {
    let p = f.profile();
    fail(
        401,
        &format!("{}-signout", p.code),
        &format!("{} no longer accepts this sign-in: sign in again", p.name),
    )
}

/// `owner/repo` of an ssh address on `host`, `git@host:o/r.git` or `ssh://git@host/o/r`.
pub(super) fn repo_of(url: &str, host: &str) -> Option<String> {
    let on_host = |a: &str| a.rsplit('@').next() == Some(host);
    let path = match url.strip_prefix("ssh://") {
        Some(rest) => rest.split_once('/').filter(|(a, _)| on_host(a))?.1,
        None => url.split_once(':').filter(|(a, _)| on_host(a))?.1,
    };
    let path = path.trim_matches('/').trim_end_matches(".git");
    (path.matches('/').count() == 1).then(|| path.to_string())
}
