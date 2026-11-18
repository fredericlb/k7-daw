//! http_file.rs — a file over HTTP as `streamFile` in `packages/http/stream.ts` serves it: Range, a
//! weak `W/"size-mtime"` validator, If-None-Match, If-Modified-Since and If-Range, and a body read as it is sent.
//! The thing a reader would get wrong: a malformed Range is not refused, it reads as `bytes=0-` (as there),
//! and only an unsatisfiable one is a 416 (`packages/http/docs/stream.md`).

use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::UNIX_EPOCH;

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use hyper::body::{Bytes, Frame, SizeHint};
use hyper::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG,
    IF_MODIFIED_SINCE, IF_NONE_MATCH, IF_RANGE, LAST_MODIFIED, RANGE,
};
use hyper::{HeaderMap, StatusCode};
use tokio::io::{AsyncRead, AsyncSeekExt, ReadBuf};

/// What a caller that sets no Cache-Control gets: keep it, but ask before every use.
const DEFAULT_CACHE_CONTROL: &str = "private, no-cache";

const CHUNK: usize = 64 * 1024;

/// The validator for a file at this size and modification time — weak, since it reads no bytes.
pub fn file_etag(size: u64, mtime_ms: u64) -> String {
    format!("W/\"{size:x}-{mtime_ms:x}\"")
}

/// Does an If-None-Match list name this tag? Weak comparison, as that header asks for.
pub fn etag_listed(header: &str, etag: &str) -> bool {
    if header.trim() == "*" {
        return true;
    }
    let bare = etag.strip_prefix("W/").unwrap_or(etag);
    header.split(',').any(|t| {
        let t = t.trim();
        t.strip_prefix("W/").unwrap_or(t) == bare
    })
}

fn header(headers: &HeaderMap, name: hyper::header::HeaderName) -> Option<&str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
}

fn http_ms(value: &str) -> Option<u64> {
    let t = httpdate::parse_http_date(value.trim()).ok()?;
    t.duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

/// HTTP dates have one-second precision, so a file's time is floored before it is compared.
fn to_http_second(ms: u64) -> u64 {
    ms / 1000 * 1000
}

fn not_modified(headers: &HeaderMap, etag: &str, mtime_ms: u64) -> bool {
    if let Some(inm) = header(headers, IF_NONE_MATCH) {
        return etag_listed(inm, etag);
    }
    header(headers, IF_MODIFIED_SINCE)
        .and_then(http_ms)
        .is_some_and(|ims| to_http_second(mtime_ms) <= ims)
}

/// Does an If-Range still describe this file? A stale one turns the Range into a full answer.
fn range_still_valid(headers: &HeaderMap, etag: &str, mtime_ms: u64) -> bool {
    let Some(if_range) = header(headers, IF_RANGE) else {
        return true;
    };
    if if_range.contains('"') {
        return if_range.trim() == etag;
    }
    http_ms(if_range).is_some_and(|at| to_http_second(mtime_ms) == at)
}

/// The first `bytes=<digits>-<digits>` anywhere in the header, as the unanchored regex there finds it.
fn find_range(range: &str) -> Option<(&str, &str)> {
    let mut rest = range;
    while let Some(at) = rest.find("bytes=") {
        let after = &rest[at + 6..];
        let first_len = after.bytes().take_while(u8::is_ascii_digit).count();
        if after[first_len..].starts_with('-') {
            let tail = &after[first_len + 1..];
            let second_len = tail.bytes().take_while(u8::is_ascii_digit).count();
            return Some((&after[..first_len], &tail[..second_len]));
        }
        rest = &rest[at + 1..];
    }
    None
}

/// The span `range` asks of a file of `size` bytes, or None when it cannot be satisfied (a 416).
pub fn span(range: &str, size: u64) -> Option<(u64, u64)> {
    let size = size as f64;
    let num = |d: &str| d.parse::<f64>().unwrap_or(f64::NAN);
    let (start, end) = match find_range(range) {
        Some(("", last)) if !last.is_empty() => ((size - num(last)).max(0.0), size - 1.0),
        Some((first, last)) => (
            if first.is_empty() { 0.0 } else { num(first) },
            if last.is_empty() {
                size - 1.0
            } else {
                num(last).min(size - 1.0)
            },
        ),
        None => (0.0, size - 1.0),
    };
    (start <= end && start < size).then_some((start as u64, end as u64))
}

/// The file at `path` as `mime`; `headers` are the request's, and `cache_control` the caller's own when it has one.
pub async fn stream_file(
    headers: &HeaderMap,
    path: &Path,
    mime: &'static str,
    cache_control: Option<&'static str>,
) -> Response {
    let Ok(meta) = std::fs::metadata(path) else {
        return crate::web::error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
    };
    let size = meta.len();
    let modified = meta.modified().unwrap_or(UNIX_EPOCH);
    let mtime_ms = modified
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let etag = file_etag(size, mtime_ms);
    let mut builder = Response::builder()
        .extension(crate::web::FileType)
        .header(ACCEPT_RANGES, "bytes")
        .header(ETAG, &etag)
        .header(LAST_MODIFIED, httpdate::fmt_http_date(modified))
        .header(
            CACHE_CONTROL,
            cache_control.unwrap_or(DEFAULT_CACHE_CONTROL),
        );
    if not_modified(headers, &etag, mtime_ms) {
        return finish(builder.status(StatusCode::NOT_MODIFIED), Body::empty());
    }
    builder = builder.header(CONTENT_TYPE, mime);
    let (mut start, mut end) = (0, size.saturating_sub(1));
    let range = header(headers, RANGE).filter(|_| range_still_valid(headers, &etag, mtime_ms));
    if let Some(range) = range {
        let Some((s, e)) = span(range, size) else {
            let unsatisfiable = builder
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(CONTENT_RANGE, format!("bytes */{size}"))
                .header(CONTENT_LENGTH, 0);
            return finish(unsatisfiable, Body::empty());
        };
        (start, end) = (s, e);
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    let length = if size == 0 { 0 } else { end - start + 1 };
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return crate::web::error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
    };
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return crate::web::error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
    }
    let body = FileBody {
        file,
        remaining: length,
        buf: vec![0; CHUNK.min(length as usize).max(1)],
    };
    finish(builder.header(CONTENT_LENGTH, length), Body::new(body))
}

fn finish(builder: hyper::http::response::Builder, body: Body) -> Response {
    builder
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// `length` bytes of `file` from where it stands, read as the client takes them.
pub(crate) fn file_body(file: tokio::fs::File, length: u64) -> Body {
    Body::new(FileBody {
        file,
        remaining: length,
        buf: vec![0; CHUNK.min(length as usize).max(1)],
    })
}

/// A file read a chunk at a time as the client takes it; dropping it, on an abort, closes the file.
struct FileBody {
    file: tokio::fs::File,
    remaining: u64,
    buf: Vec<u8>,
}

impl hyper::body::Body for FileBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        let this = &mut *self;
        if this.remaining == 0 {
            return Poll::Ready(None);
        }
        let want = this.buf.len().min(this.remaining as usize);
        let mut read = ReadBuf::new(&mut this.buf[..want]);
        match Pin::new(&mut this.file).poll_read(cx, &mut read) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Ready(Ok(())) if read.filled().is_empty() => Poll::Ready(Some(Err(
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the file shrank"),
            ))),
            Poll::Ready(Ok(())) => {
                let chunk = Bytes::copy_from_slice(read.filled());
                this.remaining -= chunk.len() as u64;
                Poll::Ready(Some(Ok(Frame::data(chunk))))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.remaining == 0
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_read_as_stream_ts_reads_them() {
        assert_eq!(span("bytes=2-5", 10), Some((2, 5)));
        assert_eq!(span("bytes=-3", 10), Some((7, 9)));
        assert_eq!(span("bytes=-30", 10), Some((0, 9)));
        assert_eq!(span("bytes=4-", 10), Some((4, 9)));
        assert_eq!(span("bytes=4-99", 10), Some((4, 9)));
        assert_eq!(span("bytes=10-", 10), None);
        assert_eq!(span("bytes=6-2", 10), None);
        assert_eq!(span("items=1-2", 10), Some((0, 9)));
        assert_eq!(span("x bytes=a bytes=3-4", 10), Some((3, 4)));
        assert_eq!(span("bytes=-", 10), Some((0, 9)));
        assert_eq!(span("bytes=-1", 0), None);
    }

    #[test]
    fn validators_compare_weakly() {
        let tag = file_etag(255, 4096);
        assert_eq!(tag, "W/\"ff-1000\"");
        assert!(etag_listed("\"ff-1000\"", &tag));
        assert!(etag_listed("\"x\", W/\"ff-1000\"", &tag));
        assert!(etag_listed(" * ", &tag));
        assert!(!etag_listed("\"ff-1001\"", &tag));
    }
}
