//! frontend.rs — the built page through tower-http's `ServeDir`: the `.br` or `.gz` the build wrote beside a file,
//! ranges and conditional GETs, then the shell for any page address, else a 404. Hashed assets are kept a year,
//! everything else asked again; an HTML file may be framed by its own origin only.
//! What a reader gets wrong: `/assets/` never falls back to the shell, or a page would parse it as a script.

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue, VARY};
use hyper::{Method, StatusCode};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::settings::Settings;

/// The shell is asked again on every load; a hashed asset is kept for good (`docs/versioning.md`).
const SHELL_CACHE: &str = "no-cache";
const ASSET_CACHE: &str = "public, max-age=31536000, immutable";

/// The built page's two services: the files alone, and the files with the shell behind them.
struct Built {
    files: ServeDir,
    shell: ServeDir<ServeFile>,
}

/// Where the built page is, found once.
pub struct Frontend {
    built: Option<Built>,
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
        let dist: Option<PathBuf> = settings.frontend_dist.clone().or_else(found);
        match &dist {
            Some(d) => eprintln!("k7-front: the page is served from {}", d.display()),
            None => eprintln!(
                "k7-front: no built page (K7_FRONTEND_DIST, or apps/k7/frontend/dist above the front): pages answer 404"
            ),
        }
        Frontend {
            built: dist.map(|d| Built {
                files: ServeDir::new(&d).precompressed_br().precompressed_gzip(),
                shell: ServeDir::new(&d)
                    .precompressed_br()
                    .precompressed_gzip()
                    .fallback(
                        ServeFile::new(d.join("index.html"))
                            .precompressed_br()
                            .precompressed_gzip(),
                    ),
            }),
        }
    }

    /// A request the routes before left: a file, the shell, or the 404.
    pub async fn answer(&self, req: Request) -> Response {
        let path = req.uri().path().to_string();
        let read = matches!(*req.method(), Method::GET | Method::HEAD);
        let (Some(built), true) = (&self.built, read) else {
            return not_found();
        };
        // A file whose name starts with a dot is never part of the page.
        if path.split('/').any(|s| s.starts_with('.') && s.len() > 1) {
            return not_found();
        }
        let page = !(path.starts_with("/api")
            || path.starts_with("/auth")
            || path.starts_with("/assets/"));
        let served = if page {
            built.shell.clone().oneshot(req).await
        } else {
            built.files.clone().oneshot(req).await
        };
        let Ok(mut res) = served;
        if res.status() == StatusCode::NOT_FOUND {
            return not_found();
        }
        if res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED {
            let h = res.headers_mut();
            let cache = if path.starts_with("/assets/") {
                ASSET_CACHE
            } else {
                SHELL_CACHE
            };
            h.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
            h.entry(VARY)
                .or_insert(HeaderValue::from_static("Accept-Encoding"));
            if h.get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|t| t.starts_with("text/html"))
            {
                h.insert(
                    "content-security-policy",
                    HeaderValue::from_static("frame-ancestors 'self'"),
                );
                h.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
            }
        }
        res.map(Body::new)
    }
}

/// What no route and no file answers.
pub fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}
