//! libraries.rs — the libraries in the front once every account's files are (always, in the front): the
//! store and its routes are native (`k7-libraries`), with no relay.
//! What a reader gets wrong: one service for the whole origin, never an account's, and it makes no account — docs/libraries.md.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use k7_hub::account::{Names, random_secret};
use k7_libraries::{Libraries, Request as LibRequest, Store};
use serde_json::json;

use super::{Services, body};
use crate::routes::files::json_answer;

/// The libraries' service: the routes and the secret that gates them.
pub struct Native {
    svc: Arc<Libraries>,
    secret: String,
}

impl Native {
    /// Whether the first `ensure` settled: the store's folders are there.
    pub fn settled(&self) -> bool {
        self.svc.settled()
    }
}

/// The libraries' service, made at the hub's launch; None when node's process keeps them.
pub async fn start(
    svc: &Services,
    libraries: &Names,
    name: &str,
) -> Option<Result<Arc<Native>, String>> {
    if !svc.libraries_in_front() {
        return None;
    }
    let made = async {
        let account = libraries.environment(name, &random_secret()?).await?;
        let env = account.env;
        let get = |k: &str| env.get(k).cloned().or_else(|| std::env::var(k).ok());
        let root = PathBuf::from(get("STUDIO_FILES_ROOT").ok_or("STUDIO_FILES_ROOT is not set")?);
        let store = Arc::new(Store::new(&root, get("STUDIO_LIBRARY_SEED").as_deref()));
        let secret = random_secret()?;
        let libs = Libraries::new(store, secret.clone());
        // The first `ensure` runs now, and the hub's readiness waits for it.
        let first = libs.clone();
        tokio::spawn(async move {
            first.first_ensure().await;
        });
        Ok::<_, String>(Arc::new(Native { svc: libs, secret }))
    };
    Some(made.await)
}

/// A request for the libraries under `target`: the client's identity dropped, the hub's admin stamp given, the body read whole.
pub async fn answer(native: &Native, target: &str, admin: bool, req: Request) -> Response {
    let method = req.method().as_str().to_string();
    // librariesRoutes.ts' `jsonBody` refuses past the file service's JSON limit, with this sentence.
    let bytes = match http_body_util::Limited::new(req.into_body(), body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return json_answer(
                413,
                &json!({ "error": format!("body over {} bytes", body::JSON_LIMIT) }),
            );
        }
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let r = native
        .svc
        .answer(&LibRequest {
            method: &method,
            path,
            query,
            secret: Some(&native.secret),
            admin,
            body: &bytes,
        })
        .await;
    match r.body {
        k7_libraries::Body::Json(v) => json_answer(r.status, &v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};

    fn native(tmp: &str) -> Arc<Native> {
        let dir = std::env::temp_dir().join(format!("k7-front-libs-{tmp}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("seed/k7-core")).unwrap();
        std::fs::write(dir.join("seed/k7-core/room.wav"), "RIFF-room").unwrap();
        let seed = dir.join("seed").display().to_string();
        let store = Arc::new(Store::new(&dir.join("data"), Some(&seed)));
        let secret = "front".to_string();
        let svc = Libraries::new(store, secret.clone());
        Arc::new(Native { svc, secret })
    }

    async fn ask(
        n: &Native,
        method: &str,
        target: &str,
        admin: bool,
        headers: &[(&str, &str)],
        body: Body,
    ) -> (u16, String) {
        let mut b = Request::builder().method(method).uri(target);
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        let r = answer(n, target, admin, b.body(body).unwrap()).await;
        let status = r.status().as_u16();
        (
            status,
            String::from_utf8_lossy(&to_bytes(r.into_body(), usize::MAX).await.unwrap())
                .into_owned(),
        )
    }

    #[tokio::test]
    async fn the_secret_gate_refuses_a_front_that_does_not_hold_the_services_secret() {
        let n = native("gate");
        n.svc.first_ensure().await;
        let wrong = Arc::new(Native {
            svc: n.svc.clone(),
            secret: "not-the-secret".into(),
        });
        let (status, text) = ask(
            &wrong,
            "GET",
            "/account/libraries",
            true,
            &[],
            Body::empty(),
        )
        .await;
        assert_eq!(
            (status, text.as_str()),
            (401, r#"{"error":"not from this service's file hub"}"#)
        );
        let (status, _) = ask(&n, "GET", "/account/libraries", true, &[], Body::empty()).await;
        assert_eq!(status, 200);
    }

    #[tokio::test]
    async fn every_refusal_keeps_its_sentence() {
        let n = native("refusals");
        n.svc.first_ensure().await;
        let big = Body::from(vec![b' '; body::JSON_LIMIT + 1]);
        let (status, text) = ask(&n, "POST", "/libraries/sync", true, &[], big).await;
        assert_eq!(
            (status, text.as_str()),
            (413, r#"{"error":"body over 8388608 bytes"}"#)
        );
        let (_, text) = ask(&n, "GET", "/nowhere", false, &[], Body::empty()).await;
        assert_eq!(text, r#"{"error":"no GET /nowhere"}"#);
    }
}
