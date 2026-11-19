//! The file hub of a server under `K7_FILES=front`: the hello's ticket, `/api/files` answering 410 but for the libraries,
//! the runtimes' callbacks, the seam's cut; and that no account runtime and no node program is ever started.
//! The host names node's service entries anyway: they are a marker script, and the test fails if it ran.

mod files_hub_support;
mod http_support;

use files_hub_support::{front, node_never_ran, send, send_with};
use http_support::cookie;
use serde_json::{Value, json};

const GONE: &str = "workspaces are not edited on this server: open them in k7 on a computer (the tray) or on a phone, and download them from the home page";

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
    let (front, root) = front().await;
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
        "ann@x"
    );

    // A server answers no workspace path whoever asks: 410, with or without the ticket, the session or a spoofed identity.
    let gone = json!({ "error": GONE });
    let spoofed = [
        ("x-studio-engine-token", ticket.as_str()),
        ("x-studio-user", "eve@x"),
        ("x-studio-hub-secret", "forged"),
        ("authorization", "Bearer forged"),
        ("x-studio-engine-id", "no-such-engine"),
    ];
    for (path, headers) in [
        ("/api/files/workspaces", &[][..]),
        ("/api/files/workspaces?x=1", &spoofed[..]),
        ("/api/files/health", &session[..]),
        ("/api/files/upload?dir=.&name=a", &spoofed[..]),
    ] {
        let r = send(front, "GET", path, headers).await;
        assert_eq!((r.status, r.json()), (410, gone.clone()), "{path}");
    }
    let by_query = format!("/api/files/health?token={ticket}");
    assert_eq!(send(front, "GET", &by_query, &[]).await.status, 410);
    // No account folder was made for anyone: a server has no account runtime.
    let accounts: Vec<String> = std::fs::read_dir(root.join("files"))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !accounts
            .iter()
            .any(|a| a.contains("ann") || a.contains("eve")),
        "{accounts:?}"
    );

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
    // A server's cut resolves in the libraries' context: it is asked for a user and makes no folder for them.
    let no_chain = send_with(
        front,
        "POST",
        "/api/internal/files/cold-run",
        &[("x-k7-internal", "s"), ("content-type", "application/json")],
        r#"{ "user": "someone@example.com", "chain": "", "want": "instrumental" }"#,
    )
    .await;
    for refused in [
        "user and want are required",
        "user, chain and want are required",
    ] {
        assert_ne!(
            no_chain.json()["error"],
            json!(refused),
            "{}",
            no_chain.text()
        );
    }
    let files = root.join("files");
    assert!(
        files.join("_cut").is_dir(),
        "the cut has no context of its own"
    );
    assert!(
        !files
            .join(k7_hub::account::account_key("someone@example.com"))
            .exists(),
        "a server's cut made the user's runtime"
    );
    // The slot takes the same context: asked for a user, it makes no folder for them either.
    let slot = send_with(
        front,
        "POST",
        "/api/internal/files/slot",
        &[("x-k7-internal", "s"), ("content-type", "application/json")],
        r#"{ "user": "someone@example.com", "ref": "@k7-core/none" }"#,
    )
    .await;
    assert_ne!(slot.json()["error"], json!("user and ref are required"));
    assert!(
        !files
            .join(k7_hub::account::account_key("someone@example.com"))
            .exists(),
        "a server's slot made the user's runtime"
    );
    assert!(node_never_ran(&root), "the hub started node for an account");
}
