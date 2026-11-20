//! seam.rs — what the engine routes ask of this front over the internal seam's file half (`apps/k7/backend-rs/docs/engine/reach.md`):
//! a job of the account's file service.

use std::time::Duration;

use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{Method, Request, header};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use serde_json::Value;

/// How long the account's file service may take to resolve a chain or compile a slot.
const HUB_JOB_TIMEOUT: Duration = Duration::from_secs(300);

pub struct Seam {
    client: Client<HttpConnector, Body>,
    upstream: String,
    secret: String,
}

impl Seam {
    pub fn new(upstream: &str, secret: &str) -> Self {
        Seam {
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
            upstream: upstream.to_string(),
            secret: secret.to_string(),
        }
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
}
