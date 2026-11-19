//! The file hub of a device under `K7_FILES=front`: the hello's ticket, the libraries, the runtimes' callbacks, the
//! seam's cut; and that no node program is ever started.
//! The host names node's service entries anyway: they are a marker script, and the test fails if it ran.

mod files_hub_support;
mod http_support;

use files_hub_support::{device_front, node_never_ran, send};
use http_support::cookie;
use serde_json::{Value, json};

fn read_ticket(ticket: &str) -> Value {
    use base64::Engine;
    let payload = ticket.split('.').next().unwrap();
    serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hub_in_the_front() {
    let (front, root) = device_front().await;
    let ann = cookie(json!({ "sub": "a", "email": "ann@x", "groups": [] }));
    let session = [("cookie", ann.as_str())];
    assert_eq!(
        send(front, "GET", "/api/files-hub", &[]).await.json(),
        json!({ "error": "unauthenticated" })
    );
    let hello = send(front, "GET", "/api/files-hub", &session).await.json();
    assert_eq!(
        hello.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["path", "ticket"]
    );
    assert_eq!(hello["path"], json!("/api/files"));
    let ticket = hello["ticket"].as_str().unwrap().to_string();
    let payload = read_ticket(&ticket);
    assert_eq!(
        payload.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["u", "exp"]
    );
    assert_eq!(
        k7_hub::ticket::read(b"test-secret:files", &ticket, 0.0)
            .unwrap()
            .user,
        "dev@example.com",
        "a device's session is its local user, whoever the cookie names"
    );

    let spoofed = [
        ("x-studio-engine-token", ticket.as_str()),
        ("x-studio-user", "eve@x"),
        ("x-studio-hub-secret", "forged"),
        ("authorization", "Bearer forged"),
        ("x-studio-engine-id", "no-such-engine"),
    ];

    // The libraries are the front's own: asked with a forged secret in a header, they answer without it.
    let libs = send(front, "GET", "/api/files/account/libraries", &spoofed).await;
    assert!(
        (200..500).contains(&libs.status),
        "{} {}",
        libs.status,
        libs.text()
    );

    let internal = send(
        front,
        "POST",
        "/api/files-hub/internal/libraries/sync",
        &[("x-studio-hub-secret", "forged")],
    )
    .await;
    assert_eq!(
        (internal.status, internal.json()),
        (
            401,
            json!({ "error": "not one of this hub's file services" })
        )
    );

    let cold = send(
        front,
        "POST",
        "/api/internal/files/cold-run",
        &[("x-k7-internal", "s")],
    )
    .await;
    assert_eq!(
        (cold.status, cold.json()),
        (400, json!({ "error": "user and want are required" }))
    );
    assert!(node_never_ran(&root), "the hub started node for an account");
}
