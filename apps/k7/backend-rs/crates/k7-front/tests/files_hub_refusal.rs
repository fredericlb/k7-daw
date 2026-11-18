//! On a device, a refusal the account's runtime sends on the headers alone reaches the caller before any byte of the body.
//! Its own binary: one front per test binary, as the file hub's other tests have.

mod files_hub_support;
mod http_support;

use files_hub_support::{device_front, send};
use http_support::cookie;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_on_the_headers_alone_reaches_the_caller() {
    let (front, root) = device_front().await;
    let ann = cookie(json!({ "sub": "a", "email": "ann@x", "groups": [] }));
    // The account's first request makes its home; a workspace folder in it is what an upload names.
    let session = [("cookie", ann.as_str())];
    assert_eq!(
        send(front, "GET", "/api/files/health", &session)
            .await
            .status,
        200
    );
    let home = root
        .join("files")
        .join(k7_hub::account::account_key("dev@example.com"));
    std::fs::create_dir_all(home.join("workspace/w")).unwrap();
    let head = format!(
        "POST /api/files/upload?dir=.&name=big.bin HTTP/1.1\r\nHost: {front}\r\ncookie: {ann}\r\nx-studio-workspace: w\r\ncontent-type: application/octet-stream\r\ncontent-length: 1073741824\r\n\r\n"
    );
    // A device's writes wait for its libraries to settle: asked again until the 503 is gone.
    let mut out = Vec::new();
    for _ in 0..100 {
        let mut s = tokio::net::TcpStream::connect(front).await.unwrap();
        s.write_all(head.as_bytes()).await.unwrap();
        // No byte of the body is sent: the answer has to come on the headers alone.
        out.clear();
        let mut buf = [0u8; 4096];
        let read = async {
            loop {
                let n = s.read(&mut buf).await.unwrap_or(0);
                out.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&out);
                if n == 0 || text.contains("the most one upload takes") || text.contains(" 503 ") {
                    break;
                }
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(20), read)
            .await
            .expect("the refusal never came");
        if !String::from_utf8_lossy(&out).starts_with("HTTP/1.1 503") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("HTTP/1.1 413"), "{text}");
    assert!(
        text.contains("larger than 512 MB, the most one upload takes"),
        "{text}"
    );
    assert!(files_hub_support::node_never_ran(&root));
}
