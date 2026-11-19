//! target.rs — where `/auth/local` may send the browser and who may ask: the request's public origin, a return
//! target resolved as the browser resolves it, and the same-origin check.

use hyper::HeaderMap;

use crate::settings::Settings;
use crate::web::request_host;

/// The app's own address after a refused return target.
pub const APP_ROOT: &str = "/";

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// `req.protocol` as the backend reads it behind the front: the edge's scheme when a hop is trusted, else http.
pub fn protocol(headers: &HeaderMap, settings: &Settings) -> String {
    let forwarded = (settings.trust_hops > 0)
        .then(|| header(headers, "x-forwarded-proto"))
        .flatten()
        .unwrap_or("");
    let first = forwarded.split(',').next().unwrap_or("").trim();
    if first.is_empty() {
        "http".into()
    } else {
        first.to_string()
    }
}

/// `publicOrigin(req)`: the scheme and the host the browser addressed, as text, unnormalized.
pub fn public_origin(headers: &HeaderMap, settings: &Settings) -> String {
    format!(
        "{}://{}",
        protocol(headers, settings),
        request_host(headers, settings)
    )
}

/// `safeReturnTarget`: a path on this origin; anything else is `/`.
pub fn safe_return_target(target: &str, own: &str) -> String {
    if target.is_empty() || target.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}') {
        return APP_ROOT.into();
    }
    let b = target.as_bytes();
    if b.len() >= 2 && matches!(b[0], b'/' | b'\\') && matches!(b[1], b'/' | b'\\') {
        return APP_ROOT.into();
    }
    let Ok(base) = url::Url::parse(own) else {
        return APP_ROOT.into();
    };
    let Ok(url) = url::Url::options().base_url(Some(&base)).parse(target) else {
        return APP_ROOT.into();
    };
    if url.scheme() != "https" && url.scheme() != "http" {
        return APP_ROOT.into();
    }
    let origin = url.origin().ascii_serialization();
    if origin == own {
        if url.path().starts_with("//") {
            return APP_ROOT.into();
        }
        let search = url
            .query()
            .filter(|q| !q.is_empty())
            .map(|q| format!("?{q}"));
        let hash = url
            .fragment()
            .filter(|f| !f.is_empty())
            .map(|f| format!("#{f}"));
        return format!(
            "{}{}{}",
            url.path(),
            search.unwrap_or_default(),
            hash.unwrap_or_default()
        );
    }
    APP_ROOT.into()
}

/// `isSameOriginRequest`: what the browser says of a state-changing request's origin.
pub fn is_same_origin_request(headers: &HeaderMap, settings: &Settings) -> bool {
    if let Some(origin) = headers.get(hyper::header::ORIGIN) {
        let origin = origin.to_str().unwrap_or("\u{0}");
        return origin == public_origin(headers, settings);
    }
    matches!(
        header(headers, "sec-fetch-site"),
        None | Some("same-origin") | Some("none")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "https://app.k7.rocks";

    #[test]
    fn return_targets_resolve_as_the_browser_does() {
        let t = |s: &str| safe_return_target(s, OWN);
        assert_eq!(t("/x?y=1#z"), "/x?y=1#z");
        assert_eq!(t("/a?"), "/a");
        assert_eq!(t("//evil.com"), "/");
        assert_eq!(t("/\\evil.com"), "/");
        assert_eq!(t("/.//evil.com"), "/");
        assert_eq!(t("\t//evil.com"), "/");
        assert_eq!(t("https://evil.com/"), "/");
        assert_eq!(t("javascript:alert(1)"), "/");
        assert_eq!(t("https://app.k7.rocks/p"), "/p");
        assert_eq!(t("https://docs.k7.rocks/p?q"), "/");
        assert_eq!(t(""), "/");
        assert_eq!(safe_return_target("/p", "https://App.k7.rocks"), "/");
    }
}
