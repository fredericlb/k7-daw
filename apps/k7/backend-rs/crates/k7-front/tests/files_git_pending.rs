//! A workspace whose merge waits for choices refuses a write to its files with 423, through the front's whole route.
//! Its own binary: one front per test binary, as the file hub's other tests have.

mod files_hub_support;
mod http_support;

use files_hub_support::{device_front_with, send, send_with, settled};
use http_support::cookie;

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_with_a_pending_merge_refuses_a_write_with_423_and_another_does_not() {
    let account = k7_hub::account::account_key("dev@example.com");
    let (front, root) = device_front_with(|root| {
        let home = root.join("files").join(&account);
        for w in ["waiting", "mine"] {
            std::fs::create_dir_all(home.join("workspace").join(w)).unwrap();
            std::fs::write(home.join("workspace").join(w).join("notes.md"), "old").unwrap();
        }
        let git = home.join("workspace/waiting/.git");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("k7-merge.json"), "{}").unwrap();
    })
    .await;
    settled(front).await;
    let ann = cookie();
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
        async move {
            send_with(
                front,
                "PUT",
                "/api/files/file",
                &headers,
                r#"{"path":"notes.md","content":"new"}"#,
            )
            .await
        }
    };
    let refused = put("waiting").await;
    assert_eq!(refused.status, 423, "{}", refused.text());
    assert_eq!(refused.json()["code"], "merge-pending");
    assert_eq!(
        std::fs::read_to_string(home.join("workspace/waiting/notes.md")).unwrap(),
        "old"
    );
    let ok = put("mine").await;
    assert_ne!(ok.status, 423, "{}", ok.text());
}
