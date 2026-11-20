//! stream.rs — the routes whose bytes are no JSON, answered as `fileRoutes.ts`, `entryRoutes.ts` and `server.ts`
//! answered them: an upload streamed to its temp beside the target, and `/media` served with Range.
//! What a reader gets wrong: the native pieces decide every path; this file only moves the bytes.

use std::path::{Path, PathBuf};

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

use super::native;
use super::routes::Stream;
use super::{Who, body, workspace};
use crate::routes::files::json_answer;

/// `MAX_UPLOAD_BYTES` (`apps/k7-daw-frontend/packages/web-protocol/workspaceTypes.ts`).
const MAX_UPLOAD: u64 = 512 * 1024 * 1024;

fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

/// `uploadTooLarge(MAX_UPLOAD_BYTES)`'s sentence.
fn too_large() -> Response {
    error(
        413,
        &format!(
            "larger than {} MB, the most one upload takes",
            MAX_UPLOAD / (1024 * 1024)
        ),
    )
}

/// An I/O failure as an entry route words it (1291): the server's own sentence is logged, never answered.
pub(super) fn io_words(code: Option<&str>) -> (u16, String) {
    match code {
        Some("ENOENT") => (404, "no such file or folder".to_string()),
        c => (
            400,
            format!("the file could not be changed ({})", c.unwrap_or("error")),
        ),
    }
}

/// The request's route, answered.
pub async fn answer(kind: Stream, who: &Who<'_>, query: &str, req: Request) -> Response {
    match kind {
        Stream::Media => media(who, query, req).await,
        Stream::Upload => upload(who, query, req).await,
        Stream::Envelope => super::record::envelope(who, query, req).await,
    }
}

/// `GET /media`: identity in the query, the file `mediaPath` resolves, sent with Range as `res.sendFile` sends it.
async fn media(who: &Who<'_>, query: &str, req: Request) -> Response {
    let (user, ws, path) = (
        body::query(query, "user"),
        body::query(query, "workspace"),
        body::query(query, "path"),
    );
    if user.is_empty() || ws.is_empty() || path.is_empty() {
        return error(400, "user, workspace and path required");
    }
    let abs = match workspace_file(who, &ws, query).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let mime = k7_mediatek::media::mime_for(&abs.to_string_lossy());
    crate::routes::http_file::stream_file(req.headers(), &abs, mime, Some("public, max-age=0"))
        .await
}

/// A workspace file a request names (`path` in its query), as `/media` reads it: the native read, then a file there.
pub(super) async fn workspace_file(
    who: &Who<'_>,
    ws: &str,
    query: &str,
) -> Result<PathBuf, Box<Response>> {
    let workspaces = who
        .held
        .account
        .home
        .join("workspace")
        .display()
        .to_string();
    let nreq = native::Req {
        env: &who.held.account.env,
        slug: &who.held.account.slug,
        workspace: ws,
        root: &workspaces,
        query,
        body: &Value::Null,
        log: who.log,
        docs: None,
        thawed: None,
    };
    let Some(c) = native::ctx_for(Some(native::Tag::Media), &nreq) else {
        return Err(Box::new(native::refuse::refused()));
    };
    // The native read of the path (1056); a decline refuses, as `dispatch` does (4-7).
    let abs = match native::reads::media(&c).await {
        Some(Err(refusal)) => return Err(refusal),
        Some(Ok(p)) => p,
        None => return Err(Box::new(native::refuse::refused())),
    };
    match std::fs::metadata(&abs) {
        Ok(m) if m.is_file() => Ok(abs),
        Ok(_) => Err(Box::new(error(
            404,
            &format!(
                "EISDIR: illegal operation on a directory, read '{}'",
                abs.display()
            ),
        ))),
        Err(_) => Err(Box::new(error(
            404,
            &format!(
                "ENOENT: no such file or directory, stat '{}'",
                abs.display()
            ),
        ))),
    }
}

/// The body into `file`, at most `max` bytes; Err(true) past it, Err(false) on a cut or a write that failed.
async fn spool(req: Request, file: &Path, max: u64) -> Result<u64, (bool, String)> {
    let mut out = tokio::fs::File::create(file)
        .await
        .map_err(|e| (false, e.to_string()))?;
    let mut body = req.into_body();
    let mut size = 0u64;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|e| (false, e.to_string()))?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        size += data.len() as u64;
        if size > max {
            return Err((true, String::new()));
        }
        out.write_all(&data)
            .await
            .map_err(|e| (false, e.to_string()))?;
    }
    out.flush().await.map_err(|e| (false, e.to_string()))?;
    Ok(size)
}

/// `POST /upload`: the target the caller picks, the bytes streamed to its temp in the capture's window, then renamed.
async fn upload(who: &Who<'_>, query: &str, req: Request) -> Response {
    let (wsname, root) = match workspace(who, req.headers()) {
        Ok(w) => w,
        Err(r) => return *r,
    };
    if let Some(refused) = native::git::merge_refusal(&root.display().to_string()) {
        return refused;
    }
    let (dir, name) = (body::query(query, "dir"), body::query(query, "name"));
    let dir = if dir.is_empty() { ".".to_string() } else { dir };
    if name.is_empty() {
        return error(400, "name required");
    }
    let declared = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > MAX_UPLOAD) {
        return too_large();
    }
    // Express's JSON parser would have spent the stream: node refuses that body, so does this.
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if content_type
        .split(';')
        .next()
        .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
    {
        return error(415, "send the bytes as application/octet-stream");
    }
    let ws = root.display().to_string();
    let nreq = native::Req {
        env: &who.held.account.env,
        slug: &who.held.account.slug,
        workspace: &wsname,
        root: &ws,
        query,
        body: &Value::Null,
        log: who.log,
        docs: None,
        thawed: None,
    };
    // The piece is the only answer an upload has: a decline refuses (4-7).
    let Some(c) = native::ctx_for(Some(native::Tag::Upload), &nreq) else {
        return native::refuse::refused();
    };
    let (abs, tmp) = match native::entries::upload_begin(&c, &dir, &name).await {
        native::entries::Begun::Ready { abs, tmp } => (abs, PathBuf::from(tmp)),
        native::entries::Begun::Refused(r) => return r,
        native::entries::Begun::Declined => return native::refuse::refused(),
    };
    // The bytes and the rename are one batch of the account's capture, as node's upload was one of node's.
    // A client that hangs up mid-upload drops this future: the window and the temp still go.
    let window = super::Window::open(&ws, "").await;
    let _temp = Temp(tmp.clone());
    let r = match spool(req, &tmp, MAX_UPLOAD).await {
        Ok(_) => native::entries::upload_end(&c, &abs, &tmp.display().to_string()).await,
        Err((true, _)) => too_large(),
        Err((false, m)) => error(400, &m),
    };
    window.close().await;
    r
}

/// A temp file removed when dropped, the request finished or not.
struct Temp(PathBuf);

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::io_words;

    #[test]
    fn an_io_failure_is_worded_without_the_servers_paths() {
        assert_eq!(
            io_words(Some("ENOENT")),
            (404, "no such file or folder".to_string())
        );
        assert_eq!(
            io_words(Some("EACCES")),
            (400, "the file could not be changed (EACCES)".to_string())
        );
        assert_eq!(
            io_words(None),
            (400, "the file could not be changed (error)".to_string())
        );
    }
}
