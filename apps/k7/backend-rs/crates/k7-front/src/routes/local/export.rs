//! export.rs — where an export lands on a computer: the host's folder picker, a file written into the folder it
//! gave, and that folder shown. Choices: `apps/k7/backend-rs/docs/export.md`.
//! What a reader gets wrong: a folder is writable only if the host's dialog returned it during this run.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use hyper::StatusCode;
use serde_json::{Value, json};

use crate::AppState;
use crate::session::holds;
use crate::web::{error, read_json};

/// What the host that links the front gives it: the system's folder dialog and its file manager.
pub trait Host: Send + Sync {
    /// The folder the person chose, starting at `start`; None when they cancelled. Blocks until they answer.
    fn pick_folder(&self, start: Option<&Path>) -> Option<PathBuf>;
    /// `path` shown in the file manager.
    fn reveal(&self, path: &Path);
}

static HOST: RwLock<Option<Arc<dyn Host>>> = RwLock::new(None);
static PICKED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// Installed once by a host that has a dialog, before the front starts; a front without one answers 501.
pub fn set_host(host: Arc<dyn Host>) {
    *HOST.write().unwrap_or_else(|p| p.into_inner()) = Some(host);
}

fn host() -> Option<Arc<dyn Host>> {
    HOST.read().unwrap_or_else(|p| p.into_inner()).clone()
}

fn remember(folder: &Path) {
    PICKED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_with(HashSet::new)
        .insert(folder.to_path_buf());
}

fn picked(folder: &Path) -> bool {
    PICKED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .is_some_and(|s| s.contains(folder))
}

fn last_file(state: &AppState) -> PathBuf {
    state.settings.prefs_dir.join("export-folder.txt")
}

fn unsigned(state: &AppState, req: &Request) -> Option<Response> {
    (!holds(req.headers(), &state.settings))
        .then(|| error(StatusCode::UNAUTHORIZED, "unauthenticated"))
}

fn no_host() -> Response {
    error(
        StatusCode::NOT_IMPLEMENTED,
        "this device has no folder picker",
    )
}

/// A file name that stays in its folder: no separator, no dot lead, nothing a file system refuses.
pub fn safe_name(name: &str) -> Option<&str> {
    let bad = name.is_empty()
        || name.len() > 200
        || name.starts_with('.')
        || name.ends_with([' ', '.'])
        || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || name.chars().any(char::is_control);
    (!bad).then_some(name)
}

/// `name`, or `stem (2).ext`, `stem (3).ext`… the first that `taken` does not claim.
pub fn unique(dir: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(name);
    if !taken(&first) {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !taken(p))
        .expect("an unbounded range")
}

/// `POST /api/local/export/folder`: the dialog, opened where the last export went; `{path, name}` or `{cancelled}`.
pub async fn folder(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if let Some(r) = unsigned(&state, &req) {
        return r;
    }
    let Some(host) = host() else {
        return no_host();
    };
    let file = last_file(&state);
    let chosen = tokio::task::spawn_blocking(move || {
        let start = std::fs::read_to_string(&file)
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| p.is_dir());
        let chosen = host.pick_folder(start.as_deref());
        if let Some(c) = &chosen {
            let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
            let _ = std::fs::write(&file, c.to_string_lossy().as_bytes());
        }
        chosen
    })
    .await
    .ok()
    .flatten();
    match chosen {
        Some(path) => {
            remember(&path);
            let name = path.file_name().map_or_else(
                || path.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            axum::Json(json!({ "path": path.to_string_lossy(), "name": name })).into_response()
        }
        None => axum::Json(json!({ "cancelled": true })).into_response(),
    }
}

/// `folder` and `name` of the query.
fn target(req: &Request) -> (String, String) {
    let pairs: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes()).collect();
    let get = |k: &str| pairs.get(k).map(|v| v.to_string()).unwrap_or_default();
    (get("folder"), get("name"))
}

/// `PUT /api/local/export/file?folder=&name=`: the body written into a folder the dialog gave, beside what is there.
pub async fn file(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if let Some(r) = unsigned(&state, &req) {
        return r;
    }
    let (folder, name) = target(&req);
    let folder = PathBuf::from(folder);
    let Some(name) = safe_name(&name) else {
        return error(StatusCode::BAD_REQUEST, "that is not a file name");
    };
    if !picked(&folder) {
        return error(
            StatusCode::FORBIDDEN,
            "that folder was not chosen in this session",
        );
    }
    match write_into(&folder, name, req.into_body()).await {
        Ok(path) => axum::Json(json!({
            "path": path.to_string_lossy(),
            "name": path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
        }))
        .into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// The body streamed to a dot-named part file, then renamed to its free name: a half file never bears the name.
pub async fn write_into(
    folder: &Path,
    name: &str,
    mut body: axum::body::Body,
) -> Result<PathBuf, String> {
    let part = folder.join(format!(".{name}.part"));
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let copied: Result<(), String> = async {
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|e| e.to_string())?;
            if let Some(data) = frame.data_ref() {
                out.write_all(data).map_err(|e| e.to_string())?;
            }
        }
        out.flush().map_err(|e| e.to_string())
    }
    .await;
    drop(out);
    let target = unique(folder, name, Path::exists);
    let done = copied.and_then(|()| std::fs::rename(&part, &target).map_err(|e| e.to_string()));
    match done {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// `POST /api/local/export/reveal {path}`: a file of a chosen folder, or the folder, shown in the file manager.
pub async fn reveal(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if let Some(r) = unsigned(&state, &req) {
        return r;
    }
    let Some(host) = host() else {
        return no_host();
    };
    let body = match read_json(req).await {
        Ok((_, b)) => b,
        Err(r) => return r,
    };
    let path = body.get("path").and_then(Value::as_str).map(PathBuf::from);
    match path {
        Some(p) if picked(&p) || p.parent().is_some_and(picked) => {
            host.reveal(&p);
            axum::Json(json!({ "ok": true })).into_response()
        }
        _ => error(
            StatusCode::FORBIDDEN,
            "that path was not chosen in this session",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_stays_in_its_folder() {
        for bad in ["", ".hidden", "a/b", "..", "a\\b", "x.", "tab\tname", "a:b"] {
            assert_eq!(safe_name(bad), None, "{bad:?}");
        }
        assert_eq!(safe_name("song (2).mp3"), Some("song (2).mp3"));
    }

    #[test]
    fn a_taken_name_gets_the_next_number() {
        let dir = Path::new("/x");
        let taken = |p: &Path| p.ends_with("a.wav") || p.ends_with("a (2).wav");
        assert_eq!(unique(dir, "a.wav", taken), dir.join("a (3).wav"));
        assert_eq!(unique(dir, "b.wav", taken), dir.join("b.wav"));
    }

    #[tokio::test]
    async fn the_body_lands_whole_and_never_over_a_file() {
        let dir = std::env::temp_dir().join(format!("k7-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = write_into(&dir, "cut.otio", axum::body::Body::from("one")).await;
        let second = write_into(&dir, "cut.otio", axum::body::Body::from("two")).await;
        assert_eq!(std::fs::read_to_string(first.unwrap()).unwrap(), "one");
        let second = second.unwrap();
        assert_eq!(second, dir.join("cut (2).otio"));
        assert_eq!(std::fs::read_to_string(second).unwrap(), "two");
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(left.len(), 2, "no part file stays");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
