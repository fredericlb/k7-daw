//! The engine core's switch: OFF, every engine path goes to the backend; ON, the front answers them.

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A backend that answers 418 to everything.
async fn backend() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = l.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let _ = s.read(&mut buf).await;
                let reply =
                    "HTTP/1.1 418 I'm a teapot\r\nContent-Length: 2\r\nConnection: close\r\n\r\nts";
                let _ = s.write_all(reply.as_bytes()).await;
            });
        }
    });
    addr
}

async fn front(engine_core: bool, upstream: SocketAddr) -> SocketAddr {
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "k7-front-engine-{engine_core}-{}-{run}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let mut env: std::collections::BTreeMap<String, String> = [
        ("SESSION_SECRET", "test-secret".to_string()),
        (
            "STUDIO_AUTH_DIR",
            root.join("auth").to_string_lossy().into_owned(),
        ),
        (
            "STUDIO_ENGINES_DIR",
            root.join("engines").to_string_lossy().into_owned(),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    if engine_core {
        env.insert("K7_ENGINE_CORE".into(), "front".into());
    }
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream.to_string(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    rx.await.unwrap()
}

/// The status and the body of one exchange.
async fn send(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String) {
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (head[9..12].parse().unwrap(), body.to_string())
}

const BEAT: &str = r#"{"id":"e1","token":"tok","hostname":"h","addresses":{"port":8765}}"#;

#[tokio::test]
async fn off_every_engine_path_is_the_backends() {
    let front = front(false, backend().await).await;
    let json = [("content-type", "application/json")];
    for (method, path) in [
        ("GET", "/api/registry/hello"),
        ("POST", "/api/registry/heartbeat"),
        ("GET", "/api/engines"),
        ("POST", "/api/engine/select"),
        ("GET", "/api/internal/engine/hub"),
    ] {
        assert_eq!(
            send(front, method, path, &json, BEAT).await,
            (418, "ts".into()),
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn on_the_front_answers() {
    let front = front(true, backend().await).await;
    let json = ("content-type", "application/json");
    assert_eq!(
        send(front, "GET", "/api/registry/hello", &[], "").await,
        (200, r#"{"service":"k7-registry"}"#.into())
    );
    let cookie = k7_auth::cookie_header(
        serde_json::json!({ "email": "ann@x" })
            .as_object()
            .unwrap()
            .clone(),
        "studio.sid",
        "test-secret",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64,
    );
    let (status, body) = send(front, "GET", "/api/engines", &[("cookie", &cookie)], "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, "[]");
    // A heartbeat with no render key is taken as an unpaired one.
    let (status, _) = send(front, "POST", "/api/registry/heartbeat", &[json], BEAT).await;
    assert_eq!(status, 200);
    let (status, body) = send(front, "GET", "/api/engines", &[("cookie", &cookie)], "").await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains(r#""id":"e1""#), "{body}");
    // A method the core does not answer there, and the local engine's half of the seam, stay the backend's.
    assert_eq!(
        send(front, "GET", "/api/registry/heartbeat", &[], "")
            .await
            .0,
        418
    );
    assert_eq!(
        send(
            front,
            "GET",
            "/api/internal/engine/local",
            &[("x-k7-internal", "s")],
            ""
        )
        .await
        .0,
        418
    );
    let (status, body) = send(
        front,
        "GET",
        "/api/internal/engine/hub",
        &[("x-k7-internal", "s")],
        "",
    )
    .await;
    assert_eq!(status, 200);
    let hub: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(hub["rendersEnabled"], false);
    assert_eq!(hub["renderer"], serde_json::Value::Null);
    let epoch = hub["epoch"].as_str().unwrap().to_string();
    assert!(epoch.len() >= 16, "{body}");
    // A binding re-established under the key its caller held, and a key shorter than a drawn one refused.
    let seam = [("x-k7-internal", "s"), json];
    let key = "k".repeat(32);
    let (status, body) = send(
        front,
        "POST",
        "/api/internal/engine/bindings",
        &seam,
        &format!(r#"{{"engineId":"e1","roots":[],"kind":"engine","key":"{key}"}}"#),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({ "path": format!("/api/engine-tunnel/local/{key}"), "epoch": epoch })
    );
    for bad in [r#""short""#, "42"] {
        let (status, _) = send(
            front,
            "POST",
            "/api/internal/engine/bindings",
            &seam,
            &format!(r#"{{"engineId":null,"roots":[],"kind":"jobs","key":{bad}}}"#),
        )
        .await;
        assert_eq!(status, 400, "{bad}");
    }
    assert_eq!(
        send(
            front,
            "GET",
            "/api/internal/engine/hub",
            &[("x-k7-internal", "x")],
            ""
        )
        .await
        .0,
        401
    );
}
