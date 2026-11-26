//! download — a file fetched to disk over the HTTP client: redirects followed (a bounded few, never from
//! https down to http), and a `.part` left by a cut transfer RESUMED with a `Range`. The server's answer
//! decides what the file is: 206 from the right byte appends, 200 replaces, a 416 naming the file's own
//! length means it is whole, anything else drops the prefix and starts again. The bytes are not judged here:
//! the caller's hash is (`docs/instruments.md`). A transfer cut short keeps what it brought.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CONTENT_RANGE, LOCATION, RANGE};
use tokio::io::AsyncWriteExt;
use url::Url;

/// More hops than any CDN needs, fewer than a loop.
const MAX_REDIRECTS: usize = 10;

/// Fetch `url` into `part`, resuming it if it exists; `progress` gets the bytes held so far, resumed ones included.
/// `timeout` bounds the connect, the wait for an answer and the wait between two chunks, each.
pub async fn fetch(
    tls: Arc<rustls::ClientConfig>,
    url: &str,
    part: &Path,
    timeout: Duration,
    mut progress: impl FnMut(u64) + Send,
) -> Result<(), String> {
    let mut have = tokio::fs::metadata(part).await.map_or(0, |m| m.len());
    let mut restarted = false;
    loop {
        let (status, head) = get(&tls, url, (have > 0).then_some(have), timeout).await?;
        let drop_prefix = match status {
            206 if content_range_start(&head) == Some(have) => false,
            206 => true,
            200 => {
                have = 0;
                false
            }
            // Already whole: the hash decides.
            416 if have > 0 && content_range_total(&head) == Some(have) => return Ok(()),
            416 if have > 0 => true,
            s => return Err(format!("{url} answered {s}")),
        };
        if drop_prefix {
            // What is there is no prefix to trust: once more from zero, never in a loop.
            if restarted {
                return Err(format!("{url} would not resume or restart"));
            }
            restarted = true;
            have = 0;
            let _ = tokio::fs::remove_file(part).await;
            continue;
        }
        return stream(head.into_body(), part, have, timeout, &mut progress).await;
    }
}

/// One GET following redirects: the final status and answer.
async fn get(
    tls: &Arc<rustls::ClientConfig>,
    url: &str,
    from: Option<u64>,
    timeout: Duration,
) -> Result<(u16, hyper::Response<Incoming>), String> {
    let mut at = Url::parse(url).map_err(|e| format!("{url} is not a url: {e}"))?;
    for _ in 0..=MAX_REDIRECTS {
        if !matches!(at.scheme(), "http" | "https") {
            return Err(format!("{at} is not an http address"));
        }
        let mut req = hyper::Request::builder()
            .method("GET")
            .uri(at.as_str())
            .header("user-agent", "k7-supervisor");
        if let Some(n) = from {
            req = req.header(RANGE, format!("bytes={n}-"));
        }
        let req = req
            .body(Full::new(Bytes::new()))
            .map_err(|e| e.to_string())?;
        let client = crate::http::client(tls.clone(), timeout);
        let res = match tokio::time::timeout(timeout, client.request(req)).await {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => return Err(format!("{at}: {}", describe(&e))),
            Err(_) => return Err(format!("{at}: timed out")),
        };
        let status = res.status().as_u16();
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            return Ok((status, res));
        }
        let next = res
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|l| at.join(l).ok())
            .ok_or_else(|| format!("{at} redirected nowhere"))?;
        if at.scheme() == "https" && next.scheme() == "http" {
            return Err(format!("{at} redirected from https to {next}, refused"));
        }
        at = next;
    }
    Err(format!("{url} redirected more than {MAX_REDIRECTS} times"))
}

/// The answer's body onto the file, appended past `have` or the file replaced when `have` is zero.
async fn stream(
    mut body: Incoming,
    part: &Path,
    have: u64,
    timeout: Duration,
    progress: &mut (impl FnMut(u64) + Send),
) -> Result<(), String> {
    let mut file = if have > 0 {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(part)
            .await
    } else {
        tokio::fs::File::create(part).await
    }
    .map_err(|e| format!("cannot write {}: {e}", part.display()))?;
    let mut received = have;
    progress(received);
    loop {
        let frame = match tokio::time::timeout(timeout, body.frame()).await {
            Err(_) => return Err("the transfer stalled".into()),
            Ok(None) => break,
            Ok(Some(Err(e))) => return Err(format!("the transfer was cut: {e}")),
            Ok(Some(Ok(f))) => f,
        };
        let Ok(data) = frame.into_data() else {
            continue;
        };
        file.write_all(&data)
            .await
            .map_err(|e| format!("cannot write {}: {e}", part.display()))?;
        received += data.len() as u64;
        progress(received);
    }
    file.flush().await.map_err(|e| e.to_string())
}

fn content_range(head: &hyper::Response<Incoming>) -> Option<&str> {
    head.headers().get(CONTENT_RANGE)?.to_str().ok()
}

/// `bytes 100-199/200` starts at 100.
fn content_range_start(head: &hyper::Response<Incoming>) -> Option<u64> {
    let (first, _) = content_range(head)?
        .strip_prefix("bytes ")?
        .split_once('-')?;
    first.trim().parse().ok()
}

/// `bytes */200` names a size of 200.
fn content_range_total(head: &hyper::Response<Incoming>) -> Option<u64> {
    let (_, total) = content_range(head)?.split_once('/')?;
    total.trim().parse().ok()
}

fn describe(e: &hyper_util::client::legacy::Error) -> String {
    let mut text = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        text += &format!(": {s}");
        source = s.source();
    }
    text
}
