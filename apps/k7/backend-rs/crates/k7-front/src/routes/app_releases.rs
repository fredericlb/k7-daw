//! app_releases.rs — the apps' downloads: CI uploads
//! a build under a bearer only it and this server know (`/api/app-releases`, the tray's under `/tray`), and anyone
//! fetches it at `/app_downloads` — the page, `<channel>.json`, and every file of a build (docs/app-downloads.md).
//! The thing a reader would get wrong: an upload is streamed to disk as it arrives, never collected first, up to 2 GB.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use hyper::header::{
    ACCEPT_LANGUAGE, AUTHORIZATION, CACHE_CONTROL, CONNECTION, CONTENT_DISPOSITION, CONTENT_TYPE,
    LOCATION,
};
use hyper::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::routes::http_file::stream_file;
use crate::routes::release_page::{downloads_page, prefers_french};
use crate::routes::release_store::{Channel, SaveError, Store, channel_of, is_file_name};
use crate::web::error;

/// Which store a route reads: the apps' at the root, the tray's in `tray/`.
#[derive(Clone, Copy)]
pub enum Product {
    Main,
    Tray,
}

/// The two stores under `STUDIO_APP_RELEASES_DIR`; neither publish drops the other's files.
pub struct Stores {
    main: Store,
    tray: Store,
}

impl Stores {
    pub fn new(root: &std::path::Path) -> Self {
        Stores {
            main: Store::new(root.to_path_buf()),
            tray: Store::new(root.join("tray")),
        }
    }

    fn of(&self, product: Product) -> &Store {
        match product {
            Product::Main => &self.main,
            Product::Tray => &self.tray,
        }
    }
}

/// Whether a request is a family path: the front hands all of it to the backend on a device, where it is not mounted.
pub fn is_family_path(path: &str) -> bool {
    path.starts_with("/api/app-releases/")
        || path == "/app_downloads"
        || path.starts_with("/app_downloads/")
}

/// The CI's bearer, compared by digest in constant time; no token configured refuses everyone.
fn authorized(headers: &HeaderMap, token: Option<&str>) -> bool {
    let Some(expected) = token else {
        return false;
    };
    let sent = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(bearer);
    sent.is_some_and(|sent| {
        let (a, b) = (
            Sha256::digest(sent.as_bytes()),
            Sha256::digest(expected.as_bytes()),
        );
        a.iter()
            .zip(b.iter())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
    })
}

/// What `/^Bearer\s+(.+)$/i` captures.
fn bearer(header: &str) -> Option<&str> {
    let scheme = header.get(..6)?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let rest = &header[6..];
    let trimmed = rest.trim_start_matches(|c: char| c.is_whitespace());
    if trimmed.len() == rest.len() {
        return None;
    }
    // A run of whitespace alone still captures its last character, as the backtracking regex does.
    if trimmed.is_empty() {
        return rest.char_indices().last().map(|(i, _)| &rest[i..]);
    }
    Some(trimmed)
}

fn unauthorized() -> Response {
    error(StatusCode::UNAUTHORIZED, "unauthorized")
}

/// PUT `<stamp>/<file>`: one file of a build, streamed into its store's incoming folder.
pub async fn upload(
    State(state): State<Arc<AppState>>,
    product: Product,
    Path((stamp, file)): Path<(String, String)>,
    req: Request,
) -> Response {
    let settings = &state.settings;
    if !authorized(req.headers(), settings.app_release_token.as_deref()) {
        return unauthorized();
    }
    if channel_of(&stamp).is_none() || !is_file_name(&file) {
        return error(StatusCode::BAD_REQUEST, "invalid stamp or file");
    }
    let store = state.app_releases.of(product);
    match store
        .save_file(
            &stamp,
            &file,
            req.into_body(),
            settings.app_release_max_bytes,
        )
        .await
    {
        Ok(saved) => (StatusCode::CREATED, axum::Json(saved)).into_response(),
        Err(SaveError::TooLarge(message)) => {
            let mut r = error(StatusCode::PAYLOAD_TOO_LARGE, &message);
            r.headers_mut()
                .insert(CONNECTION, hyper::header::HeaderValue::from_static("close"));
            r
        }
        Err(SaveError::Failed) => error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
    }
}

/// POST `<stamp>/publish`: the build becomes its channel's current one at once.
pub async fn publish(
    State(state): State<Arc<AppState>>,
    product: Product,
    Path((stamp, _)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers, state.settings.app_release_token.as_deref()) {
        return unauthorized();
    }
    if channel_of(&stamp).is_none() {
        return error(StatusCode::BAD_REQUEST, "invalid stamp");
    }
    match state.app_releases.of(product).publish(&stamp) {
        Ok(Some(release)) => axum::Json(release).into_response(),
        Ok(None) => error(
            StatusCode::NOT_FOUND,
            &format!("nothing uploaded for {stamp}"),
        ),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
    }
}

fn no_cache(mut r: Response) -> Response {
    r.headers_mut().insert(
        CACHE_CONTROL,
        hyper::header::HeaderValue::from_static("no-cache"),
    );
    r
}

/// `release.json` / `snapshot.json`: the build a channel serves, for an app's check.
pub async fn feed(State(state): State<Arc<AppState>>, product: Product, name: String) -> Response {
    let build = name
        .strip_suffix(".json")
        .filter(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .and_then(Channel::parse)
        .and_then(|c| state.app_releases.of(product).current(c));
    no_cache(match build {
        Some(build) => axum::Json(build).into_response(),
        None => error(StatusCode::NOT_FOUND, "not found"),
    })
}

/// One file of a channel's current or previous build, by ranges, text for what an updater reads.
pub async fn file(
    State(state): State<Arc<AppState>>,
    product: Product,
    (channel, name): (String, String),
    headers: HeaderMap,
) -> Response {
    let found =
        Channel::parse(&channel).and_then(|c| state.app_releases.of(product).file_path(c, &name));
    let Some(found) = found else {
        let mut r = (StatusCode::NOT_FOUND, "not found").into_response();
        r.headers_mut().insert(
            CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        return r;
    };
    let text = [".yml", ".json", ".sig"].iter().any(|e| name.ends_with(e)) || name == "SHA256SUMS";
    let mime = if text {
        "text/plain"
    } else {
        "application/octet-stream"
    };
    let mut r = stream_file(&headers, &found, mime, Some("no-cache")).await;
    if !text {
        r.headers_mut().insert(
            CONTENT_DISPOSITION,
            hyper::header::HeaderValue::from_static("attachment"),
        );
    }
    r
}

/// The page, in French or English; `/app_downloads/` redirects, since its links resolve against the bare path.
pub async fn page(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if req.uri().path().ends_with('/') {
        let mut r = (
            StatusCode::MOVED_PERMANENTLY,
            "Moved Permanently. Redirecting to /app_downloads",
        )
            .into_response();
        let h = r.headers_mut();
        h.insert(
            LOCATION,
            hyper::header::HeaderValue::from_static("/app_downloads"),
        );
        h.insert(
            CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        return r;
    }
    let s = &state.app_releases;
    let builds = |store: &Store| {
        [
            store.current(Channel::Release),
            store.current(Channel::Snapshot),
        ]
    };
    let french = prefers_french(
        req.headers()
            .get(ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok()),
    );
    let html = downloads_page(&builds(&s.main), &builds(&s.tray), french);
    let mut r = no_cache(html.into_response());
    r.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    r
}

type Pair = Path<(String, String)>;
type Shared = State<Arc<AppState>>;

/// The family's routes; a literal `tray` wins over a channel, as the tray's router mounted first does there.
pub fn router() -> Router<Arc<AppState>> {
    let uploads = |product: Product| {
        put(move |s: Shared, p: Pair, req: Request| upload(s, product, p, req))
            .post(move |s: Shared, p: Pair, h: HeaderMap| publish(s, product, p, h))
    };
    let feeds = |product: Product| get(move |s: Shared, Path(n): Path<String>| feed(s, product, n));
    let files = |product: Product| {
        get(move |s: Shared, Path(p): Pair, h: HeaderMap| file(s, product, p, h))
    };
    Router::new()
        .route(
            "/api/app-releases/tray/{stamp}/{file}",
            uploads(Product::Tray),
        )
        .route("/api/app-releases/{stamp}/{file}", uploads(Product::Main))
        .route("/app_downloads", get(page))
        .route("/app_downloads/", get(page))
        .route("/app_downloads/tray/{name}", feeds(Product::Tray))
        .route("/app_downloads/{name}", feeds(Product::Main))
        .route("/app_downloads/tray/{channel}/{file}", files(Product::Tray))
        .route("/app_downloads/{channel}/{file}", files(Product::Main))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bearer_reads_as_the_regex_does() {
        assert_eq!(bearer("Bearer abc"), Some("abc"));
        assert_eq!(bearer("bearer \t abc def"), Some("abc def"));
        assert_eq!(bearer("Bearerabc"), None);
        assert_eq!(bearer("Basic abc"), None);
        assert_eq!(bearer("Bearer   "), Some(" "));
        let mut h = HeaderMap::new();
        h.insert(AUTHORIZATION, "Bearer ci".parse().unwrap());
        assert!(authorized(&h, Some("ci")));
        assert!(!authorized(&h, Some("other")));
        assert!(!authorized(&h, None));
    }
}
