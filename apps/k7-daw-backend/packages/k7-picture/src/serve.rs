//! serve.rs — the one-off loopback server the render page talks to: its script, job, mixdown and media in, its film out.
//! What a reader gets wrong: the film is written to a part file as it arrives (2505), never held in memory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

use crate::Rendered;

/// The largest film the render page may post back (2505).
pub(crate) const RESULT_LIMIT: u64 = 2 << 30;

/// The largest chunk of base64 text a post may carry; the page sends 3 MiB of film at a time.
const CHUNK_TEXT_LIMIT: usize = 16 << 20;

/// The page around the script: nothing but the script, which does the rest.
const PAGE: &str = r#"<!doctype html><html><body><script src="page.js"></script></body></html>"#;

pub(crate) type Settle = Arc<Mutex<Option<oneshot::Sender<Result<Rendered, String>>>>>;

pub(crate) struct Served {
    pub token: String,
    pub script: PathBuf,
    pub job: String,
    pub mix: Option<PathBuf>,
    pub media: HashMap<String, PathBuf>,
    pub settle: Settle,
    pub debug: bool,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
    /// Where a film is written, as `<out>`'s dot-named part file.
    pub out: PathBuf,
    pub limit: u64,
    /// The URLs the browser was refused (2506), shown to the caller with the film.
    pub blocked: Arc<Mutex<Vec<String>>>,
    /// The film so far; dropped unclaimed, its part file goes with it.
    pub film: tokio::sync::Mutex<Option<Rendered>>,
}

/// Hands the render's end to its waiter; a result nobody waits for is dropped, its part file with it.
pub(crate) fn settle(s: &Settle, r: Result<Rendered, String>) {
    if let Some(tx) = s.lock().unwrap_or_else(|p| p.into_inner()).take() {
        let _ = tx.send(r);
    }
}

fn status(code: StatusCode) -> Response {
    code.into_response()
}

/// A dot-named file beside `out`, so no walk of its folder counts a film before it is whole.
pub fn part_for(out: &Path) -> PathBuf {
    let name = out
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    out.with_file_name(format!(".{name}.part-{}", token()))
}

/// `bytes=a-b`, `bytes=a-` and `bytes=-n` against `size`, as pictureRender.ts's `sendFile` reads them.
pub(crate) fn range_of(h: &HeaderMap, size: u64) -> Option<(u64, u64)> {
    let r = h
        .get(header::RANGE)?
        .to_str()
        .ok()?
        .strip_prefix("bytes=")?;
    let (a, b) = r.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    if !a.chars().all(|c| c.is_ascii_digit())
        || !b.chars().all(|c| c.is_ascii_digit())
        || (a.is_empty() && b.is_empty())
    {
        return None;
    }
    let last = size.saturating_sub(1);
    let start = if a.is_empty() {
        size.saturating_sub(b.parse().unwrap_or(0))
    } else {
        a.parse().ok()?
    };
    let end = if !a.is_empty() && !b.is_empty() {
        b.parse::<u64>().ok()?.min(last)
    } else {
        last
    };
    Some((start, end))
}

/// `len` bytes of `file` as they are read: a film is never held whole.
fn body_of(file: tokio::fs::File, len: u64) -> Body {
    use tokio::io::AsyncReadExt;
    let stream = futures_util::stream::unfold((file, len), |(mut f, left)| async move {
        if left == 0 {
            return None;
        }
        let mut buf = vec![0u8; (64 << 10).min(left as usize)];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((
                    Ok::<_, std::io::Error>(Bytes::from(buf)),
                    (f, left - n as u64),
                ))
            }
            Err(e) => Some((Err(e), (f, 0))),
        }
    });
    Body::from_stream(stream)
}

/// One file with the Range support mediabunny's reader asks for; only a file, never a folder.
async fn send_file(h: &HeaderMap, abs: &Path, mime: &'static str) -> Response {
    use tokio::io::AsyncSeekExt;
    let size = match tokio::fs::metadata(abs).await {
        Ok(m) if m.is_file() => m.len(),
        _ => return status(StatusCode::NOT_FOUND),
    };
    let Ok(mut file) = tokio::fs::File::open(abs).await else {
        return status(StatusCode::NOT_FOUND);
    };
    let mut r = match range_of(h, size) {
        Some((start, end)) if start <= end && start < size => {
            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return status(StatusCode::NOT_FOUND);
            }
            let mut r = Response::new(body_of(file, end - start + 1));
            *r.status_mut() = StatusCode::PARTIAL_CONTENT;
            if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")) {
                r.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            if let Ok(v) = HeaderValue::from_str(&(end - start + 1).to_string()) {
                r.headers_mut().insert(header::CONTENT_LENGTH, v);
            }
            r
        }
        _ => {
            let mut r = Response::new(body_of(file, size));
            if let Ok(v) = HeaderValue::from_str(&size.to_string()) {
                r.headers_mut().insert(header::CONTENT_LENGTH, v);
            }
            r
        }
    };
    r.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    r.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    r
}

/// One chunk of the posted film, base64 text: it is appended to the part file, and the `last=1` chunk hands the film over (2505).
async fn receive(s: &Served, req: Request) -> Response {
    let query = req.uri().query().unwrap_or_default().to_string();
    let last = query.split('&').any(|p| p == "last=1");
    let mime = query
        .split('&')
        .find_map(|p| p.strip_prefix("type="))
        .map(percent_decode)
        .unwrap_or_default();
    let too_big = || format!("the render page posted a film over {} bytes", s.limit);
    let Ok(text) = axum::body::to_bytes(req.into_body(), CHUNK_TEXT_LIMIT).await else {
        settle(&s.settle, Err(too_big()));
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    };
    let Ok(raw) = BASE64.decode(&text) else {
        settle(
            &s.settle,
            Err("the render page posted a chunk that is not base64".into()),
        );
        return status(StatusCode::BAD_REQUEST);
    };
    let mut film = s.film.lock().await;
    if film.is_none() {
        let part = part_for(&s.out);
        if let Err(e) = tokio::fs::File::create(&part).await {
            settle(
                &s.settle,
                Err(format!("cannot write {}: {e}", part.display())),
            );
            return status(StatusCode::INTERNAL_SERVER_ERROR);
        }
        *film = Some(Rendered::new(part, mime.clone()));
    }
    let Some(rendered) = film.as_mut() else {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    };
    if !mime.is_empty() {
        rendered.mime = mime;
    }
    rendered.len += raw.len() as u64;
    if rendered.len > s.limit {
        *film = None;
        settle(&s.settle, Err(too_big()));
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let wrote = async {
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&rendered.part)
            .await?;
        file.write_all(&raw).await?;
        file.flush().await
    }
    .await;
    if let Err(e) = wrote {
        *film = None;
        settle(&s.settle, Err(format!("cannot write the film: {e}")));
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if last && let Some(mut done) = film.take() {
        done.blocked = s.blocked.lock().unwrap_or_else(|p| p.into_inner()).clone();
        settle(&s.settle, Ok(done));
    }
    status(StatusCode::NO_CONTENT)
}

/// Every request of the page: under the job's path, or a 404.
pub(crate) async fn serve(State(s): State<Arc<Served>>, req: Request) -> Response {
    let prefix = format!("/{}/", s.token);
    let path = req.uri().path().to_string();
    let Some(rest) = path.strip_prefix(&prefix) else {
        return status(StatusCode::NOT_FOUND);
    };
    let route = percent_decode(rest);
    if s.debug {
        let range = req
            .headers()
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        (s.log)(&format!("render: {} {route} {range}", req.method()));
    }
    let post = req.method() == Method::POST;
    let text =
        |mime: &'static str, body: String| ([(header::CONTENT_TYPE, mime)], body).into_response();
    match route.as_str() {
        "" => text("text/html", PAGE.to_string()),
        "page.js" => match tokio::fs::read_to_string(&s.script).await {
            Ok(js) => text("text/javascript", js),
            Err(_) => status(StatusCode::NOT_FOUND),
        },
        "job.json" => text("application/json", s.job.clone()),
        "mix.wav" => match &s.mix {
            Some(m) => send_file(req.headers(), m, "audio/wav").await,
            None => status(StatusCode::NOT_FOUND),
        },
        "progress" if post => status(StatusCode::NO_CONTENT),
        "result" if post => receive(&s, req).await,
        "error" if post => {
            let Ok(body) = axum::body::to_bytes(req.into_body(), 1 << 20).await else {
                return status(StatusCode::BAD_REQUEST);
            };
            let why = String::from_utf8_lossy(&body).into_owned();
            let why = if why.is_empty() {
                "the render page failed".into()
            } else {
                why
            };
            settle(&s.settle, Err(why));
            status(StatusCode::NO_CONTENT)
        }
        r if r.starts_with("media/") => {
            // The scene names its media `media/<encodeURIComponent(path)>`: looked up as the page asks it.
            match s
                .media
                .get(&format!("media/{}", &rest["media/".len()..]))
                .or_else(|| s.media.get(r))
            {
                Some(abs) => send_file(req.headers(), abs, "application/octet-stream").await,
                None => status(StatusCode::NOT_FOUND),
            }
        }
        _ => status(StatusCode::NOT_FOUND),
    }
}

/// `decodeURIComponent`, a malformed escape left as it is.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A random path segment: the page needs no account, and nothing else on the loopback can guess it.
pub(crate) fn token() -> String {
    let mut r = [0u8; 16];
    let _ = getrandom::fill(&mut r);
    r.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(v: &str) -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert(header::RANGE, HeaderValue::from_str(v).unwrap());
        m
    }

    #[test]
    fn reads_ranges_as_send_file_does() {
        assert_eq!(range_of(&h("bytes=0-3"), 10), Some((0, 3)));
        assert_eq!(range_of(&h("bytes=4-"), 10), Some((4, 9)));
        assert_eq!(range_of(&h("bytes=-4"), 10), Some((6, 9)));
        assert_eq!(range_of(&h("bytes=2-99"), 10), Some((2, 9)));
        assert_eq!(range_of(&h("items=0-1"), 10), None);
        assert_eq!(range_of(&HeaderMap::new(), 10), None);
    }

    #[test]
    fn decodes_as_decode_uri_component() {
        assert_eq!(
            percent_decode("media/clips%2Fsample.mp4"),
            "media/clips/sample.mp4"
        );
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
        assert_eq!(percent_decode("%C3%A9t%C3%A9"), "été");
    }

    use tokio::io::AsyncReadExt;

    /// A server over `served`, and the POST of `body` as base64 text to `/<token>/result`; the status line comes back.
    async fn post_result(served: Arc<Served>, body: &[u8]) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let app = axum::Router::new()
            .fallback(serve)
            .with_state(served.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let mut c = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let text = BASE64.encode(body);
        let head = format!(
            "POST /{}/result?type=video%2Fwebm&last=1 HTTP/1.1\r\nhost: x\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            served.token,
            text.len()
        );
        c.write_all(head.as_bytes()).await.unwrap();
        let _ = c.write_all(text.as_bytes()).await;
        let mut out = Vec::new();
        let _ = c.read_to_end(&mut out).await;
        server.abort();
        String::from_utf8_lossy(&out)
            .lines()
            .next()
            .unwrap_or("")
            .to_string()
    }

    fn served_in(
        dir: &Path,
        limit: u64,
    ) -> (Arc<Served>, oneshot::Receiver<Result<Rendered, String>>) {
        let (tx, rx) = oneshot::channel();
        let s = Served {
            token: "tok".into(),
            script: dir.join("page.js"),
            job: "{}".into(),
            mix: None,
            media: HashMap::new(),
            settle: Arc::new(Mutex::new(Some(tx))),
            debug: false,
            log: Arc::new(|_| {}),
            out: dir.join("film"),
            limit,
            blocked: Arc::default(),
            film: Default::default(),
        };
        (Arc::new(s), rx)
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "k7-picture-{name}-{}-{}",
            std::process::id(),
            token()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// 2505: the film lands in a dot-named part file beside `out`, whole, and is handed to the caller.
    #[tokio::test]
    async fn the_posted_film_is_written_to_a_part_file_beside_out() {
        let dir = scratch("film");
        let (s, rx) = served_in(&dir, 1 << 20);
        let line = post_result(s, &[7u8; 300_000]).await;
        assert!(line.contains("204"), "{line}");
        let r = rx.await.unwrap().unwrap();
        assert_eq!((r.len, r.mime.as_str()), (300_000, "video/webm"));
        assert!(
            r.part
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".film.part-")
        );
        assert_eq!(std::fs::metadata(&r.part).unwrap().len(), 300_000);
        drop(r);
        assert!(
            names(&dir).is_empty(),
            "a result dropped unclaimed leaves its part: {:?}",
            names(&dir)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 2505: a film past the bound fails the render and leaves no part file, announced or not.
    #[tokio::test]
    async fn a_film_past_the_bound_is_refused_and_leaves_nothing() {
        let dir = scratch("big");
        let (s, rx) = served_in(&dir, 1000);
        let line = post_result(s, &[1u8; 5000]).await;
        assert!(line.contains("413"), "{line}");
        assert!(rx.await.unwrap().unwrap_err().contains("over 1000 bytes"));
        assert!(names(&dir).is_empty(), "{:?}", names(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 2505: a second result of the same render is written and removed; the first is the answer.
    #[tokio::test]
    async fn a_second_result_leaves_no_part_file() {
        let dir = scratch("twice");
        let (s, rx) = served_in(&dir, 1 << 20);
        post_result(s.clone(), b"first").await;
        post_result(s, b"second").await;
        let r = rx.await.unwrap().unwrap();
        assert_eq!(std::fs::read(&r.part).unwrap(), b"first");
        assert_eq!(names(&dir).len(), 1, "{:?}", names(&dir));
        drop(r);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
