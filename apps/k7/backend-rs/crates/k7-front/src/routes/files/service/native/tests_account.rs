//! The account the native tests make: the environment of a scratch home, a route's call, and its answer as text.
//! What a reader gets wrong: each `start` is a fresh account with its own song sessions; share the `Arc` to share them.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::Value;

use crate::routes::files::service::Acct;
use crate::routes::files::service::routes::Call;

/// The song sessions' heap a test account is sized for: the server's.
pub(super) const HEAP: usize = 384 << 20;

/// An account over a scratch home under `dir`.
pub(super) fn start(dir: &std::path::Path) -> Arc<Acct> {
    start_with(dir, &[])
}

/// An account whose environment also holds `extra`.
pub(super) fn start_with(dir: &std::path::Path, extra: &[(&str, &str)]) -> Arc<Acct> {
    Acct::make(env_with(dir, extra), HEAP)
}

/// The environment of such an account, for a test that builds its own pieces from it.
pub(super) fn env_with(dir: &std::path::Path, extra: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for (k, v) in extra {
        env.insert(k.to_string(), v.to_string());
    }
    // No fsevents stream per opened project: on a busy Mac each blocks for minutes (docs/service.md).
    env.insert("K7_NO_FS_WATCH".to_string(), "1".to_string());
    env.insert(
        "STUDIO_HOME".to_string(),
        super::tests::slashed(&dir.join("home")),
    );
    env.insert(
        "STUDIO_LIBRARIES_HOME".to_string(),
        super::tests::slashed(&dir.join("libraries")),
    );
    env
}

/// A route's call on the workspace `songs` under `dir`.
pub(super) fn call(
    dir: &std::path::Path,
    query: &str,
    body: Value,
    native: Option<super::Ctx>,
) -> Call {
    Call {
        runtime: None,
        workspace: "songs".into(),
        root: dir.join("home/workspace/songs").display().to_string(),
        query: query.into(),
        body,
        log: Arc::new(|_: &str| {}),
        native,
        media: None,
        engine: None,
    }
}

pub(super) async fn text(r: Response) -> (u16, String) {
    let status = r.status().as_u16();
    (
        status,
        String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap(),
    )
}
