//! git_github.rs — GitHub behind the forge seam (`docs/files/git.md`): its profile, the device flow, the scopes from
//! `X-OAuth-Scopes`, github.com's pinned host key and the topic list. What a reader gets wrong: the token keeps its own
//! `github-oauth` keychain entry, so nobody is signed out by the seam; both base URLs are overridable
//! (`K7_GITHUB_WEB_URL`, `K7_GITHUB_API_URL`), honoured on 127.0.0.1, ::1 and localhost only.

use std::collections::HashMap;
use std::sync::{Mutex, Once, OnceLock};

use axum::response::Response;
use serde_json::{Value, json};

use super::git::{Ctx, bad};
use super::git_forge::{Entry, Profile, Step, TOPIC, Tokens, fail};
use super::git_forge_api::{Api, Reply, base};
use super::git_forge_token::fingerprint;
use crate::routes::files::json_answer;

/// GitHub's account id in the routes (`/git/forges/github.com`), and its ssh host.
pub(super) const ID: &str = "github.com";
/// k7's registered OAuth App: a client id is public, so every build carries it and nothing is set to get the sign-in.
const CLIENT_ID: &str = "Ov23liM3s3gY8btYCMSy";

static WEB_SAID: Once = Once::new();
static API_SAID: Once = Once::new();

/// The OAuth App's client id (none turns the sign-in off), GitHub's web address for the device flow, and its profile.
#[derive(Clone, Debug)]
pub(super) struct GitHub {
    pub client_id: Option<String>,
    pub web: String,
    pub profile: Profile,
}

/// The run's variable (empty turns the path off), else k7's own; never read at build, which would rebuild the front.
fn client_id_from(run: Option<String>) -> Option<String> {
    match run {
        Some(v) => Some(v).filter(|v| !v.is_empty()),
        None => Some(CLIENT_ID.to_string()),
    }
}

struct Pending {
    device_code: String,
    interval: u64,
}

fn pending() -> &'static Mutex<HashMap<String, Pending>> {
    static P: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

/// One sign-in at a time per account folder and forge.
fn slot(ctx: &Ctx) -> String {
    format!("{}\n{ID}", ctx.account_home)
}

fn take(ctx: &Ctx) {
    pending()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&slot(ctx));
}

/// The scopes GitHub granted, from `X-OAuth-Scopes`.
pub(super) fn scopes_of(r: &Reply) -> Vec<String> {
    let line = r.scopes.as_deref().unwrap_or("");
    line.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl GitHub {
    /// The client id may be absent: the account is then listed as not offered.
    pub fn new(client_id: Option<String>, web: String, api: String) -> GitHub {
        let profile = Profile {
            id: ID.into(),
            kind: "github",
            name: "GitHub",
            code: "github",
            ssh_host: ID.into(),
            entry: Entry {
                host: "github-oauth".into(),
                user: "k7",
            },
            api: Api {
                base: api,
                name: "GitHub",
                accept: "application/vnd.github+json",
            },
            repos_path: "/user/repos?sort=pushed&per_page=100".into(),
            pages: 10,
            repo_scope: "repo",
        };
        GitHub {
            client_id,
            web,
            profile,
        }
    }

    pub fn from_env() -> GitHub {
        GitHub::new(
            client_id_from(std::env::var("K7_GITHUB_CLIENT_ID").ok()),
            base("K7_GITHUB_WEB_URL", "https://github.com", &WEB_SAID),
            base("K7_GITHUB_API_URL", "https://api.github.com", &API_SAID),
        )
    }

    async fn login(&self, path: &str, body: Value) -> Result<Reply, String> {
        let url = format!("{}{path}", self.web);
        self.profile.api.call("POST", &url, None, Some(body)).await
    }

    /// `POST /git/forges/github.com/start { scopes }`: the code the person types on github.com; the device code stays here.
    pub async fn start(&self, ctx: &Ctx, b: &Value) -> Response {
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
        let sent = json!({ "client_id": self.client_id, "scope": scopes.join(" ") });
        let r = match self.login("/login/device/code", sent).await {
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
            slot(ctx),
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

    /// `POST /git/forges/github.com/poll`: one ask of GitHub; the token, when there is one, ends the sign-in.
    pub async fn poll(&self, ctx: &Ctx) -> Step {
        let (device_code, interval) = {
            let map = pending().lock().unwrap_or_else(|p| p.into_inner());
            match map.get(&slot(ctx)) {
                Some(p) => (p.device_code.clone(), p.interval),
                None => return Step::Answer(json_answer(200, &json!({ "state": "expired" }))),
            }
        };
        let sent = json!({
            "client_id": self.client_id, "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        });
        let r = match self.login("/login/oauth/access_token", sent).await {
            Ok(r) => r,
            Err(e) => return Step::Answer(fail(502, "offline", &e)),
        };
        if let Some(token) = r.body.get("access_token").and_then(Value::as_str) {
            take(ctx);
            return Step::Token(Tokens {
                access: token.to_string(),
                refresh: None,
                expiry: None,
            });
        }
        let state =
            |s: &str, interval: u64| json_answer(200, &json!({ "state": s, "interval": interval }));
        Step::Answer(match r.body.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => state("pending", interval),
            Some("slow_down") => {
                let mut map = pending().lock().unwrap_or_else(|p| p.into_inner());
                let more = interval + 5;
                if let Some(p) = map.get_mut(&slot(ctx)) {
                    p.interval = more;
                }
                state("slow_down", more)
            }
            Some(done @ ("expired_token" | "access_denied")) => {
                take(ctx);
                let said = if done == "expired_token" {
                    "expired"
                } else {
                    "denied"
                };
                state(said, interval)
            }
            _ => fail(502, "github", &r.message()),
        })
    }

    /// Trusts github.com's ed25519 host key, the one the SSH client meets first; a different listed key is never replaced.
    pub async fn pin_host_keys(&self, ctx: &Ctx) {
        let Ok(r) = self.profile.api.get_open("/meta").await else {
            return;
        };
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
            let _ = ctx.hosts.remember(ID, 22, &fp);
        }
    }

    /// A new repository has no topic yet: the list is just k7's.
    pub async fn tag_new(&self, token: &str, full: &str) {
        let path = format!("/repos/{full}/topics");
        let names = json!({ "names": [TOPIC] });
        let _ = self.profile.api.send("PUT", token, &path, names).await;
    }

    /// GitHub replaces the whole list, so it is read first and k7's topic added to it.
    pub async fn tag_existing(&self, token: &str, repo: &str) {
        let path = format!("/repos/{repo}/topics");
        let Ok(r) = self.profile.api.get(token, &path).await else {
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
        let sent = json!({ "names": names });
        let _ = self.profile.api.send("PUT", token, &path, sent).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{CLIENT_ID, client_id_from};

    #[test]
    fn github_client_id_is_k7s_unless_the_run_says_otherwise() {
        assert_eq!(client_id_from(None).as_deref(), Some(CLIENT_ID));
        assert_eq!(client_id_from(Some("run".into())).as_deref(), Some("run"));
        assert_eq!(client_id_from(Some(String::new())), None);
    }
}
