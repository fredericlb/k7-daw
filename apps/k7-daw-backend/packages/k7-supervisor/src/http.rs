//! http — the one HTTP client of the supervisor: hyper over rustls with the trust roots, every call bounded,
//! a redirect never followed (the answer must come from the address asked). A registry that connects and never
//! answers is harmless on a heartbeat and fatal on the deregister that holds Quit open, hence the bounds.
//! What a reader gets wrong: `timeout` bounds the connect AND the rest, each, as the C++ client did.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

/// What came back: the status (0 when nothing answered), the body, and why when nothing did.
#[derive(Debug, Clone, Default)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub error: String,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// One call.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: &'static str,
    pub url: String,
    pub body: String,
    pub content_type: String,
    /// The connect and the transfer are each bounded by it.
    pub timeout: Duration,
    /// Sent as given, beside the content type.
    pub headers: BTreeMap<String, String>,
}

impl Request {
    pub fn get(url: String) -> Request {
        Request {
            method: "GET",
            url,
            body: String::new(),
            content_type: String::new(),
            timeout: Duration::from_secs(2),
            headers: BTreeMap::new(),
        }
    }

    pub fn post_json(url: String, body: String) -> Request {
        Request {
            method: "POST",
            body,
            content_type: "application/json".into(),
            ..Request::get(url)
        }
    }
}

fn failed(error: impl Into<String>) -> Response {
    Response {
        error: error.into(),
        ..Response::default()
    }
}

/// Perform `r` over the trust roots; never panics, and answers within twice its timeout.
pub async fn send(r: Request) -> Response {
    send_with(crate::trust::client_config(), r).await
}

/// `send` over a TLS configuration of the caller's.
pub async fn send_with(tls: Arc<rustls::ClientConfig>, r: Request) -> Response {
    let bound = r.timeout;
    match tokio::time::timeout(bound * 2, exchange(tls, r)).await {
        Ok(res) => res,
        Err(_) => failed("timed out"),
    }
}

async fn exchange(tls: Arc<rustls::ClientConfig>, r: Request) -> Response {
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    http.set_connect_timeout(Some(r.timeout));
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config((*tls).clone())
        .https_or_http()
        .enable_http1()
        .wrap_connector(http);
    let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new())
        .pool_max_idle_per_host(0)
        .build(connector);
    let mut req = hyper::Request::builder().method(r.method).uri(&r.url);
    if !r.content_type.is_empty() {
        req = req.header("content-type", &r.content_type);
    }
    for (k, v) in &r.headers {
        req = req.header(k, v);
    }
    let Ok(req) = req.body(Full::new(Bytes::from(r.body))) else {
        return failed("not a request");
    };
    let res = match client.request(req).await {
        Ok(res) => res,
        Err(e) => return failed(describe(&e)),
    };
    let status = res.status().as_u16();
    match res.into_body().collect().await {
        Ok(body) => Response {
            status,
            body: String::from_utf8_lossy(&body.to_bytes()).into_owned(),
            error: String::new(),
        },
        Err(e) => failed(e.to_string()),
    }
}

/// A client error with its causes joined, so a refused certificate and a missing host do not read alike.
fn describe(e: &hyper_util::client::legacy::Error) -> String {
    let mut text = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        text += &format!(": {s}");
        source = s.source();
    }
    text
}
