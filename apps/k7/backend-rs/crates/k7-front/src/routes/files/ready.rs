//! ready.rs — `/api/ready` (`docs/files.md`, "Ready"): every volume takes a write and the hub reports its
//! runtimes; answered to this machine only, by its peer and by the Host it names (`loopbackOnly`).
//! What a reader gets wrong: the hub's figures keep node's shape and never make a host unready.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use serde_json::{Map, Value, json};

use super::Routes;
use crate::AppState;

/// A volume that has not taken a one-byte write in this long counts as unwritable.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Write and remove a file in `dir`: the error's message, or None when the volume took it.
async fn probe(dir: String) -> Option<String> {
    let file = std::path::Path::new(&dir).join(format!(".ready-{}", std::process::id()));
    let write = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&file, "1")?;
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    });
    match tokio::time::timeout(PROBE_TIMEOUT, write).await {
        Ok(Ok(Ok(()))) => None,
        Ok(Ok(Err(e))) => Some(e.to_string()),
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some(format!("no write within {} ms", PROBE_TIMEOUT.as_millis())),
    }
}

async fn ready(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    if !crate::web::is_loopback_request(peer, req.headers()) {
        let mut r = (
            StatusCode::FORBIDDEN,
            "This server answers its own machine only.",
        )
            .into_response();
        r.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        return r;
    }
    let dirs = state.settings.ready_volumes.clone();
    let probes: Vec<_> = dirs
        .iter()
        .cloned()
        .map(|d| tokio::spawn(probe(d)))
        .collect();
    let mut failed = Vec::with_capacity(probes.len());
    for p in probes {
        failed.push(p.await.unwrap_or_else(|e| Some(e.to_string())));
    }
    let mut volumes = Map::new();
    let mut problems = Vec::new();
    for (d, f) in dirs.iter().zip(&failed) {
        volumes.insert(d.clone(), json!(f.as_deref().unwrap_or("ok")));
        if let Some(e) = f {
            problems.push(json!(format!("{d}: {e}")));
        }
    }
    let files = super::files_of(&state).hub.status();
    let status = if problems.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let ok = problems.is_empty();
    (status, Json(json!({ "ok": ok, "problems": Value::Array(problems), "volumes": volumes, "files": files }))).into_response()
}

/// Its routes behind the chain.
pub fn routes() -> Routes {
    axum::Router::new().route("/api/ready", get(ready).fallback(crate::proxied))
}
