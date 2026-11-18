//! claude.rs — one streamed `POST /v1/messages` as `@anthropic-ai/sdk` 0.128's `messages.stream` makes it: the
//! same headers that matter, the body as given, its retries (two, on a connection error, 408, 409, 429 and 5xx,
//! `x-should-retry` and `retry-after` obeyed) and its ten-minute timeout, which runs until the headers only.
//! What a reader gets wrong: once the answer has begun nothing is retried — a dropped stream is the caller's error.

use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{HeaderMap, Method, Request, StatusCode};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use serde_json::Value;

use super::sse::{Decoder, Lines, Message, make_message};

/// The SDK's `maxRetries` default.
const MAX_RETRIES: u32 = 2;
/// The SDK's `DEFAULT_TIMEOUT`, armed around the fetch, which resolves with the headers.
const TIMEOUT: Duration = Duration::from_secs(600);
/// The most one streamed answer may bring: max_tokens bounds a real one, a hostile upstream is cut here.
const STREAM_LIMIT: usize = 8 << 20;
/// What the SDK sends as `anthropic-version`.
const API_VERSION: &str = "2023-06-01";

/// Why a turn has no message: an `APIError` (said `Claude API: …`) or any other error, said bare.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Api(String),
    Other(String),
}

impl Failure {
    /// What the route streams as the error event's `error`.
    pub fn message(&self) -> String {
        match self {
            Failure::Api(m) => format!("Claude API: {m}"),
            Failure::Other(m) => m.clone(),
        }
    }
}

type Https = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// The Messages API at one base URL.
pub struct Claude {
    url: String,
    http: Https,
}

impl Claude {
    /// `buildURL`: the base with `/v1/messages`, one slash between them.
    pub fn new(base: &str) -> Self {
        let url = format!("{}/v1/messages", base.strip_suffix('/').unwrap_or(base));
        Claude {
            url,
            http: crate::https::client(),
        }
    }

    /// One streamed turn: `on_text` hears every text delta; the message is the one `message_stop` sealed.
    pub async fn stream(
        &self,
        key: &str,
        body: Bytes,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Value, Failure> {
        let mut attempt = 0;
        let res = loop {
            let req = self.request(key, body.clone(), attempt);
            let left = MAX_RETRIES - attempt;
            let res = match tokio::time::timeout(TIMEOUT, self.http.request(req)).await {
                Ok(Ok(res)) => res,
                failed => {
                    if left > 0 {
                        tokio::time::sleep(backoff(attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    let message = match failed {
                        Err(_) => "Request timed out.",
                        Ok(_) => "Connection error.",
                    };
                    return Err(Failure::Api(message.into()));
                }
            };
            if res.status().is_success() {
                break res;
            }
            let status = res.status();
            let headers = res.headers().clone();
            if left > 0 && should_retry(status, &headers) {
                tokio::time::sleep(backoff(attempt, Some(&headers))).await;
                attempt += 1;
                continue;
            }
            let text = match res.into_body().collect().await {
                Ok(b) => String::from_utf8_lossy(&b.to_bytes()).into_owned(),
                Err(e) => e.to_string(),
            };
            let json = serde_json::from_str::<Value>(&text).ok();
            let message = if json.is_some() {
                None
            } else {
                Some(text.as_str())
            };
            return Err(Failure::Api(make_message(
                Some(status.as_u16()),
                json.as_ref(),
                message,
            )));
        };
        read(res.into_body(), on_text).await
    }

    fn request(&self, key: &str, body: Bytes, attempt: u32) -> Request<Full<Bytes>> {
        let timeout = TIMEOUT.as_secs().to_string();
        let builder = Request::builder()
            .method(Method::POST)
            .uri(&self.url)
            .header("accept", "application/json")
            .header(
                "user-agent",
                concat!("k7-front/", env!("CARGO_PKG_VERSION")),
            )
            .header("x-stainless-retry-count", attempt.to_string())
            .header("x-stainless-timeout", timeout)
            .header("anthropic-version", API_VERSION)
            .header("x-api-key", key)
            .header("content-type", "application/json")
            .header("x-stainless-helper-method", "stream");
        builder
            .body(Full::new(body))
            .unwrap_or_else(|_| Request::new(Full::new(Bytes::new())))
    }
}

/// The SSE body read to its end, folded into one message as `MessageStream` folds it.
async fn read(
    mut body: hyper::body::Incoming,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Value, Failure> {
    let mut lines = Lines::default();
    let mut decoder = Decoder::default();
    let mut message = Message::default();
    let mut feed = |line: String, message: &mut Message| -> Result<(), Failure> {
        if let Some(ev) = decoder.line(&line)
            && let Some(text) = message.event(&ev)?
        {
            on_text(&text);
        }
        Ok(())
    };
    let mut seen = 0usize;
    while let Some(frame) = body.frame().await {
        // Undici's word for a body cut short, which the SDK passes through as the error's message.
        let frame = frame.map_err(|_| Failure::Other("terminated".into()))?;
        if let Some(chunk) = frame.data_ref() {
            seen += chunk.len();
            if seen > STREAM_LIMIT {
                return Err(Failure::Other(
                    "the answer ran past the size accepted".into(),
                ));
            }
            for line in lines.push(chunk) {
                feed(line, &mut message)?;
            }
        }
    }
    for line in lines.flush() {
        feed(line, &mut message)?;
    }
    message.finish()
}

/// `shouldRetry` for an answer that is not a success.
fn should_retry(status: StatusCode, headers: &HeaderMap) -> bool {
    match headers.get("x-should-retry").and_then(|v| v.to_str().ok()) {
        Some("true") => return true,
        Some("false") => return false,
        _ => {}
    }
    matches!(status.as_u16(), 408 | 409 | 429) || status.as_u16() >= 500
}

/// `retryRequest`'s wait: `retry-after-ms`, `retry-after` (seconds or a date), or 0.5 s doubled up to 8 s, jittered.
fn backoff(attempt: u32, headers: Option<&HeaderMap>) -> Duration {
    let header = |name: &str| {
        headers
            .and_then(|h| h.get(name))
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let mut ms = header("retry-after-ms").and_then(|v| parse_float(&v));
    if ms.is_none_or(|m| m == 0.0)
        && let Some(after) = header("retry-after")
    {
        ms = parse_float(&after).map(|s| s * 1000.0).or_else(|| {
            httpdate::parse_http_date(&after).ok().map(|at| {
                let now = std::time::SystemTime::now();
                match at.duration_since(now) {
                    Ok(d) => d.as_secs_f64() * 1000.0,
                    Err(e) => -e.duration().as_secs_f64() * 1000.0,
                }
            })
        });
    }
    let ms = match ms {
        Some(m) if m > 0.0 && m <= f64::from(i32::MAX) => m,
        _ => {
            let base = (0.5 * 2f64.powi(attempt as i32)).min(8.0);
            base * (1.0 - random_unit() * 0.25) * 1000.0
        }
    };
    Duration::from_secs_f64(ms / 1000.0)
}

/// `parseFloat`: the longest decimal prefix after leading spaces, or None for NaN.
fn parse_float(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let mut end = 0;
    let mut seen_dot = false;
    let mut seen_exp = false;
    let b = s.as_bytes();
    let mut best = None;
    while end < b.len() {
        let c = b[end];
        let ok = c.is_ascii_digit()
            || (c == b'.' && !seen_dot && !seen_exp)
            || ((c == b'e' || c == b'E') && !seen_exp && end > 0)
            || ((c == b'+' || c == b'-') && (end == 0 || matches!(b[end - 1], b'e' | b'E')));
        if !ok {
            break;
        }
        seen_dot |= c == b'.';
        seen_exp |= c == b'e' || c == b'E';
        end += 1;
        if let Ok(v) = s[..end].parse::<f64>() {
            best = Some(v);
        }
    }
    best
}

/// A uniform number in [0, 1) for the jitter.
fn random_unit() -> f64 {
    let mut b = [0u8; 8];
    if getrandom::fill(&mut b).is_err() {
        return 0.5;
    }
    (u64::from_le_bytes(b) >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_float_reads_a_prefix() {
        assert_eq!(parse_float("1.5abc"), Some(1.5));
        assert_eq!(parse_float(" 2"), Some(2.0));
        assert_eq!(parse_float("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(parse_float("3e2"), Some(300.0));
    }

    #[test]
    fn backoff_obeys_the_server_then_doubles() {
        let mut h = HeaderMap::new();
        h.insert("retry-after-ms", "250".parse().unwrap());
        assert_eq!(backoff(0, Some(&h)), Duration::from_millis(250));
        let first = backoff(0, None).as_secs_f64();
        assert!((0.375..=0.5).contains(&first));
        let late = backoff(6, None).as_secs_f64();
        assert!((6.0..=8.0).contains(&late));
    }

    #[test]
    fn retries_follow_the_sdk() {
        let none = HeaderMap::new();
        assert!(should_retry(StatusCode::TOO_MANY_REQUESTS, &none));
        assert!(should_retry(StatusCode::from_u16(529).unwrap(), &none));
        assert!(!should_retry(StatusCode::BAD_REQUEST, &none));
        let mut h = HeaderMap::new();
        h.insert("x-should-retry", "true".parse().unwrap());
        assert!(should_retry(StatusCode::BAD_REQUEST, &h));
    }
}
