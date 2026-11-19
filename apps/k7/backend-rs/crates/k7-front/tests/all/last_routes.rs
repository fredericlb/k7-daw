//! `K7_LAST_ROUTES=front`: the built page and its fallback, Express's 404, the site page, the bearer
//! guard and the seam's `verify-token`, answered with a backend that counts every connection and must see none.
//! What a reader gets wrong: the hub's services cannot start here, which none of these routes needs.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

async fn send(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let host = headers
        .iter()
        .find(|(k, _)| *k == "host")
        .map_or(addr.to_string(), |(_, v)| v.to_string());
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers.iter().filter(|(k, _)| *k != "host") {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    let at = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..at]).to_string();
    let mut lines = head.lines();
    let status = lines.next().unwrap()[9..12].parse().unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: out[at + 4..].to_vec(),
    }
}

/// A backend that answers nothing and counts who called it.
async fn counting_backend() -> (SocketAddr, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let counted = seen.clone();
    tokio::spawn(async move {
        while let Ok((s, _)) = l.accept().await {
            counted.fetch_add(1, Ordering::Relaxed);
            drop(s);
        }
    });
    (addr, seen)
}

fn cookie() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let user = serde_json::json!({ "sub": "ann", "email": "ann@x", "groups": [] });
    k7_auth::cookie_header(
        user.as_object().unwrap().clone(),
        "studio.sid",
        "test-secret",
        now,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_routes_ask_the_backend_nothing() {
    let root = std::env::temp_dir().join(format!("k7-front-last-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dist = root.join("dist");
    std::fs::create_dir_all(dist.join("assets")).unwrap();
    std::fs::write(dist.join("index.html"), "<!doctype html><p>shell</p>").unwrap();
    std::fs::write(dist.join("assets/app-1.js"), "console.log(1)").unwrap();
    std::fs::write(dist.join("assets/app-1.js.gz"), b"\x1f\x8bgz").unwrap();
    let (backend, seen) = counting_backend().await;
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
        ("K7_LAST_ROUTES", "front".into()),
        ("K7_FRONTEND_DIST", dist.display().to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: backend.to_string(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    let front = rx.await.unwrap();
    let ann = cookie();

    let shell = send(front, "GET", "/some/page", &[], "").await;
    assert_eq!(
        (shell.status, shell.text().as_str()),
        (200, "<!doctype html><p>shell</p>")
    );
    assert_eq!(shell.header("cache-control"), Some("no-cache"));
    assert_eq!(
        shell.header("content-type"),
        Some("text/html; charset=UTF-8")
    );
    assert_eq!(shell.header("x-frame-options"), Some("SAMEORIGIN"));
    assert_eq!(shell.header("vary"), Some("Accept-Encoding"));
    let gz = send(
        front,
        "GET",
        "/assets/app-1.js",
        &[("accept-encoding", "gzip")],
        "",
    )
    .await;
    assert_eq!(
        (gz.status, gz.header("content-encoding")),
        (200, Some("gzip"))
    );
    assert_eq!(
        gz.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );
    assert_eq!(
        gz.header("content-type"),
        Some("application/javascript; charset=utf-8")
    );
    let plain = send(front, "GET", "/assets/app-1.js", &[], "").await;
    assert_eq!(
        (plain.text().as_str(), plain.header("accept-ranges")),
        ("console.log(1)", Some("bytes"))
    );
    let part = send(
        front,
        "GET",
        "/assets/app-1.js",
        &[("range", "bytes=0-6")],
        "",
    )
    .await;
    assert_eq!((part.status, part.text().as_str()), (206, "console"));
    let fresh = send(
        front,
        "GET",
        "/assets/app-1.js",
        &[("if-none-match", plain.header("etag").unwrap())],
        "",
    )
    .await;
    assert_eq!(fresh.status, 304);
    let missing = send(front, "GET", "/assets/missing.js", &[], "").await;
    assert_eq!(missing.status, 404);
    assert!(
        missing
            .text()
            .contains("<pre>Cannot GET /assets/missing.js</pre>")
    );
    let folder = send(front, "GET", "/assets", &[], "").await;
    assert_eq!(
        (folder.status, folder.header("location")),
        (301, Some("/assets/"))
    );

    let anonymous = send(front, "GET", "/api/conformance", &[], "").await;
    assert_eq!(
        (anonymous.status, anonymous.text().as_str()),
        (401, r#"{"error":"unauthenticated"}"#)
    );
    let signed = send(front, "GET", "/api/conformance", &[("cookie", &ann)], "").await;
    assert_eq!(signed.status, 404);
    let forged = send(
        front,
        "GET",
        "/api/preferences",
        &[("cookie", &ann), ("authorization", "Bearer x.y")],
        "",
    )
    .await;
    assert_eq!(
        (forged.status, forged.text().as_str()),
        (401, r#"{"error":"invalid token"}"#)
    );

    let site = send(front, "GET", "/", &[("host", "k7.rocks")], "").await;
    assert!(site.status == 200 && site.text().contains("<h1>Coming soon</h1>"));
    let away = send(front, "GET", "/x", &[("host", "www.k7.rocks")], "").await;
    assert_eq!(
        (away.status, away.header("location"), away.text().as_str()),
        (302, Some("/"), "Found. Redirecting to /")
    );
    let gate = [("x-k7-internal", "s"), ("content-type", "application/json")];
    let body = serde_json::json!({ "token": "x.y" }).to_string();
    let verified = send(
        front,
        "POST",
        "/api/internal/files/verify-token",
        &gate,
        &body,
    )
    .await;
    assert_eq!(
        (verified.status, verified.text().as_str()),
        (200, r#"{"sub":null}"#)
    );
    assert_eq!(verified.header("cache-control"), Some("no-store"));
    let refused = send(
        front,
        "POST",
        "/api/internal/files/verify-token",
        &[("x-k7-internal", "no")],
        "",
    )
    .await;
    assert_eq!(refused.status, 401);

    assert_eq!(seen.load(Ordering::Relaxed), 0, "the backend was asked");
    let _ = std::fs::remove_dir_all(&root);
}
