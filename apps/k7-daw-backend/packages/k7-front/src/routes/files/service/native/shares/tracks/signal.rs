//! signal.rs — the "download started" cookie of `/export/tracks`: set on the answer's first byte, named by a token the page chose.
//! What a reader gets wrong: a refusal sets it too, carrying its sentence, so the page can say why no download came.

use axum::response::Response;
use hyper::header::{HeaderValue, SET_COOKIE};

/// The cookie's name starts with this, then the token.
pub const PREFIX: &str = "k7dl-";
/// The longest token and the longest sentence the cookie carries.
const TOKEN_MAX: usize = 64;
const SENTENCE_MAX: usize = 160;

/// The token the page named in the query's `dl`, when it is a plain run of letters and digits.
pub fn token_of(query: &str) -> Option<String> {
    let t = query.split('&').find_map(|p| p.strip_prefix("dl="))?;
    (!t.is_empty() && t.len() <= TOKEN_MAX && t.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then(|| t.to_owned())
}

fn escaped(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().take(SENTENCE_MAX) {
        if c.is_ascii_alphanumeric() || " .,-_".contains(c) {
            out.push(c);
        } else {
            let mut b = [0u8; 4];
            for byte in c.encode_utf8(&mut b).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

/// `r` with the cookie of `token`: `ok` on an answer, `err.<sentence>` on a refusal.
pub fn mark(mut r: Response, token: &str, refusal: Option<&str>) -> Response {
    let value = refusal.map_or_else(|| "ok".to_owned(), |s| format!("err.{}", escaped(s)));
    let cookie = format!("{PREFIX}{token}={value}; Path=/; Max-Age=120; SameSite=Lax");
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        r.headers_mut().append(SET_COOKIE, v);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(r: &Response) -> String {
        r.headers()[SET_COOKIE].to_str().unwrap().to_owned()
    }

    #[test]
    fn a_token_is_plain_letters_and_digits_or_nothing() {
        assert_eq!(token_of("a=1&dl=Ab12").as_deref(), Some("Ab12"));
        for bad in [
            "",
            "dl=",
            "dl=a;b",
            "dl=a%20b",
            "dl=a/b",
            &format!("dl={}", "a".repeat(65)),
        ] {
            assert_eq!(token_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_cookie_says_ok_or_carries_the_refusal_escaped_and_cut() {
        let ok = mark(Response::new(axum::body::Body::empty()), "t1", None);
        assert!(cookie(&ok).starts_with("k7dl-t1=ok;"), "{}", cookie(&ok));
        let no = mark(
            Response::new(axum::body::Body::empty()),
            "t1",
            Some("no engine; é"),
        );
        let c = cookie(&no);
        assert!(c.starts_with("k7dl-t1=err.no engine%3B %C3%A9;"), "{c}");
        let long = mark(
            Response::new(axum::body::Body::empty()),
            "t1",
            Some(&"x".repeat(500)),
        );
        assert!(cookie(&long).len() < 260);
    }
}
