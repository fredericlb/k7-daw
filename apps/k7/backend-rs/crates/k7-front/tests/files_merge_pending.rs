//! A workspace whose merge waits for choices refuses a write to its files with 423 and a sentence, through the front's whole route.
//! Its own binary: one front per test binary, as the file hub's other tests have.

mod files_hub_support;
mod http_support;

use files_hub_support::{device_front_with, send, send_with, settled};
use http_support::cookie;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_with_a_merge_waiting_refuses_a_write_with_423_and_another_does_not() {
    // The workspaces and the pending merge are on the disk before the account starts.
    let (front, root) = device_front_with(|root| {
        let home = root
            .join("files")
            .join(k7_hub::account::account_key("dev@example.com"));
        for w in ["shared", "mine"] {
            std::fs::create_dir_all(home.join("workspace").join(w)).unwrap();
            std::fs::write(home.join("workspace").join(w).join("notes.md"), "old").unwrap();
        }
        let git = home.join("workspace/shared/.git");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("k7-merge.json"), "{}").unwrap();
    })
    .await;
    settled(front).await;
    let ann = cookie(json!({ "sub": "a", "email": "dev@example.com", "groups": [] }));
    let session = [
        ("cookie", ann.as_str()),
        ("content-type", "application/json"),
    ];
    let home = root
        .join("files")
        .join(k7_hub::account::account_key("dev@example.com"));
    assert_eq!(
        send(front, "GET", "/api/files/health", &session)
            .await
            .status,
        200
    );
    let put = |w: &'static str| {
        let headers = [
            ("cookie", ann.as_str()),
            ("content-type", "application/json"),
            ("x-studio-workspace", w),
        ];
        let body = r#"{"path":"notes.md","content":"new"}"#;
        async move { send_with(front, "PUT", "/api/files/file", &headers, body).await }
    };
    let refused = put("shared").await;
    assert_eq!(refused.status, 423, "{}", refused.text());
    assert_eq!(refused.json()["code"], "merge-pending");
    assert_eq!(
        std::fs::read_to_string(home.join("workspace/shared/notes.md")).unwrap(),
        "old"
    );
    // The routes that file a result in the workspace are as read-only as the file put.
    for path in [
        "/api/files/library/move",
        "/api/files/module",
        "/api/files/verb-host/take/save",
        "/api/files/verb-host/picture",
        "/api/files/verb-host/prepare",
        "/api/files/module-base/snapshot",
    ] {
        let headers = [
            ("cookie", ann.as_str()),
            ("content-type", "application/json"),
            ("x-studio-workspace", "shared"),
        ];
        let said = send_with(front, "POST", path, &headers, "{}").await;
        assert_eq!(said.status, 423, "{path}: {}", said.text());
    }
    let allowed = put("mine").await;
    assert_ne!(allowed.status, 423, "{}", allowed.text());
    assert_eq!(
        std::fs::read_to_string(home.join("workspace/mine/notes.md")).unwrap(),
        "new"
    );
    assert!(files_hub_support::node_never_ran(&root));
}
