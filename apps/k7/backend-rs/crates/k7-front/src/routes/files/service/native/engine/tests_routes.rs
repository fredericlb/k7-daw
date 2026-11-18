//! The front's own engine routes (2226, 2227), reached as the router reaches them: `live_hook` with a built `Who`, over the
//! real account, link and fake engine of `tests_real`. What a reader gets wrong: the account here is the hub's, built by hand.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};

use super::fake::{FakeJobs, JobEnd, until};
use super::routes::live_hook;
use super::tests_real::{Account, SESSION, account, account_with_lane};
use super::tests_review::silent_wav;
use crate::routes::files::service::{Held, Who};

/// The person's front: the hub's account, the held account and link.
struct Front {
    a: Account,
    held: Held,
    log: k7_hub::Log,
}

async fn front(tag: &str) -> Front {
    front_of(account(tag).await).await
}

async fn front_of(a: Account) -> Front {
    let home = a.dir.join("home");
    let env =
        std::collections::BTreeMap::from([("STUDIO_HOME".to_string(), home.display().to_string())]);
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

impl Front {
    /// One request through `live_hook`: the status and body, or `None` when the route is handed back.
    async fn ask(&self, method: &str, path: &str, body: Option<Value>) -> Option<(u16, Value)> {
        let who = Who {
            held: &self.held,
            user: "a@b.c",
            admin: false,
            device: false,
            log: &self.log,
            code_root: None,
        };
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-studio-workspace", "songs")
            .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
            .unwrap();
        let r = live_hook(&who, path, req).await.ok()?;
        let status = r.status().as_u16();
        let bytes = r.into_body().collect().await.unwrap().to_bytes();
        Some((
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        ))
    }

    async fn started(&self) {
        self.a.open("p.k7");
        self.a.start().await;
        until(30, "the document", || {
            self.a.kinds().iter().any(|k| k == "project")
        })
        .await;
    }
}

// The browser's document: a body without one is refused as the schema words it, one with it reaches the engine (2226).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_clients_document_route_checks_its_body_and_sends_the_document() {
    let f = front("route-project").await;
    f.started().await;
    let (status, body) = f
        .ask("POST", "/engine/project", Some(json!({})))
        .await
        .unwrap();
    assert_eq!(status, 400, "{body}");
    let mut doc = super::fake::minimal_doc(-5.0);
    doc["name"] = json!("browser");
    let (status, body) = f
        .ask("POST", "/engine/project", Some(json!({ "doc": doc })))
        .await
        .unwrap();
    assert_eq!((status, body), (200, json!({ "ok": true })));
    until(30, "the browser's document", || {
        f.a.fake
            .of(SESSION)
            .iter()
            .any(|x| x["type"] == "project" && x["doc"]["name"] == "browser")
    })
    .await;
}

// The served project is told by address, and none after the unload route let it go (2227).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_served_and_unload_routes_agree_on_what_the_engine_holds() {
    let f = front("route-served").await;
    f.started().await;
    let (status, body) = f.ask("GET", "/engine/served", None).await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({ "project": { "workspace": "songs", "path": "p.k7" } })
    );
    let (status, body) = f
        .ask("POST", "/engine/unload", Some(json!({})))
        .await
        .unwrap();
    assert_eq!((status, body), (200, json!({ "ok": true })));
    let (_, body) = f.ask("GET", "/engine/served", None).await.unwrap();
    assert_eq!(body, json!({ "project": null }));
    until(10, "the drop", || {
        f.a.kinds().iter().any(|k| k == "drop-session")
    })
    .await;
}

// The retry reloads the active project: a second document reaches the engine (2227).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_retry_route_sends_the_active_project_again() {
    let f = front("route-retry").await;
    f.started().await;
    let sent = || f.a.kinds().iter().filter(|k| *k == "project").count();
    let before = sent();
    let (status, body) = f
        .ask("POST", "/engine/sync/retry", Some(json!({})))
        .await
        .unwrap();
    assert_eq!((status, body), (200, json!({ "ok": true })));
    until(30, "the reloaded document", || sent() > before).await;
}

// The poll answers the workspace's info with the link's keys, and a hidden server hands every route back.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_poll_carries_the_links_keys_and_a_hidden_server_hands_the_routes_back() {
    let f = front("route-poll").await;
    f.started().await;
    let (status, body) = f.ask("GET", "/workspace", None).await.unwrap();
    assert_eq!(status, 200, "{body}");
    for key in [
        "engineLoad",
        "engineSync",
        "engineScan",
        "lastTake",
        "processes",
    ] {
        assert!(body.get(key).is_some(), "{key} missing: {body}");
    }
    assert!(f.ask("GET", "/engine/else", None).await.is_none());
}

// The retry's answer comes after the copy began, as the TypeScript's synchronous retry did: the next poll shows it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_retry_route_answers_once_the_copy_is_under_way() {
    let f = front_of(account_with_lane("route-retry-copy").await).await;
    let jobs = FakeJobs::start().await;
    jobs.ends(JobEnd::Synced {
        failed: 1,
        failures: vec!["a.wav: gone".into()],
    });
    let p = std::path::Path::new(&f.a.ws).join("p.k7");
    silent_wav(&p.join("clips/a.wav"));
    std::fs::write(
        p.join("clips/a1.k7x"),
        "source(\"clips/a.wav\")\nfrom(\"1:1\")\nto(\"2:1\")\n",
    )
    .unwrap();
    std::fs::write(
        p.join("nodes/gtr.k7x"),
        "extends(\"@meta/track\")\nevents([{ ref: \"a1\", start: \"1:1\" }])\n",
    )
    .unwrap();
    f.a.bind(&json!({ "url": f.a.fake.url, "jobsUrl": jobs.url, "engineId": "e1" }));
    f.a.open("p.k7");
    f.a.start().await;
    let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let (_, body) = f.ask("GET", "/workspace", None).await.unwrap();
        if body["engineSync"]["failed"].as_u64().unwrap_or(0) > 0 {
            break;
        }
        assert!(std::time::Instant::now() < end, "the failure never showed");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    jobs.takes(std::time::Duration::from_secs(5));
    jobs.ends(JobEnd::Synced {
        failed: 0,
        failures: vec![],
    });
    f.ask("POST", "/engine/sync/retry", Some(json!({})))
        .await
        .unwrap();
    let (_, body) = f.ask("GET", "/workspace", None).await.unwrap();
    assert!(
        !body["engineSync"].is_null(),
        "the poll after the retry: {body}"
    );
}
