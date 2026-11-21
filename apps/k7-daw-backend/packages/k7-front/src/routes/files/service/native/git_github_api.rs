//! git_github_api.rs — the few HTTPS calls to GitHub (the device flow, the account's keys, its repositories) on the front's own
//! client. What a reader gets wrong: a token only ever talks to the REST API; git itself stays SSH, and both base URLs are
//! overridable (`K7_GITHUB_WEB_URL`, `K7_GITHUB_API_URL`), honoured on 127.0.0.1, ::1 and localhost only.

use std::sync::Once;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde_json::{Value, json};

static WEB_SAID: Once = Once::new();
static API_SAID: Once = Once::new();

/// The OAuth App's public client id and the two places GitHub answers.
#[derive(Clone, Debug)]
pub(super) struct Hub {
    pub client_id: String,
    pub web: String,
    pub api: String,
}

/// What GitHub answered: the status, the two headers the routes read, and the JSON body (null when there is none).
pub(super) struct Reply {
    pub status: u16,
    pub scopes: Option<String>,
    pub link: Option<String>,
    pub body: Value,
}

impl Reply {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// GitHub's own sentence for a refusal, or a plain one.
    pub fn message(&self) -> String {
        self.body
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("GitHub answered {}", self.status))
    }
}

/// k7's registered OAuth App: a client id is public, so every build carries it and nothing is set to get the sign-in.
const CLIENT_ID: &str = "Ov23liM3s3gY8btYCMSy";

/// The run's variable (empty turns the path off), else k7's own; never read at build, which would rebuild the front.
fn client_id_from(run: Option<String>) -> Option<String> {
    match run {
        Some(v) => Some(v).filter(|v| !v.is_empty()),
        None => Some(CLIENT_ID.to_string()),
    }
}

fn client_id() -> Option<String> {
    client_id_from(std::env::var("K7_GITHUB_CLIENT_ID").ok())
}

/// Whether an override points at this machine (127.0.0.1, ::1, localhost): the only place a test's fake may be.
fn is_loopback(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => authority.split(':').next().unwrap_or(""),
    };
    matches!(
        host.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "::1" | "localhost"
    )
}

/// The override when it stays on this machine, else the default: a stray variable never sends the token elsewhere.
fn base(var: &str, default: &str, said: &Once) -> String {
    let v = std::env::var(var).ok().filter(|s| !s.is_empty());
    let kept = match v {
        Some(v) if is_loopback(&v) => v,
        Some(_) => {
            said.call_once(|| {
                eprintln!("[git] {var} ignored: only 127.0.0.1, ::1 and localhost are accepted")
            });
            default.to_string()
        }
        None => default.to_string(),
    };
    kept.trim_end_matches('/').to_string()
}

impl Hub {
    /// `None` when this build has no client id: the GitHub path is then not offered at all.
    pub fn from_env() -> Option<Hub> {
        Some(Hub {
            client_id: client_id()?,
            web: base("K7_GITHUB_WEB_URL", "https://github.com", &WEB_SAID),
            api: base("K7_GITHUB_API_URL", "https://api.github.com", &API_SAID),
        })
    }

    pub async fn login(&self, path: &str, body: Value) -> Result<Reply, String> {
        send("POST", &format!("{}{path}", self.web), None, Some(body)).await
    }

    pub async fn get(&self, token: &str, path: &str) -> Result<Reply, String> {
        send("GET", &format!("{}{path}", self.api), Some(token), None).await
    }

    /// A `Link` header's next page, taken only when it stays on the API's own address.
    pub async fn get_url(&self, token: &str, url: &str) -> Result<Reply, String> {
        if !url.starts_with(&self.api) {
            return Err("GitHub pointed to another address".into());
        }
        send("GET", url, Some(token), None).await
    }

    pub async fn send(
        &self,
        method: &str,
        token: &str,
        path: &str,
        body: Value,
    ) -> Result<Reply, String> {
        send(
            method,
            &format!("{}{path}", self.api),
            Some(token),
            Some(body),
        )
        .await
    }

    pub async fn meta(&self) -> Result<Reply, String> {
        send("GET", &format!("{}/meta", self.api), None, None).await
    }
}

async fn send(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Result<Reply, String> {
    let mut b = hyper::Request::builder()
        .method(method)
        .uri(url)
        .header(USER_AGENT, "k7")
        .header(ACCEPT, "application/vnd.github+json");
    if let Some(t) = token {
        b = b.header(AUTHORIZATION, format!("Bearer {t}"));
    }
    let payload = match body {
        Some(v) => {
            b = b.header(CONTENT_TYPE, "application/json");
            Bytes::from(v.to_string())
        }
        None => Bytes::new(),
    };
    let req = b.body(Full::new(payload)).map_err(|e| e.to_string())?;
    let work = async {
        let res = crate::https::client::<Full<Bytes>>()
            .request(req)
            .await
            .map_err(|e| format!("GitHub cannot be reached: {e}"))?;
        let text = |name: &str| {
            res.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let (status, scopes, link) = (res.status().as_u16(), text("x-oauth-scopes"), text("link"));
        let bytes = res
            .into_body()
            .collect()
            .await
            .map_err(|e| format!("GitHub cut the answer short: {e}"))?
            .to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(json!(null));
        Ok(Reply {
            status,
            scopes,
            link,
            body,
        })
    };
    tokio::time::timeout(Duration::from_secs(20), work)
        .await
        .map_err(|_| "GitHub did not answer in time".to_string())?
}

/// The `rel="next"` address of a `Link` header.
pub(super) fn next_page(link: &str) -> Option<String> {
    link.split(',').find_map(|part| {
        let (url, rel) = part.split_once(';')?;
        rel.contains("rel=\"next\"").then(|| {
            url.trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{CLIENT_ID, client_id_from, is_loopback};

    #[test]
    fn github_base_override_is_taken_only_on_this_machine() {
        for ok in [
            "http://127.0.0.1:9000",
            "http://localhost:1/x",
            "http://[::1]:80",
            "https://127.0.0.1",
        ] {
            assert!(is_loopback(ok), "{ok}");
        }
        for no in [
            "https://evil.example",
            "http://localhost.evil.example",
            "http://127.0.0.1@evil.example",
            "http://127.0.0.1.evil.example",
            "ftp://127.0.0.1",
            "127.0.0.1:9000",
            "",
        ] {
            assert!(!is_loopback(no), "{no}");
        }
    }

    #[test]
    fn github_client_id_is_k7s_unless_the_run_says_otherwise() {
        assert_eq!(client_id_from(None).as_deref(), Some(CLIENT_ID));
        assert_eq!(client_id_from(Some("run".into())).as_deref(), Some("run"));
        assert_eq!(client_id_from(Some(String::new())), None);
    }
}
