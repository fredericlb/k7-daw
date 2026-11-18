//! The leaves the front owns — the AI settings — against a backend that is not there: what Rust answers,
//! and what it hands on (a 502 here).

use std::net::SocketAddr;
use std::path::PathBuf;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Front {
    addr: SocketAddr,
    root: PathBuf,
}

async fn front(name: &str, extra: &[(&str, &str)]) -> Front {
    let root = std::env::temp_dir().join(format!("k7-front-leaves-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let gone = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    let mut env: std::collections::BTreeMap<String, String> = [
        ("SESSION_SECRET", "test-secret".to_string()),
        ("STUDIO_PREFS_DIR", root.join("prefs").display().to_string()),
        ("STUDIO_AUTH_DIR", root.join("auth").display().to_string()),
        (
            "STUDIO_MEDIATEK",
            root.join("mediatek").display().to_string(),
        ),
        ("AUTH_ADMIN_GROUPS", "Admins".to_string()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.clone()))
    .collect();
    for (k, v) in extra {
        env.insert(k.to_string(), v.to_string());
    }
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: gone.to_string(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    Front {
        addr: rx.await.unwrap(),
        root,
    }
}

fn cookie(user: Value) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let header = k7_auth::cookie_header(
        user.as_object().unwrap().clone(),
        "studio.sid",
        "test-secret",
        now,
    );
    format!("Cookie: {header}")
}

/// Status, headers lowercased, and the body.
async fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[String],
    body: &str,
) -> (u16, String, String) {
    let mut r = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for h in headers {
        r.push_str(h);
        r.push_str("\r\n");
    }
    r.push_str("\r\n");
    r.push_str(body);
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(r.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (
        head[9..12].parse().unwrap(),
        head.to_ascii_lowercase(),
        body.to_string(),
    )
}

fn json_type() -> String {
    "Content-Type: application/json".into()
}

#[tokio::test]
async fn ai_settings_keep_the_key_and_show_only_its_hint() {
    let f = front("ai", &[]).await;
    let c = cookie(json!({ "sub": "acct|1" }));
    assert_eq!(
        call(f.addr, "GET", "/api/ai/settings", &[], "").await.0,
        401
    );
    let (s, _, b) = call(
        f.addr,
        "GET",
        "/api/ai/organize",
        std::slice::from_ref(&c),
        "",
    )
    .await;
    assert_eq!((s, b.as_str()), (200, r#"{"enabled":false,"model":null}"#));
    let put = r#"{"apiKey":"  sk-ant-api03-secret-wxyz ","model":"claude-sonnet-4-5"}"#;
    let (s, _, b) = call(
        f.addr,
        "PUT",
        "/api/ai/settings",
        &[c.clone(), json_type()],
        put,
    )
    .await;
    assert_eq!(s, 200, "{b}");
    assert_eq!(
        b,
        r#"{"model":"claude-sonnet-4-5","defaultModel":"claude-opus-5","keyHint":"sk-ant-…wxyz"}"#
    );
    let files: Vec<_> = std::fs::read_dir(f.root.join("prefs").join("ai"))
        .unwrap()
        .collect();
    let file = std::fs::read_to_string(files[0].as_ref().unwrap().path()).unwrap();
    assert_eq!(
        file,
        "{\n  \"apiKey\": \"sk-ant-api03-secret-wxyz\",\n  \"model\": \"claude-sonnet-4-5\"\n}\n"
    );
    let (_, _, b) = call(
        f.addr,
        "GET",
        "/api/ai/organize",
        std::slice::from_ref(&c),
        "",
    )
    .await;
    assert_eq!(b, r#"{"enabled":true,"model":"claude-sonnet-4-5"}"#);
    let (s, _, b) = call(
        f.addr,
        "PUT",
        "/api/ai/settings",
        &[c.clone(), json_type()],
        r#"{"model":null}"#,
    )
    .await;
    assert_eq!(
        (s, b.as_str()),
        (
            200,
            r#"{"model":null,"defaultModel":"claude-opus-5","keyHint":"sk-ant-…wxyz"}"#
        )
    );
    let (s, _, b) = call(
        f.addr,
        "PUT",
        "/api/ai/settings",
        &[c.clone(), json_type()],
        r#"{"foo":1}"#,
    )
    .await;
    assert_eq!(
        (s, b.as_str()),
        (400, r#"{"error":"Unrecognized key: \"foo\""}"#)
    );
    let (s, _, _) = call(
        f.addr,
        "PUT",
        "/api/ai/settings",
        &[c.clone(), json_type()],
        "5",
    )
    .await;
    assert_eq!(s, 400);
}

#[tokio::test]
async fn the_removed_organize_route_and_a_bearer_are_the_backends() {
    let f = front("ai-proxied", &[]).await;
    let c = cookie(json!({ "sub": "a" }));
    let (s, _, _) = call(
        f.addr,
        "POST",
        "/api/ai/organize",
        &[c.clone(), json_type()],
        "{}",
    )
    .await;
    assert_eq!(
        s, 502,
        "the whole-conversation route is no longer the front's: it is handed on"
    );
    let bearer = "Authorization: Bearer some-library-token".to_string();
    let (s, _, _) = call(f.addr, "GET", "/api/ai/settings", &[c, bearer], "").await;
    assert_eq!(s, 502, "a bearer is checked by the backend's /api guard");
}
