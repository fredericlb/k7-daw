//! express.rs — the answers Express 4 writes itself, to the byte: finalhandler's 404 page, `res.redirect`'s
//! negotiated body, `encodeurl`, `escape-html`, the `etag` of a body sent whole, and `fresh`'s conditional GET.
//! What a reader gets wrong: the 404 names the path only, never its query, encoded and then escaped.

use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hyper::header::{
    CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH,
    LOCATION, VARY,
};
use hyper::{HeaderMap, Method, StatusCode};
use sha1::{Digest, Sha1};

/// `escape-html`: the five characters HTML gives a meaning.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// `encodeurl` 2: every character outside the URL's own set percent-encoded, a `%` kept only before two hex digits.
pub fn encode_url(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut out = String::with_capacity(url.len());
    let mut i = 0;
    for c in url.chars() {
        let kept = match c {
            '%' => {
                bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                    && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
            }
            '!' | '#'..=';' | '=' | '?'..='_' | 'a'..='z' | '|' | '~' => true,
            _ => false,
        };
        if kept {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        i += c.len_utf8();
    }
    out
}

/// The page Express's final handler sends when no route answered.
fn not_found_page(method: &Method, path: &str) -> String {
    let message = escape_html(&format!("Cannot {method} {}", encode_url(path)))
        .replace('\n', "<br>")
        .replace("  ", " &nbsp;");
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Error</title>\n</head>\n<body>\n<pre>{message}</pre>\n</body>\n</html>\n"
    )
}

/// finalhandler's 404 for `method` on `path`; `vary` when the static step had already named `Accept-Encoding`.
pub fn not_found(method: &Method, path: &str, vary: bool) -> Response {
    let page = not_found_page(method, path);
    let mut builder = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-security-policy", "default-src 'none'")
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header(CONTENT_LENGTH, page.len());
    if vary {
        builder = builder.header(VARY, "Accept-Encoding");
    }
    let body = if *method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(page)
    };
    builder.body(body).unwrap_or_default()
}

/// One `Accept` entry: its type, subtype and weight, in the order the client wrote it.
struct Accepted {
    kind: String,
    sub: String,
    q: f64,
}

fn accepted(header: &str) -> Vec<Accepted> {
    header
        .split(',')
        .filter_map(|part| {
            let mut params = part.split(';');
            let (kind, sub) = params.next()?.trim().split_once('/')?;
            let q = params
                .filter_map(|p| p.trim().strip_prefix("q="))
                .next()
                .map_or(Some(1.0), |q| q.trim().parse::<f64>().ok())?;
            Some(Accepted {
                kind: kind.trim().to_ascii_lowercase(),
                sub: sub.trim().to_ascii_lowercase(),
                q,
            })
        })
        .collect()
}

/// `req.accepts(["text", "html"])` as negotiator ranks them: weight, then how exactly the entry matched, then order.
fn negotiate(headers: &HeaderMap) -> Option<&'static str> {
    let Some(header) = headers
        .get(hyper::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
    else {
        return Some("text/plain");
    };
    let entries = accepted(header);
    let rank = |kind: &str, sub: &str| {
        entries
            .iter()
            .enumerate()
            .filter_map(|(o, e)| {
                let s = match (e.kind == kind, e.sub == sub) {
                    (true, true) => 6,
                    (true, false) if e.sub == "*" => 4,
                    (false, _) if e.kind == "*" && e.sub == "*" => 0,
                    _ => return None,
                };
                Some((s, e.q, o))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)))
    };
    let mut best: Option<(&'static str, f64, i32, usize, usize)> = None;
    for (i, (t, kind, sub)) in [
        ("text/plain", "text", "plain"),
        ("text/html", "text", "html"),
    ]
    .into_iter()
    .enumerate()
    {
        let Some((s, q, o)) = rank(kind, sub).filter(|r| r.1 > 0.0) else {
            continue;
        };
        let better = best
            .is_none_or(|(_, bq, bs, bo, _)| q > bq || q == bq && (s > bs || s == bs && o < bo));
        if better {
            best = Some((t, q, s, o, i));
        }
    }
    best.map(|b| b.0)
}

/// `res.redirect(status, url)`: the address encoded, and a body in the type the client takes.
pub fn redirect(req: &Request, status: StatusCode, url: &str) -> Response {
    let address = encode_url(url);
    let message = status.canonical_reason().unwrap_or("");
    let mut builder = Response::builder()
        .status(status)
        .header(
            LOCATION,
            HeaderValue::from_str(&address).unwrap_or(HeaderValue::from_static("/")),
        )
        .header(VARY, "Accept");
    let body = match negotiate(req.headers()) {
        Some("text/html") => {
            builder = builder.header(CONTENT_TYPE, "text/html; charset=utf-8");
            format!("<p>{message}. Redirecting to {}</p>", escape_html(&address))
        }
        Some(_) => {
            builder = builder.header(CONTENT_TYPE, "text/plain; charset=utf-8");
            format!("{message}. Redirecting to {address}")
        }
        None => String::new(),
    };
    builder = builder.header(CONTENT_LENGTH, body.len());
    let body = if req.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from(body)
    };
    builder.body(body).unwrap_or_default()
}

/// The `etag` package's weak tag of a body sent whole: its length and a SHA-1, in base64 cut to 27 characters.
pub fn body_etag(body: &[u8]) -> String {
    let hash = STANDARD.encode(Sha1::digest(body));
    format!("W/\"{:x}-{}\"", body.len(), &hash[..27])
}

/// `If-None-Match` and `If-Modified-Since` read as the `fresh` package reads them; a `no-cache` request is never fresh.
pub fn fresh(headers: &HeaderMap, etag: Option<&str>, last_modified: Option<&str>) -> bool {
    let get = |n| {
        headers
            .get(n)
            .and_then(|v: &HeaderValue| v.to_str().ok())
            .filter(|v| !v.is_empty())
    };
    let (since, none_match) = (get(IF_MODIFIED_SINCE), get(IF_NONE_MATCH));
    if since.is_none() && none_match.is_none() {
        return false;
    }
    if get(CACHE_CONTROL).is_some_and(no_cache) {
        return false;
    }
    if let Some(list) = none_match.filter(|m| *m != "*") {
        let Some(etag) = etag else {
            return false;
        };
        let listed =
            tokens(list).any(|m| m == etag || format!("W/{etag}") == m || format!("W/{m}") == etag);
        if !listed {
            return false;
        }
    }
    if let Some(since) = since {
        let at = |v: &str| httpdate::parse_http_date(v.trim()).ok();
        let fresh = last_modified
            .and_then(at)
            .zip(at(since))
            .is_some_and(|(m, s)| m <= s);
        if !fresh {
            return false;
        }
    }
    true
}

/// `/(?:^|,)\s*?no-cache\s*?(?:,|$)/`.
fn no_cache(v: &str) -> bool {
    v.split(',').any(|d| d.trim() == "no-cache")
}

/// `parseTokenList`: the entries between spaces and commas.
pub fn tokens(list: &str) -> impl Iterator<Item = &str> {
    list.split([' ', ',']).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(accept: Option<&str>) -> Request {
        let mut b = Request::builder().uri("/x");
        if let Some(a) = accept {
            b = b.header(hyper::header::ACCEPT, a);
        }
        b.body(Body::empty()).unwrap()
    }

    #[test]
    fn urls_are_encoded_as_encodeurl_encodes_them() {
        assert_eq!(encode_url("/a b"), "/a%20b");
        assert_eq!(encode_url("/a%20b%zz%4"), "/a%20b%25zz%254");
        assert_eq!(encode_url("/é?x=[1]|~"), "/%C3%A9?x=[1]|~");
        assert_eq!(encode_url("/`{}\"<>"), "/%60%7B%7D%22%3C%3E");
    }

    #[test]
    fn the_404_page_is_finalhandlers() {
        assert_eq!(
            not_found_page(&Method::GET, "/a<b"),
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Error</title>\n</head>\n<body>\n<pre>Cannot GET /a%3Cb</pre>\n</body>\n</html>\n"
        );
    }

    #[test]
    fn a_redirect_speaks_the_type_asked() {
        assert_eq!(negotiate(req(None).headers()), Some("text/plain"));
        assert_eq!(negotiate(req(Some("*/*")).headers()), Some("text/plain"));
        assert_eq!(
            negotiate(req(Some("text/html,application/xhtml+xml,*/*;q=0.8")).headers()),
            Some("text/html")
        );
        assert_eq!(negotiate(req(Some("application/json")).headers()), None);
        assert_eq!(
            negotiate(req(Some("text/*;q=0.5, text/html")).headers()),
            Some("text/html")
        );
    }

    #[test]
    fn a_body_tag_is_the_etag_packages() {
        assert_eq!(
            body_etag(b"{\"error\":\"unauthenticated\"}"),
            "W/\"1b-RCkDNv1wa3UitMAdSM+QgID7d6A\""
        );
    }
}
