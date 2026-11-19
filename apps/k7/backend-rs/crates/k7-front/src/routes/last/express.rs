//! express.rs — the answers Express 4 writes itself, to the byte: finalhandler's 404 page, `encodeurl`, `escape-html`, and `fresh`'s conditional GET.
//! What a reader gets wrong: the 404 names the path only, never its query, encoded and then escaped.

use axum::body::Body;
use axum::response::Response;
use hyper::header::{
    CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH,
    VARY,
};
use hyper::{HeaderMap, Method, StatusCode};

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
}
