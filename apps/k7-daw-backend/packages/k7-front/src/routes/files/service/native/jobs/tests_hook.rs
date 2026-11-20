//! tests_hook.rs — `hook` and `read_body` as the router reaches them: a built `Who`, the real account and link of `tests_real`.
//! What a reader gets wrong: a declined request must come back with the bytes the route already read, so the first test compares them.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use http_body_util::BodyExt;
use serde_json::Value;

use super::routes::hook;
use crate::routes::files::service::body::JSON_LIMIT;
use crate::routes::files::service::native::engine::tests_real::{Account, account};
use crate::routes::files::service::{Held, Who};

struct Front {
    a: Account,
    held: Held,
    log: k7_hub::Log,
}

async fn front(tag: &str) -> Front {
    let a = account(tag).await;
    let home = a.dir.join("home");
    let env = std::collections::BTreeMap::from([
        ("STUDIO_HOME".to_string(), home.display().to_string()),
        (
            "STUDIO_LIBRARIES_HOME".to_string(),
            a.dir.join("libraries").display().to_string(),
        ),
    ]);
    let engine = a.engine.clone().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    engine.hold_jobs(&env, &root);
    let account = k7_hub::account::Account {
        user: "a@b.c".into(),
        slug: "a-b-c".into(),
        home,
        secret: "s".into(),
        env,
    };
    let held = Held::of_rig(
        account,
        a.acct.clone(),
        std::env::temp_dir(),
        a.engine.clone(),
        false,
    );
    let log: k7_hub::Log = Arc::new(|_: &str| {});
    Front { a, held, log }
}

/// What a request came to: the route's answer, or the bytes of the request handed back.
type Outcome = Result<(u16, Value), Vec<u8>>;

struct Ask<'a> {
    user: &'a str,
    workspace: Option<&'a str>,
}

impl Default for Ask<'_> {
    fn default() -> Self {
        Ask {
            user: "a@b.c",
            workspace: Some("songs"),
        }
    }
}

impl Front {
    async fn ask(&self, o: Ask<'_>, method: &str, path: &str, body: Vec<u8>) -> Outcome {
        let who = Who {
            held: &self.held,
            user: o.user,
            device: false,
            log: &self.log,
            code_root: None,
        };
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(w) = o.workspace {
            b = b.header("x-studio-workspace", w);
        }
        let req = b.body(Body::from(body)).unwrap();
        let bare = path.split('?').next().unwrap_or("");
        let engine = self.held.engine.clone().unwrap();
        match hook(&who, &engine, bare, req).await {
            Ok(r) => {
                let status = r.status().as_u16();
                let bytes = r.into_body().collect().await.unwrap().to_bytes();
                Ok((
                    status,
                    serde_json::from_slice(&bytes).unwrap_or(Value::Null),
                ))
            }
            Err(req) => Err(req.into_body().collect().await.unwrap().to_bytes().to_vec()),
        }
    }
}

fn error_of(o: &Outcome) -> &str {
    o.as_ref()
        .ok()
        .and_then(|(_, v)| v["error"].as_str())
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_request_the_piece_does_not_answer_goes_back_with_its_bytes() {
    let f = front("hook-back").await;
    let body = br#"{"kind":"source","source":"clips/a.wav"}"#.to_vec();
    // A decline after the body was read: the open project is a library, which the front declines.
    let marks = f.a.dir.join("home/workspace/.active-projects");
    std::fs::create_dir_all(&marks).unwrap();
    std::fs::write(marks.join("songs"), "_libraries").unwrap();
    assert_eq!(
        f.ask(Ask::default(), "POST", "/preview", body.clone())
            .await,
        Err(body)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_body_past_the_limit_is_413_and_bad_json_is_400() {
    let f = front("hook-body").await;
    let big = vec![b' '; JSON_LIMIT + 1];
    let r = f
        .ask(Ask::default(), "POST", "/auto-freeze/thaw", big)
        .await;
    assert_eq!(r.as_ref().map(|(s, _)| *s), Ok(413), "{r:?}");
    let r = f
        .ask(Ask::default(), "POST", "/auto-freeze/thaw", b"{".to_vec())
        .await;
    assert_eq!(r.as_ref().map(|(s, _)| *s), Ok(400), "{r:?}");
    assert!(error_of(&r).contains("JSON"), "{r:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_workspace_is_judged_before_the_fields() {
    let f = front("hook-ws").await;
    // `{}` lacks every field, so a 400 about the fields would be the answer if the workspace were not asked first.
    let body = b"{}".to_vec();
    let none = Ask {
        workspace: None,
        ..Ask::default()
    };
    let r = f.ask(none, "POST", "/auto-freeze/thaw", body.clone()).await;
    assert_eq!(
        (r.as_ref().map(|(s, _)| *s), error_of(&r)),
        (Ok(400), "missing workspace header")
    );
    let ghost = Ask {
        workspace: Some("ghost"),
        ..Ask::default()
    };
    let r = f
        .ask(ghost, "POST", "/auto-freeze/thaw", body.clone())
        .await;
    assert_eq!(
        (r.as_ref().map(|(s, _)| *s), error_of(&r)),
        (Ok(404), "unknown workspace: ghost")
    );
    // The first person to ask owns the workspaces; another gets 403.
    let owner = f
        .ask(Ask::default(), "POST", "/auto-freeze/thaw", body.clone())
        .await;
    assert_eq!(owner.as_ref().map(|(s, _)| *s), Ok(400), "{owner:?}");
    let other = Ask {
        user: "x@y.z",
        ..Ask::default()
    };
    let r = f.ask(other, "POST", "/auto-freeze/thaw", body).await;
    assert_eq!(r.as_ref().map(|(s, _)| *s), Ok(403), "{r:?}");
}
