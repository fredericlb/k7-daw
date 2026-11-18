//! site.rs — what the root domain shows, `apps/k7/backend/src/sitePage.ts`: the splash and "Coming soon" at `/`, every
//! other address sent there; the download page and the tab's icon pass. The drawing is the frontend's own file.
//! What a reader gets wrong: the page is sent as Express sends a string, with its ETag and its 304.

use std::sync::OnceLock;

use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use hyper::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG};
use hyper::{Method, StatusCode};

use super::express;

/// The drawing, inlined as the TypeScript imports it.
const SPLASH: &str = include_str!("../../../../../../frontend/icon/k7-splash.inline.json");

/// What still passes on those hosts: `/app_downloads` and below, and `/favicon.ico`.
pub fn passes(path: &str) -> bool {
    path == "/favicon.ico"
        || path
            .strip_prefix("/app_downloads")
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// `SITE_PAGE`, built once.
fn page() -> &'static str {
    static PAGE: OnceLock<String> = OnceLock::new();
    PAGE.get_or_init(|| {
        let svg = serde_json::from_str::<serde_json::Value>(SPLASH)
            .ok()
            .and_then(|v| v.get("svg").and_then(|s| s.as_str()).map(str::to_string))
            .unwrap_or_default();
        format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"color-scheme\" content=\"dark\"><meta name=\"theme-color\" content=\"#07031F\">\n<title>K7 DAW — coming soon</title><style>\nhtml,body{{margin:0;height:100%;background:#07031F;color:#e9e9ee;font:16px/1.5 system-ui,-apple-system,\"Segoe UI\",sans-serif}}\nmain{{min-height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:40px;padding:24px;box-sizing:border-box}}\nsvg{{display:block;width:min(80vw,520px);height:auto}}\nh1{{margin:0;font-size:clamp(20px,4vw,28px);font-weight:500;letter-spacing:.08em;color:#b8b3d9}}\n</style></head><body><main>{svg}<h1>Coming soon</h1></main></body></html>"
        )
    })
}

/// On a site host: `/` is the page, every other address or method goes to `/`.
pub fn answer(req: &Request) -> Response {
    let read = matches!(*req.method(), Method::GET | Method::HEAD);
    if req.uri().path() != "/" || !read {
        return express::redirect(req, StatusCode::FOUND, "/");
    }
    let page = page();
    let etag = express::body_etag(page.as_bytes());
    let builder = Response::builder()
        .header(CACHE_CONTROL, "no-cache")
        .header(ETAG, &etag);
    if express::fresh(req.headers(), Some(&etag), None) {
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap_or_default();
    }
    let body = if req.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from(page)
    };
    builder
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header(CONTENT_LENGTH, page.len())
        .body(body)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_holds_the_drawing_and_passes_two_paths() {
        assert!(
            page().contains("<svg viewBox=")
                && page().ends_with("<h1>Coming soon</h1></main></body></html>")
        );
        assert!(
            passes("/app_downloads") && passes("/app_downloads/tray") && passes("/favicon.ico")
        );
        assert!(!passes("/app_downloadsx") && !passes("/") && !passes("/favicon.ico/x"));
    }
}
