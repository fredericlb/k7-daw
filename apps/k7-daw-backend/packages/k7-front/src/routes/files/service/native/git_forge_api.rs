//! git_forge_api.rs — the few HTTPS calls to a forge's REST API (keys, repositories, topics, a sign-in's endpoints) on the
//! front's own client, shared by every forge. What a reader gets wrong: a token only ever talks to the REST API; git itself
//! stays SSH, and a base address taken from the environment is honoured on 127.0.0.1, ::1 and localhost only.

use std::sync::Once;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde_json::{Value, json};

/// One forge's REST API: its base address, its name in sentences and the media type it answers.
#[derive(Clone, Debug)]
pub(super) struct Api {
    pub base: String,
    pub name: &'static str,
    pub accept: &'static str,
}

/// What the forge answered: the status, the two headers the routes read, and the JSON body (null when there is none).
pub(super) struct Reply {
    pub status: u16,
    pub scopes: Option<String>,
    pub link: Option<String>,
    pub body: Value,
    pub from: &'static str,
}

impl Reply {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The forge's own sentence for a refusal, or a plain one.
    pub fn message(&self) -> String {
        self.body
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{} answered {}", self.from, self.status))
    }
}

/// Whether an override points at this machine (127.0.0.1, ::1, localhost): the only place a test's fake may be.
pub(super) fn is_loopback(url: &str) -> bool {
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
pub(super) fn base(var: &str, default: &str, said: &Once) -> String {
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

impl Api {
    pub async fn get(&self, token: &str, path: &str) -> Result<Reply, String> {
        self.call("GET", &format!("{}{path}", self.base), Some(token), None)
            .await
    }

    /// A call without a token, such as a forge's published metadata.
    pub async fn get_open(&self, path: &str) -> Result<Reply, String> {
        self.call("GET", &format!("{}{path}", self.base), None, None)
            .await
    }

    /// A `Link` header's next page, taken only when it stays on the API's own address.
    pub async fn get_url(&self, token: &str, url: &str) -> Result<Reply, String> {
        if !url.starts_with(&self.base) {
            return Err(format!("{} pointed to another address", self.name));
        }
        self.call("GET", url, Some(token), None).await
    }

    pub async fn send(
        &self,
        method: &str,
        token: &str,
        path: &str,
        body: Value,
    ) -> Result<Reply, String> {
        self.call(
            method,
            &format!("{}{path}", self.base),
            Some(token),
            Some(body),
        )
        .await
    }

    /// Any address of the forge's (its web sign-in endpoints too), with this API's name and media type.
    pub async fn call(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Result<Reply, String> {
        let name = self.name;
        let mut b = hyper::Request::builder()
            .method(method)
            .uri(url)
            .header(USER_AGENT, "k7")
            .header(ACCEPT, self.accept);
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
                .map_err(|e| format!("{name} cannot be reached: {e}"))?;
            let text = |h: &str| {
                res.headers()
                    .get(h)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string)
            };
            let (status, scopes, link) =
                (res.status().as_u16(), text("x-oauth-scopes"), text("link"));
            let bytes = res
                .into_body()
                .collect()
                .await
                .map_err(|e| format!("{name} cut the answer short: {e}"))?
                .to_bytes();
            let body = serde_json::from_slice(&bytes).unwrap_or(json!(null));
            Ok(Reply {
                status,
                scopes,
                link,
                body,
                from: name,
            })
        };
        tokio::time::timeout(Duration::from_secs(20), work)
            .await
            .map_err(|_| format!("{name} did not answer in time"))?
    }
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
    use super::is_loopback;

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
}
