//! seam.rs — what the engine routes ask of this front over the internal seam's file half (`apps/k7/backend-rs/docs/engine/reach.md`):
//! whose a bearer is, and a job of the account's file service.
//! What a reader gets wrong: an answer is cached by the token's hash, a refusal briefly and a subject longer.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{Method, Request, header};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// How long the backend may take to answer one question.
const ASK_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a bearer that verified is believed without asking again.
const SUBJECT_TTL: Duration = Duration::from_secs(30);
/// How long a bearer that did not verify is refused without asking again.
const REFUSAL_TTL: Duration = Duration::from_secs(5);
/// How long the account's file service may take to resolve a chain or compile a slot.
const HUB_JOB_TIMEOUT: Duration = Duration::from_secs(300);
/// Past this many cached bearers the cache starts again empty.
const CACHE_MAX: usize = 4096;

/// A bearer's hash, and what it verified as when.
type Bearers = HashMap<[u8; 32], (Option<String>, Instant)>;

pub struct Seam {
    client: Client<HttpConnector, Body>,
    upstream: String,
    secret: String,
    bearers: Mutex<Bearers>,
}

impl Seam {
    pub fn new(upstream: &str, secret: &str) -> Self {
        Seam {
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
            upstream: upstream.to_string(),
            secret: secret.to_string(),
            bearers: Mutex::new(HashMap::new()),
        }
    }

    /// One question to the file half: the status and the JSON body, Err when the backend did not answer.
    async fn ask(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<(u16, Value), ()> {
        self.ask_at(
            method,
            &format!("/api/internal/files{path}"),
            body,
            ASK_TIMEOUT,
        )
        .await
    }

    /// One question to the seam at `path`, given `timeout` to answer.
    pub async fn ask_at(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        timeout: Duration,
    ) -> Result<(u16, Value), ()> {
        let req = Request::builder()
            .method(method)
            .uri(format!("http://{}{path}", self.upstream))
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-k7-internal", &self.secret)
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .map_err(|_| ())?;
        let ask = async {
            let res = self.client.request(req).await.ok()?;
            let status = res.status().as_u16();
            let bytes = res.into_body().collect().await.ok()?.to_bytes();
            Some((
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            ))
        };
        tokio::time::timeout(timeout, ask)
            .await
            .ok()
            .flatten()
            .ok_or(())
    }

    /// The subject a library bearer verifies as, None when it does not; Err when the backend did not answer.
    pub async fn verify_bearer(&self, token: &str) -> Result<Option<String>, ()> {
        let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let now = Instant::now();
        if let Some((sub, at)) = self.cache().get(&key) {
            let ttl = if sub.is_some() {
                SUBJECT_TTL
            } else {
                REFUSAL_TTL
            };
            if now.duration_since(*at) < ttl {
                return Ok(sub.clone());
            }
        }
        let sub = match self
            .ask(
                Method::POST,
                "/verify-token",
                Some(json!({ "token": token })),
            )
            .await?
        {
            (200, v) => v.get("sub").and_then(Value::as_str).map(str::to_string),
            (400, _) => None,
            _ => return Err(()),
        };
        let mut cache = self.cache();
        if cache.len() >= CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, (sub.clone(), now));
        Ok(sub)
    }

    /// The hub's cold run or slot over the seam (`POST /cold-run`, `/slot`): the answer, or the words of its refusal.
    pub async fn file_hub_job(&self, path: &str, body: Value) -> Result<Value, String> {
        match self
            .ask_at(
                Method::POST,
                &format!("/api/internal/files{path}"),
                Some(body),
                HUB_JOB_TIMEOUT,
            )
            .await
        {
            Ok((200, v)) => Ok(v),
            Ok((_, v)) => Err(v
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("the file hub refused")
                .to_string()),
            Err(()) => Err("k7's backend did not answer for the account's files; try again".into()),
        }
    }

    /// Drop every cached bearer, so a revoked one is asked again at its next use.
    pub fn forget_bearers(&self) {
        self.cache().clear();
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, Bearers> {
        self.bearers.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A file half that verifies `good` as `ann@x` and counts what it is asked.
    async fn file_half() -> (String, Arc<AtomicUsize>) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let asked = Arc::new(AtomicUsize::new(0));
        let count = asked.clone();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                count.fetch_add(1, Ordering::Relaxed);
                let mut buf = vec![0u8; 8192];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let b = if text.contains("\"good\"") {
                    r#"{"sub":"ann@x"}"#
                } else {
                    r#"{"sub":null}"#
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                    b.len()
                );
                let _ = s.write_all(reply.as_bytes()).await;
            }
        });
        (addr, asked)
    }

    #[tokio::test]
    async fn a_bearer_is_asked_once_then_believed_until_forgotten() {
        let (addr, asked) = file_half().await;
        let seam = Seam::new(&addr, "s");
        assert_eq!(seam.verify_bearer("good").await, Ok(Some("ann@x".into())));
        assert_eq!(seam.verify_bearer("good").await, Ok(Some("ann@x".into())));
        assert_eq!(asked.load(Ordering::Relaxed), 1);
        assert_eq!(seam.verify_bearer("forged").await, Ok(None));
        assert_eq!(seam.verify_bearer("forged").await, Ok(None));
        assert_eq!(asked.load(Ordering::Relaxed), 2);
        seam.forget_bearers();
        assert_eq!(seam.verify_bearer("good").await, Ok(Some("ann@x".into())));
        assert_eq!(asked.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn an_unanswered_question_proves_nothing() {
        let gone = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        let seam = Seam::new(&gone.to_string(), "s");
        assert_eq!(seam.verify_bearer("good").await, Err(()));
    }
}
