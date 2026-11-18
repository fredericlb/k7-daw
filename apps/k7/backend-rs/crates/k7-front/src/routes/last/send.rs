//! send.rs — a file answered as the `send` package 0.19 answers it under `express.static` and `res.sendFile`: the
//! headers a caller set kept, then Accept-Ranges, Last-Modified, a weak `W/"size-mtime"` ETag and mime 1's type;
//! If-Match and If-Unmodified-Since (412), a fresh request's 304, a single range's 206, an unsatisfiable one's 416.
//! What a reader gets wrong: a malformed or multiple range is no error, it is the whole file (range-parser's -2).

use std::path::Path;
use std::time::UNIX_EPOCH;

use axum::body::Body;
use axum::response::Response;
use hyper::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG, HeaderName,
    HeaderValue, IF_MATCH, IF_RANGE, IF_UNMODIFIED_SINCE, LAST_MODIFIED, RANGE,
};
use hyper::{HeaderMap, Method, StatusCode};
use tokio::io::AsyncSeekExt;

use super::express::{fresh, tokens};
use crate::routes::http_file::{file_body, file_etag};

/// Why `send` refused, with the headers it had set by then, which the backend's error handler keeps.
pub enum Refused {
    Precondition(HeaderMap),
    /// The headers, the 416's `Content-Range` among them.
    Range(HeaderMap),
    /// A file that could not be read.
    Gone,
}

/// mime 1.6's type for a file name, by its extension, `application/octet-stream` for one it does not know.
pub fn mime(name: &str) -> &'static str {
    let ext = name
        .rsplit(['.', '/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "application/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "xml" => "application/xml",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/x-flac",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "yaml" | "yml" => "text/yaml",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
}

/// `mime.charsets.lookup`: text and JavaScript or JSON are UTF-8.
pub fn charset(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.starts_with("application/javascript")
        || mime.starts_with("application/json")
}

/// JavaScript's `parseInt(s, 10)` on what a range names: leading blanks, a sign, then digits.
fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let n: String = digits.chars().take_while(char::is_ascii_digit).collect();
    let v = n.parse::<i64>().ok()?;
    Some(if neg { -v } else { v })
}

/// range-parser with `combine`: Err for unsatisfiable (-1), Ok(None) for malformed or several (-2), else the one span.
fn range(size: u64, header: &str) -> Result<Option<(u64, u64)>, ()> {
    let Some((_, list)) = header.split_once('=') else {
        return Ok(None);
    };
    let size = size as i64;
    let mut spans: Vec<(i64, i64)> = Vec::new();
    for part in list.split(',') {
        let mut halves = part.split('-');
        let (a, b) = (
            halves.next().and_then(parse_int),
            halves.next().and_then(parse_int),
        );
        let (start, end) = match (a, b) {
            (None, Some(b)) => (size - b, size - 1),
            (None, None) => continue,
            (Some(a), None) => (a, size - 1),
            (Some(a), Some(b)) => (a, b),
        };
        let end = end.min(size - 1);
        if start > end || start < 0 {
            continue;
        }
        spans.push((start, end));
    }
    if spans.is_empty() {
        return Err(());
    }
    spans.sort_by_key(|s| s.0);
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (s, e) in spans {
        match merged.last_mut() {
            Some(last) if s <= last.1 + 1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    Ok((merged.len() == 1).then(|| (merged[0].0 as u64, merged[0].1 as u64)))
}

fn header(h: &HeaderMap, n: HeaderName) -> Option<&str> {
    h.get(n)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
}

fn http_date(v: &str) -> Option<std::time::SystemTime> {
    httpdate::parse_http_date(v.trim()).ok()
}

/// `isPreconditionFailure`: an If-Match no tag satisfies, or a file changed since If-Unmodified-Since.
fn precondition_failed(req: &HeaderMap, etag: &str, last_modified: &str) -> bool {
    if let Some(m) = header(req, IF_MATCH) {
        return m != "*"
            && tokens(m)
                .all(|t| t != etag && format!("W/{etag}") != t && format!("W/{t}") != etag);
    }
    match header(req, IF_UNMODIFIED_SINCE).and_then(http_date) {
        Some(since) => http_date(last_modified).is_none_or(|m| m > since),
        None => false,
    }
}

/// `isRangeFresh`: no If-Range, or one naming this file's tag or date.
fn range_fresh(req: &HeaderMap, etag: &str, last_modified: &str) -> bool {
    let Some(v) = header(req, IF_RANGE) else {
        return true;
    };
    if v.contains('"') {
        return v.contains(etag);
    }
    http_date(last_modified)
        .zip(http_date(v))
        .is_some_and(|(m, at)| m <= at)
}

/// The file at `path`, with `preset` the headers the caller set before `send` ran; `ranges` is its `acceptRanges`.
pub async fn send(
    req: &HeaderMap,
    method: &Method,
    path: &Path,
    preset: HeaderMap,
    ranges: bool,
) -> Result<Response, Refused> {
    let meta = std::fs::metadata(path).map_err(|_| Refused::Gone)?;
    let size = meta.len();
    let modified = meta.modified().unwrap_or(UNIX_EPOCH);
    let mtime_ms = modified
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let mut h = preset;
    if ranges {
        h.entry(ACCEPT_RANGES)
            .or_insert(HeaderValue::from_static("bytes"));
    }
    h.entry(CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("public, max-age=0"));
    let last_modified = httpdate::fmt_http_date(modified);
    let etag = file_etag(size, mtime_ms);
    for (name, value) in [(LAST_MODIFIED, &last_modified), (ETAG, &etag)] {
        if let Ok(v) = HeaderValue::from_str(value) {
            h.entry(name).or_insert(v);
        }
    }
    if !h.contains_key(CONTENT_TYPE) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let m = mime(&name);
        let value = if charset(m) {
            format!("{m}; charset=UTF-8")
        } else {
            m.to_string()
        };
        if let Ok(v) = HeaderValue::from_str(&value) {
            h.insert(CONTENT_TYPE, v);
        }
    }
    let conditional = [
        IF_MATCH,
        IF_UNMODIFIED_SINCE,
        hyper::header::IF_NONE_MATCH,
        hyper::header::IF_MODIFIED_SINCE,
    ]
    .into_iter()
    .any(|n| header(req, n).is_some());
    let shown_tag = h
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if conditional {
        if precondition_failed(req, &shown_tag, &last_modified) {
            return Err(Refused::Precondition(h));
        }
        if fresh(req, Some(&shown_tag), Some(&last_modified)) {
            for n in [
                "content-encoding",
                "content-language",
                "content-length",
                "content-range",
                "content-type",
            ] {
                h.remove(n);
            }
            return Ok(finish(StatusCode::NOT_MODIFIED, h, Body::empty()));
        }
    }
    let (mut start, mut length, mut status) = (0, size, StatusCode::OK);
    let asked =
        header(req, RANGE).filter(|r| ranges && r.trim_start_matches(' ').starts_with("bytes="));
    if let Some(asked) = asked {
        let Ok(span) = range(size, asked) else {
            if let Ok(v) = HeaderValue::from_str(&format!("bytes */{size}")) {
                h.insert(CONTENT_RANGE, v);
            }
            return Err(Refused::Range(h));
        };
        if let Some((s, e)) = span.filter(|_| range_fresh(req, &shown_tag, &last_modified)) {
            status = StatusCode::PARTIAL_CONTENT;
            if let Ok(v) = HeaderValue::from_str(&format!("bytes {s}-{e}/{size}")) {
                h.insert(CONTENT_RANGE, v);
            }
            (start, length) = (s, e - s + 1);
        }
    }
    h.insert(CONTENT_LENGTH, HeaderValue::from(length));
    if *method == Method::HEAD {
        return Ok(finish(status, h, Body::empty()));
    }
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| Refused::Gone)?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|_| Refused::Gone)?;
    Ok(finish(status, h, file_body(file, length)))
}

fn finish(status: StatusCode, headers: HeaderMap, body: Body) -> Response {
    let mut r = Response::new(body);
    *r.status_mut() = status;
    *r.headers_mut() = headers;
    r.extensions_mut().insert(crate::web::FileType);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_read_as_range_parser_reads_them() {
        assert_eq!(range(10, "bytes=2-5"), Ok(Some((2, 5))));
        assert_eq!(range(10, "bytes=-3"), Ok(Some((7, 9))));
        assert_eq!(range(10, "bytes=4-"), Ok(Some((4, 9))));
        assert_eq!(range(10, "bytes=4-99"), Ok(Some((4, 9))));
        assert_eq!(range(10, "bytes=0-2,3-5"), Ok(Some((0, 5))));
        assert_eq!(range(10, "bytes=0-1,5-6"), Ok(None));
        assert_eq!(range(10, "bytes=10-"), Err(()));
        assert_eq!(range(10, "bytes=-30"), Err(()));
        assert_eq!(range(10, "bytes"), Ok(None));
        assert_eq!(range(10, "bytes=a-b"), Err(()));
    }

    #[test]
    fn types_are_mime_ones() {
        assert_eq!(mime("index.html"), "text/html");
        assert_eq!(mime("AboutPage-Bhusu5PF.JS"), "application/javascript");
        assert_eq!(mime("LICENSE"), "application/octet-stream");
        assert!(charset("text/css") && charset("application/json") && !charset("image/svg+xml"));
    }
}
