//! `GET /api/files/health` with a files ticket, by header or by query, answered by a device's front on a host with no node:
//! `K7_NODE` names no program and no service entry exists, so a start of one would fail where it was tried.
//! Its own binary: one front per test binary, as the file hub's other tests have.

mod files_hub_support;
mod http_support;

use files_hub_support::send;
use http_support::cookie;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn the_actor_answers_its_health_with_no_node() {
    let root = std::env::temp_dir().join(format!("k7-front-health-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let missing = root.join("no-such-service.mjs").display().to_string();
    let env = [
        ("SESSION_SECRET", "test-secret".to_string()),
        ("STUDIO_AUTH_DIR", root.join("auth").display().to_string()),
        (
            "STUDIO_MEDIATEK",
            root.join("library").display().to_string(),
        ),
        (
            "STUDIO_FILES_ROOT",
            root.join("files").display().to_string(),
        ),
        ("K7_ENGINE_CORE", "front".into()),
        ("K7_FILES", "front".into()),
        ("STUDIO_LOCAL_USER", "1".into()),
        ("K7_NODE", root.join("no-such-node").display().to_string()),
        ("STUDIO_FILE_SERVICE_ENTRY", missing.clone()),
        ("STUDIO_LIBRARY_SERVICE_ENTRY", missing),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    let front = rx.await.unwrap();
    let mut up = false;
    for _ in 0..200 {
        if matches!(
            send(front, "GET", "/api/files-hub", &[]).await.status,
            200 | 401
        ) {
            up = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(up, "the front never took the file hub");
    let ann = cookie(json!({ "sub": "a", "email": "ann@x", "groups": [] }));
    let hello = send(front, "GET", "/api/files-hub", &[("cookie", ann.as_str())])
        .await
        .json();
    let ticket = hello["ticket"].as_str().unwrap().to_string();

    let health = send(
        front,
        "GET",
        "/api/files/health",
        &[("x-studio-engine-token", &ticket)],
    )
    .await;
    assert_eq!(
        (health.status, health.json()),
        (200, json!({ "ok": true })),
        "{}",
        health.text()
    );
    let by_query = send(
        front,
        "GET",
        &format!("/api/files/health?token={ticket}"),
        &[],
    )
    .await;
    assert_eq!(
        (by_query.status, by_query.json()),
        (200, json!({ "ok": true })),
        "{}",
        by_query.text()
    );
    let none = send(front, "GET", "/api/files/health", &[]).await;
    assert_eq!(none.status, 401, "the health still asks who is calling");
    let _ = std::fs::remove_dir_all(&root);
}
