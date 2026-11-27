//! web.rs — what every route the front OWNS goes through, in order: the security headers, a device's loopback
//! lock, then CORS and the cross-site write refusal. Declined requests meet it once, ahead of the last routes.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hyper::header::{CONTENT_TYPE, HOST, HeaderValue, ORIGIN, VARY};
use hyper::{HeaderMap, Method, StatusCode};
use serde_json::{Map, Value};

use crate::AppState;
use crate::settings::Settings;

/// What a page may send: the studio headers, never the hub secret (`media/cors.ts`).
const ALLOWED_HEADERS: &str = "Content-Type, Authorization, x-studio-user, x-studio-workspace, x-studio-engine-token, x-studio-engine-id, x-k7-file, x-k7-as";

/// The default JSON body limit.
const JSON_LIMIT: usize = 256 * 1024;

/// The body as the shared parser reads it, ahead of every guard: JSON when typed so, `{}` otherwise.
#[allow(clippy::result_large_err, reason = "the refusal is the answer itself")]
pub async fn read_json(req: Request) -> Result<(HeaderMap, Value), Response> {
    read_json_within(req, JSON_LIMIT).await
}

/// A peer's answer read whole, refused once it passes `limit`: a server or a provider never chooses the front's memory.
pub async fn collect_within<B>(body: B, limit: usize) -> Result<axum::body::Bytes, String>
where
    B: axum::body::HttpBody<Data = axum::body::Bytes>,
    B::Error: Into<axum::BoxError>,
{
    use http_body_util::BodyExt;
    match http_body_util::Limited::new(body, limit).collect().await {
        Ok(c) => Ok(c.to_bytes()),
        Err(e) => Err(e.to_string()),
    }
}

/// `read_json` under a route's own limit.
#[allow(clippy::result_large_err, reason = "the refusal is the answer itself")]
pub async fn read_json_within(req: Request, limit: usize) -> Result<(HeaderMap, Value), Response> {
    let (parts, body) = req.into_parts();
    let is_json = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(is_json_type);
    // body-parser leaves a body of another type unread, whatever its size.
    if !is_json {
        return Ok((parts.headers, Value::Object(Map::new())));
    }
    let Ok(body) = axum::body::to_bytes(body, limit).await else {
        return Err(error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request entity too large",
        ));
    };
    if body.is_empty() {
        return Ok((parts.headers, Value::Object(Map::new())));
    }
    // Strict, as body-parser's default: only an object or an array is a body.
    let first = body
        .iter()
        .find(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'));
    if !matches!(first, Some(b'{' | b'[')) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid JSON: not an object or an array",
        ));
    }
    match serde_json::from_slice(&body) {
        Ok(v) => Ok((parts.headers, v)),
        Err(e) => Err(error(
            StatusCode::BAD_REQUEST,
            &format!("invalid JSON: {e}"),
        )),
    }
}

/// body-parser's `application/json`: that type, or any `+json` suffix.
pub fn is_json_type(ct: &str) -> bool {
    let essence = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    essence == "application/json"
        || (essence.starts_with("application/") && essence.ends_with("+json"))
}

/// A JSON `{ error }` answer, the backend's shape for every refusal.
pub fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// `localhost`, `127.x.x.x` or `[::1]`.
pub fn is_loopback_hostname(host: &str) -> bool {
    if host == "localhost" || host == "[::1]" {
        return true;
    }
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts[0] == "127"
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// A `Host` header naming this machine, port and all; absent or unparsable is not.
fn is_loopback_host_header(host: Option<&str>) -> bool {
    host.and_then(|h| url::Url::parse(&format!("http://{h}")).ok())
        .and_then(|u| u.host_str().map(str::to_string))
        .is_some_and(|h| is_loopback_hostname(&h))
}

fn is_loopback_peer(peer: IpAddr) -> bool {
    match peer {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// `isLoopbackRequest`: the peer is this machine, and the page asked for this machine by name.
pub fn is_loopback_request(peer: SocketAddr, headers: &HeaderMap) -> bool {
    is_loopback_peer(peer.ip())
        && is_loopback_host_header(headers.get(HOST).and_then(|v| v.to_str().ok()))
}

/// The host the browser addressed: a trusted edge's `X-Forwarded-Host` first, else `Host`.
pub fn request_host(headers: &HeaderMap, settings: &Settings) -> String {
    let forwarded = (settings.trust_hops > 0)
        .then(|| {
            headers
                .get("x-forwarded-host")
                .and_then(|v| v.to_str().ok())
        })
        .flatten()
        .and_then(|v| v.split(',').next());
    forwarded
        .or_else(|| headers.get(HOST).and_then(|v| v.to_str().ok()))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Whether `origin` may call with credentials, as `isAllowedOrigin` decides it.
pub fn is_allowed_origin(origin: &str, request_host: &str) -> bool {
    let Ok(u) = url::Url::parse(origin) else {
        return false;
    };
    if u.scheme() != "https" && u.scheme() != "http" {
        return false;
    }
    let Some(hostname) = u.host_str() else {
        return false;
    };
    let host = match u.port() {
        Some(p) => format!("{hostname}:{p}"),
        None => hostname.to_string(),
    };
    if !request_host.is_empty() && host == request_host.to_ascii_lowercase() {
        return true;
    }
    is_loopback_hostname(hostname)
}

/// Set on a file's answer, whose content type is the file's own and not a JSON answer's.
#[derive(Clone, Copy)]
pub struct FileType;

/// The middleware every owned route sits behind; see the header for the order and why.
pub async fn guard(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let settings = &state.settings;
    let mut response = guarded(settings, peer, req, next).await;
    let h = response.headers_mut();
    h.entry("x-content-type-options")
        .or_insert(HeaderValue::from_static("nosniff"));
    h.entry("referrer-policy")
        .or_insert(HeaderValue::from_static("strict-origin-when-cross-origin"));
    // Express names the charset of every JSON answer, never of a file it streams with its own type.
    let file = response.extensions().get::<FileType>().is_some();
    let h = response.headers_mut();
    if !file && h.get(CONTENT_TYPE).is_some_and(|v| v == "application/json") {
        h.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
    }
    response
}

async fn guarded(settings: &Settings, peer: SocketAddr, req: Request, next: Next) -> Response {
    if settings.local_user && !is_loopback_request(peer, req.headers()) {
        let mut r = (
            StatusCode::FORBIDDEN,
            "This server answers its own machine only.",
        )
            .into_response();
        r.headers_mut().insert(
            "content-type",
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        return r;
    }
    let origin = req
        .headers()
        .get(ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let allowed = origin
        .as_deref()
        .is_some_and(|o| is_allowed_origin(o, &request_host(req.headers(), settings)));
    let fetch_site = req
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let method = req.method().clone();
    let mut response = if method == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else if is_cross_site_write(&method, origin.as_deref(), fetch_site.as_deref(), allowed) {
        error(StatusCode::FORBIDDEN, "cross-site request refused")
    } else {
        next.run(req).await
    };
    let h = response.headers_mut();
    // The backend's CORS writes `Vary: Origin` first; what a route adds after (Express's `Accept`) follows it.
    if origin.is_some() {
        let later = h
            .get(VARY)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
            .map(|v| format!("Origin, {v}"));
        let vary = later.and_then(|v| HeaderValue::from_str(&v).ok());
        h.insert(VARY, vary.unwrap_or(HeaderValue::from_static("Origin")));
    }
    if allowed && let Some(v) = origin.and_then(|o| HeaderValue::from_str(&o).ok()) {
        h.insert("access-control-allow-origin", v);
        h.insert(
            "access-control-allow-credentials",
            HeaderValue::from_static("true"),
        );
        // Under `/auth`, `setupAuth` narrows the methods for a trusted origin after this was set.
        h.entry("access-control-allow-methods")
            .or_insert(HeaderValue::from_static(
                "GET, POST, PUT, PATCH, DELETE, OPTIONS",
            ));
        h.insert(
            "access-control-allow-headers",
            HeaderValue::from_static(ALLOWED_HEADERS),
        );
    }
    response
}

/// A write a page on another site sent: refused unless its origin is allowed; with no Origin, `Sec-Fetch-Site` decides.
pub fn is_cross_site_write(
    method: &Method,
    origin: Option<&str>,
    fetch_site: Option<&str>,
    allowed: bool,
) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    match origin {
        Some(_) => !allowed,
        None => fetch_site == Some("cross-site"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_follow_media_cors() {
        assert!(
            is_allowed_origin("https://app.k7.rocks", "app.k7.rocks"),
            "same host"
        );
        assert!(
            !is_allowed_origin("https://docs.k7.rocks", "app.k7.rocks"),
            "another subdomain"
        );
        assert!(!is_allowed_origin("https://evil.example", "app.k7.rocks"));
        assert!(!is_allowed_origin("null", "app.k7.rocks"));
        assert!(is_allowed_origin(
            "http://127.0.0.1:5173",
            "127.0.0.1:18789"
        ));
        assert!(is_allowed_origin("http://[::1]:5173", "127.0.0.1:18789"));
    }

    #[test]
    fn writes_from_elsewhere_are_refused() {
        assert!(!is_cross_site_write(
            &Method::GET,
            Some("https://evil"),
            None,
            false
        ));
        assert!(is_cross_site_write(
            &Method::PUT,
            Some("https://evil"),
            None,
            false
        ));
        assert!(!is_cross_site_write(
            &Method::PUT,
            Some("https://ok"),
            None,
            true
        ));
        assert!(is_cross_site_write(
            &Method::POST,
            None,
            Some("cross-site"),
            false
        ));
        assert!(!is_cross_site_write(
            &Method::POST,
            None,
            Some("same-origin"),
            false
        ));
    }

    #[test]
    fn loopback_names() {
        assert!(
            is_loopback_hostname("localhost")
                && is_loopback_hostname("127.0.0.1")
                && is_loopback_hostname("[::1]")
        );
        assert!(
            !is_loopback_hostname("127.0.0.1.evil")
                && !is_loopback_hostname("localhost.evil")
                && !is_loopback_hostname("::1")
        );
        assert!(
            is_loopback_host_header(Some("127.0.0.1:18789"))
                && !is_loopback_host_header(Some("evil:80"))
                && !is_loopback_host_header(None)
        );
    }
}
