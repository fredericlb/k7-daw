//! http.rs — the hub's own loopback calls: a service's ready probe and callbacks, and the engine seam.
//! What a reader gets wrong: an answer that is not JSON reads as `{}`, as `r.json().catch(() => ({}))` does.

use std::sync::OnceLock;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use serde_json::Value;

type Http = Client<HttpConnector, Full<Bytes>>;

fn client() -> &'static Http {
    static CLIENT: OnceLock<Http> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let mut c = HttpConnector::new();
        c.set_nodelay(true);
        Client::builder(TokioExecutor::new())
            // Under node's 5 s `keepAliveTimeout`: a socket the service is closing is never reused.
            .pool_idle_timeout(Duration::from_secs(4))
            .build(c)
    })
}

/// One call: its status and its body read as JSON; Err when nothing answered.
pub async fn call(
    method: Method,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&Value>,
    timeout: Duration,
) -> Result<(u16, Value), String> {
    let mut req = Request::builder().method(method).uri(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let bytes = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Bytes::from(b.to_string())
        }
        None => Bytes::new(),
    };
    let req = req.body(Full::new(bytes)).map_err(|e| e.to_string())?;
    let ask = async {
        let res = client().request(req).await.map_err(|e| e.to_string())?;
        let status = res.status().as_u16();
        let body = res
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes();
        let v = serde_json::from_slice(&body).unwrap_or_else(|_| Value::Object(Default::default()));
        Ok((status, v))
    };
    tokio::time::timeout(timeout, ask)
        .await
        .map_err(|_| format!("no answer within {} ms", timeout.as_millis()))?
}
