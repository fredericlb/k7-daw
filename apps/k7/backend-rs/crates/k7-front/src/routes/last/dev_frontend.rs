//! dev_frontend.rs — in the dev tray, the page comes from a running Vite instead of `frontend/dist`: each request the
//! built page would answer is streamed there and back, so 127.0.0.1:18789 gets HMR on its own origin
//! (`docs/dev.md`). What a reader gets wrong: only `K7_DEV_FRONTEND` turns it on.

use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use hyper::header::{CONTENT_TYPE, HOST, HeaderValue};
use hyper::{StatusCode, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

/// A Vite that never answers must not hold the page for ever.
const ANSWER_WITHIN: Duration = Duration::from_secs(15);

pub struct DevFrontend {
    base: String,
    host: HeaderValue,
    client: Client<HttpConnector, Body>,
}

impl DevFrontend {
    /// The Vite at `base`, an `http://host:port` address; None for one that does not parse.
    pub fn new(base: &str) -> Option<DevFrontend> {
        let uri: Uri = base.parse().ok()?;
        let host = HeaderValue::from_str(uri.authority()?.as_str()).ok()?;
        eprintln!("k7-front: the page comes from the dev frontend at {base}");
        Some(DevFrontend {
            base: base.to_string(),
            host,
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
        })
    }

    /// The request as it came, the host Vite answers named; its absence is a 502 that names the resource.
    pub async fn forward(&self, req: Request) -> Response {
        let (mut parts, body) = req.into_parts();
        let path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
        let Ok(uri) = format!("{}{path}", self.base.trim_end_matches('/')).parse::<Uri>() else {
            return (StatusCode::BAD_REQUEST, "bad request target").into_response();
        };
        crate::hop::strip_hop_by_hop(&mut parts.headers, false);
        parts.headers.insert(HOST, self.host.clone());
        parts.uri = uri;
        parts.version = hyper::Version::HTTP_11;
        let asked = self.client.request(Request::from_parts(parts, body));
        match tokio::time::timeout(ANSWER_WITHIN, asked).await {
            Ok(Ok(mut r)) => {
                crate::hop::strip_hop_by_hop(r.headers_mut(), false);
                r.map(Body::new)
            }
            Ok(Err(e)) => self.missing(&e.to_string()),
            Err(_) => self.missing("no answer in 15 s"),
        }
    }

    fn missing(&self, why: &str) -> Response {
        let text = format!(
            "no dev frontend at {} ({why}) — start the k7-frontend resource",
            self.base
        );
        let mut r = (StatusCode::BAD_GATEWAY, text).into_response();
        r.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        r
    }
}
