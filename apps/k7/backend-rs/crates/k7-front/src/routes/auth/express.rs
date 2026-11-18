//! express.rs — the few Express behaviours the `/auth` answers show a client: `req.query` as qs reads it,
//! the HTML escaping, and `res.redirect` with its negotiated body and `Vary: Accept`.

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use hyper::header::{CONTENT_TYPE, HeaderValue, LOCATION, VARY};
use hyper::{HeaderMap, StatusCode};

/// A query string as Express's qs parser exposes it, for the plain string parameters `/auth/local` reads.
pub struct Query {
    pairs: Vec<(String, String)>,
}

/// qs's decoder: `+` is a space, and a malformed escape leaves the text as it was.
fn qs_decode(s: &str) -> String {
    let spaced = s.replace('+', " ");
    percent_decode(&spaced).unwrap_or(spaced)
}

/// `decodeURIComponent`: None where it would throw.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(s.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

impl Query {
    pub fn parse(raw: Option<&str>) -> Self {
        let pairs = raw
            .unwrap_or("")
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|p| match p.split_once('=') {
                Some((k, v)) => (qs_decode(k), qs_decode(v)),
                None => (qs_decode(p), String::new()),
            })
            .collect();
        Query { pairs }
    }

    fn values(&self, name: &str) -> (Vec<&str>, bool) {
        let bracketed = self
            .pairs
            .iter()
            .any(|(k, _)| k.strip_prefix(name).is_some_and(|r| r.starts_with('[')));
        let values = self
            .pairs
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .collect();
        (values, bracketed)
    }

    /// `req.query[name]` when `typeof` it is "string": named once, never as an array or an object.
    pub fn string(&self, name: &str) -> Option<String> {
        match self.values(name) {
            (v, false) if v.len() == 1 => Some(v[0].to_string()),
            _ => None,
        }
    }
}

/// escape-html, as Express's redirect body uses it.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `res.send(text)` of a string: an HTML answer.
pub fn html(status: StatusCode, body: impl Into<String>) -> Response {
    let mut r = (status, body.into()).into_response();
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    r
}

/// One media range of an `Accept` header: type, subtype, whether it carries parameters, its q and position.
struct Range {
    kind: String,
    sub: String,
    params: bool,
    q: f64,
    index: usize,
}

fn ranges(accept: &str) -> Vec<Range> {
    accept
        .split(',')
        .enumerate()
        .filter_map(|(index, part)| {
            let mut pieces = part.split(';');
            let (kind, sub) = pieces.next()?.trim().split_once('/')?;
            let (mut q, mut params) = (1.0, false);
            for p in pieces {
                match p.trim().split_once('=') {
                    Some((k, v)) if k.trim() == "q" => q = v.trim().parse().unwrap_or(0.0),
                    _ => params = true,
                }
            }
            (!kind.is_empty() && !sub.is_empty()).then(|| Range {
                kind: kind.to_ascii_lowercase(),
                sub: sub.trim().to_ascii_lowercase(),
                params,
                q,
                index,
            })
        })
        .collect()
}

/// What `req.accepts(["text", "html"])` picks: negotiator's specificity, q and order, or None.
fn negotiate(accept: Option<&str>) -> Option<&'static str> {
    let Some(accept) = accept else {
        return Some("text/plain");
    };
    let ranges = ranges(accept);
    let mut best: Vec<(usize, f64, i32, usize)> = Vec::new();
    for (i, (kind, sub)) in [("text", "plain"), ("text", "html")].iter().enumerate() {
        let mut pick: Option<(f64, i32, usize)> = None;
        for r in &ranges {
            let mut s = 0;
            if r.kind == *kind {
                s |= 4;
            } else if r.kind != "*" {
                continue;
            }
            if r.sub == *sub {
                s |= 2;
            } else if r.sub != "*" {
                continue;
            }
            if r.params {
                continue;
            }
            let better = match pick {
                None => true,
                Some((q, ps, o)) => {
                    (s - ps).signum() * 4
                        + ((r.q - q).signum() as i32) * 2
                        + (r.index as i32 - o as i32).signum()
                        > 0
                }
            };
            if better {
                pick = Some((r.q, s, r.index));
            }
        }
        if let Some((q, s, o)) = pick.filter(|p| p.0 > 0.0) {
            best.push((i, q, s, o));
        }
    }
    best.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.2.cmp(&a.2))
            .then(a.3.cmp(&b.3))
            .then(a.0.cmp(&b.0))
    });
    best.first()
        .map(|b| if b.0 == 0 { "text/plain" } else { "text/html" })
}

/// `res.redirect(status, location)`: the body negotiated as Express does, and `Vary: Accept`.
pub fn redirect(status: StatusCode, location: &str, headers: &HeaderMap) -> Response {
    let reason = status.canonical_reason().unwrap_or("");
    let accept = headers
        .get_all(hyper::header::ACCEPT)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>();
    let accept = Some(accept.join(", ")).filter(|a| !a.is_empty());
    let (kind, body) = match negotiate(accept.as_deref()) {
        Some("text/plain") => (
            Some("text/plain; charset=utf-8"),
            format!("{reason}. Redirecting to {location}"),
        ),
        Some(_) => (
            Some("text/html; charset=utf-8"),
            format!("<p>{reason}. Redirecting to {}</p>", escape_html(location)),
        ),
        None => (None, String::new()),
    };
    let mut r = Response::new(Body::from(body));
    *r.status_mut() = status;
    let h = r.headers_mut();
    if let Ok(v) = HeaderValue::from_str(location) {
        h.insert(LOCATION, v);
    }
    h.insert(VARY, HeaderValue::from_static("Accept"));
    if let Some(kind) = kind {
        h.insert(CONTENT_TYPE, HeaderValue::from_static(kind));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_query_as_qs_does() {
        let q = Query::parse(Some("a=1&b=x+y%20z&c=1&c=2&d[x]=1&e=%ZZ&f"));
        assert_eq!(q.string("a").as_deref(), Some("1"));
        assert_eq!(q.string("b").as_deref(), Some("x y z"));
        assert_eq!(q.string("c"), None);
        assert_eq!(q.string("d"), None);
        assert_eq!(q.string("e").as_deref(), Some("%ZZ"));
        assert_eq!(q.string("f").as_deref(), Some(""));
    }

    #[test]
    fn negotiates_as_express() {
        assert_eq!(negotiate(None), Some("text/plain"));
        assert_eq!(negotiate(Some("*/*")), Some("text/plain"));
        assert_eq!(
            negotiate(Some(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            )),
            Some("text/html")
        );
        assert_eq!(negotiate(Some("application/json")), None);
        assert_eq!(
            negotiate(Some("text/*;q=0.5, text/html;q=0.4")),
            Some("text/plain")
        );
    }
}
