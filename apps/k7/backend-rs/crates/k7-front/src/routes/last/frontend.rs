//! frontend.rs — the built page as `server.ts` serves `frontend/dist`: the `.br` or `.gz` the build wrote beside a file
//! (`precompressedStatic`), else `express.static`, else the shell for any page address, else Express's 404. Hashed
//! assets are kept a year, everything else asked again; an HTML file may be framed by its own origin only.
//! What a reader gets wrong: `/assets/` never falls back to the shell, or a page would parse it as a script.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use hyper::header::{
    ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue,
    LOCATION, VARY,
};
use hyper::http::request::Parts;
use hyper::{HeaderMap, Method, StatusCode};

use super::express::{self, escape_html};
use super::send::{self, Refused};
use crate::settings::Settings;
use crate::web::error;

/// Best first: brotli is a fifth smaller than gzip on a bundle.
const ENCODINGS: [(&str, &str); 2] = [("br", ".br"), ("gzip", ".gz")];

/// The shell is asked again on every load; a hashed asset is kept for good (`docs/versioning.md`).
const SHELL_CACHE: &str = "no-cache";
const ASSET_CACHE: &str = "public, max-age=31536000, immutable";

/// Where the built page is, found once.
pub struct Frontend {
    dist: Option<PathBuf>,
}

impl Frontend {
    /// `K7_FRONTEND_DIST`, else the first `apps/k7/frontend/dist` with an `index.html` above the front's folder or binary.
    pub fn find(settings: &Settings) -> Frontend {
        let found = || {
            let starts = [std::env::current_dir().ok(), std::env::current_exe().ok()];
            starts.into_iter().flatten().find_map(|start| {
                start
                    .ancestors()
                    .map(|a| a.join("apps").join("k7").join("frontend").join("dist"))
                    .find(|d| d.join("index.html").is_file())
            })
        };
        let dist = settings.frontend_dist.clone().or_else(found);
        match &dist {
            Some(d) => eprintln!("k7-front: the page is served from {}", d.display()),
            None => eprintln!(
                "k7-front: no built page (K7_FRONTEND_DIST, or apps/k7/frontend/dist above the front): pages answer 404"
            ),
        }
        Frontend { dist }
    }

    /// A request the routes before left: a file, the shell, or the 404.
    pub async fn answer(&self, req: Request) -> Response {
        let (req, _) = req.into_parts();
        let read = matches!(req.method, Method::GET | Method::HEAD);
        let (Some(dist), true) = (&self.dist, read) else {
            return express::not_found(&req.method, req.uri.path(), false);
        };
        let path = req.uri.path().to_string();
        let mut vary = false;
        if let Some(file) = decode(&path)
            .filter(|rel| !rel.ends_with('/') && !rel.contains("/."))
            .and_then(|rel| inside(dist, &rel))
        {
            vary = true;
            let taken = req
                .headers
                .get(ACCEPT_ENCODING)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            for (name, ext) in ENCODINGS {
                let sibling = with_suffix(&file, ext);
                if accepts(taken, name) && sibling.is_file() {
                    return with_vary(compressed(dist, &req, &file, &sibling, name).await, vary);
                }
            }
        }
        if let Some(r) = static_file(dist, &req, &path).await {
            return with_vary(r, vary);
        }
        if !(path.starts_with("/api") || path.starts_with("/auth") || path.starts_with("/assets/"))
        {
            return with_vary(shell(dist, &req).await, vary);
        }
        express::not_found(&req.method, &path, vary)
    }
}

/// `Vary: Accept-Encoding`, which the precompressed step names before it hands on.
fn with_vary(mut r: Response, vary: bool) -> Response {
    if vary {
        r.headers_mut()
            .entry(VARY)
            .or_insert(HeaderValue::from_static("Accept-Encoding"));
    }
    r
}

fn with_suffix(file: &Path, ext: &str) -> PathBuf {
    let mut s = file.as_os_str().to_owned();
    s.push(ext);
    PathBuf::from(s)
}

/// `decodeURIComponent`: None for a broken escape or bytes that are not UTF-8.
fn decode(path: &str) -> Option<String> {
    let b = path.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The path's segments with `.` and `..` resolved as `path.normalize` does; None when one climbs out.
fn segments(rel: &str) -> Option<Vec<&str>> {
    let mut out: Vec<&str> = Vec::new();
    let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    for s in rel.split(separators) {
        match s {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s),
        }
    }
    Some(out)
}

/// The file a decoded path names inside `dist`, never `dist` itself nor above it.
fn inside(dist: &Path, rel: &str) -> Option<PathBuf> {
    let parts = segments(rel).filter(|p| !p.is_empty())?;
    Some(parts.iter().fold(dist.to_path_buf(), |p, s| p.join(s)))
}

/// Whether `Accept-Encoding` takes `name`; a `q=0` refuses it.
fn accepts(header: &str, name: &str) -> bool {
    header.split(',').any(|part| {
        let mut params = part.trim().split(';');
        if params.next().unwrap_or("").trim().to_ascii_lowercase() != name {
            return false;
        }
        params
            .map(str::trim)
            .find(|p| p.starts_with("q="))
            .is_none_or(|q| q[2..].trim().parse::<f64>().is_ok_and(|v| v > 0.0))
    })
}

/// What `setHeaders` puts on every file it is shown: the cache rule, and the framing rule on HTML.
fn shown_headers(dist: &Path, file: &Path) -> HeaderMap {
    let mut h = HeaderMap::new();
    let asset = file
        .to_string_lossy()
        .starts_with(&*dist.join("assets").to_string_lossy());
    h.insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if asset { ASSET_CACHE } else { SHELL_CACHE }),
    );
    if file.to_string_lossy().ends_with(".html") {
        frame(&mut h);
    }
    h
}

/// `shellFrameHeaders`: the recorder frames the bench, nobody else frames the app.
fn frame(h: &mut HeaderMap) {
    h.insert(
        "content-security-policy",
        HeaderValue::from_static("frame-ancestors 'self'"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
}

/// An error `res.sendFile` hands on, as the backend's last handler answers it: JSON, over the file's headers.
fn refused(r: Refused) -> Response {
    let (status, message, mut h) = match r {
        Refused::Precondition(h) => (StatusCode::PRECONDITION_FAILED, "Precondition Failed", h),
        Refused::Range(h) => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            "Range Not Satisfiable",
            h,
        ),
        Refused::Gone => return error(StatusCode::NOT_FOUND, "not found"),
    };
    // `res.json` keeps a type already set, and `res.send` names its charset utf-8.
    let typed = h
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or("application/json".to_string(), |t| {
            t.split(';').next().unwrap_or("").trim().to_string()
        });
    if let Ok(v) = HeaderValue::from_str(&format!("{typed}; charset=utf-8")) {
        h.insert(CONTENT_TYPE, v);
    }
    // A compressed sibling's encoding would mislabel the JSON.
    h.remove(CONTENT_ENCODING);
    let body = serde_json::json!({ "error": message }).to_string();
    h.insert(CONTENT_LENGTH, HeaderValue::from(body.len()));
    let mut res = Response::new(Body::from(body));
    *res.status_mut() = status;
    *res.headers_mut() = h;
    res.extensions_mut().insert(crate::web::FileType);
    res
}

/// The sibling the build compressed, typed as the original and never ranged.
async fn compressed(
    dist: &Path,
    req: &Parts,
    file: &Path,
    sibling: &Path,
    encoding: &'static str,
) -> Response {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let m = send::mime(&name);
    let typed = if send::charset(m) {
        format!("{m}; charset=utf-8")
    } else {
        m.to_string()
    };
    let mut h = shown_headers(dist, file);
    if let Ok(v) = HeaderValue::from_str(&typed) {
        h.insert(CONTENT_TYPE, v);
    }
    h.insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
    send::send(&req.headers, &req.method, sibling, h, false)
        .await
        .unwrap_or_else(refused)
}

/// `express.static`: None hands on, as its fall-through does for a missing file, a dotfile or a bad path.
async fn static_file(dist: &Path, req: &Parts, raw: &str) -> Option<Response> {
    let decoded = decode(raw).filter(|d| !d.contains('\0'))?;
    let parts = segments(&decoded)?;
    let trailing = raw.ends_with('/');
    // send's legacy dotfile rule: hidden only when the last segment is the dotfile.
    let dotted = parts.iter().any(|p| p.len() > 1 && p.starts_with('.'));
    if dotted && !trailing && parts.last().is_some_and(|p| p.starts_with('.')) {
        return None;
    }
    let file = parts.iter().fold(dist.to_path_buf(), |p, s| p.join(s));
    let target = if trailing {
        file.join("index.html")
    } else {
        file.clone()
    };
    let meta = std::fs::metadata(&target).ok()?;
    if meta.is_dir() {
        return (!trailing).then(|| directory(req, raw));
    }
    // Once the file is found serve-static forwards a refusal rather than falling through.
    match send::send(
        &req.headers,
        &req.method,
        &target,
        shown_headers(dist, &target),
        true,
    )
    .await
    {
        Ok(r) => Some(r),
        Err(Refused::Gone) => None,
        Err(r) => Some(refused(r)),
    }
}

/// serve-static's redirect of a folder named without its slash.
fn directory(req: &Parts, raw: &str) -> Response {
    let collapsed = format!("/{}/", raw.trim_start_matches('/'));
    let search = req.uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let location = express::encode_url(&format!("{collapsed}{search}"));
    let doc = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Redirecting</title>\n</head>\n<body>\n<pre>Redirecting to {}</pre>\n</body>\n</html>\n",
        escape_html(&location)
    );
    let body = if req.method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(doc.clone())
    };
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(CONTENT_TYPE, "text/html; charset=UTF-8")
        .header(CONTENT_LENGTH, doc.len())
        .header("content-security-policy", "default-src 'none'")
        .header("x-content-type-options", "nosniff")
        .header(
            LOCATION,
            HeaderValue::from_str(&location).unwrap_or(HeaderValue::from_static("/")),
        )
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// The SPA's shell for any page address.
async fn shell(dist: &Path, req: &Parts) -> Response {
    let mut h = HeaderMap::new();
    h.insert(CACHE_CONTROL, HeaderValue::from_static(SHELL_CACHE));
    frame(&mut h);
    send::send(&req.headers, &req.method, &dist.join("index.html"), h, true)
        .await
        .unwrap_or_else(refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_decode_and_resolve_as_node_does() {
        assert_eq!(decode("/a%20b").as_deref(), Some("/a b"));
        assert_eq!(decode("/a%zz"), None);
        assert_eq!(decode("/%ff"), None);
        assert_eq!(segments("/a/../b/./c/"), Some(vec!["b", "c"]));
        assert_eq!(segments("/a/../../b"), None);
        assert!(inside(Path::new("/d"), "/").is_none());
    }

    #[test]
    fn encodings_are_taken_unless_refused() {
        assert!(accepts("gzip, deflate, br", "br"));
        assert!(accepts("br;q=0.5", "br"));
        assert!(!accepts("br;q=0", "br"));
        assert!(!accepts("gzip", "br"));
    }
}
