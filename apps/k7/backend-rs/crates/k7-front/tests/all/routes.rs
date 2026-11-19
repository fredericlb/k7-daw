//! The routes the front owns, against a backend that is not there: what Rust answers, and what it hands on.

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

struct Front {
    addr: SocketAddr,
    prefs: PathBuf,
}

async fn front(name: &str, extra: &[(&str, &str)]) -> Front {
    let root = std::env::temp_dir().join(format!("k7-front-routes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut env: std::collections::BTreeMap<String, String> = [
        ("SESSION_SECRET", "test-secret"),
        ("STUDIO_PREFS_DIR", root.join("prefs").to_str().unwrap()),
        ("STUDIO_AUTH_DIR", root.join("auth").to_str().unwrap()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    for (k, v) in extra {
        if v.is_empty() {
            env.remove(*k);
        } else {
            env.insert(k.to_string(), v.to_string());
        }
    }
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
    Front {
        addr: rx.await.unwrap(),
        prefs: root.join("prefs"),
    }
}

fn cookie(sub: &str) -> String {
    let user = serde_json::json!({ "sub": sub, "email": "a@b.c" })
        .as_object()
        .unwrap()
        .clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    k7_auth::cookie_header(user, "studio.sid", "test-secret", now)
}

async fn send(addr: SocketAddr, raw: String) -> String {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out
}

fn request(method: &str, path: &str, headers: &[String], body: &str) -> String {
    let mut r = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for h in headers {
        r.push_str(h);
        r.push_str("\r\n");
    }
    format!("{r}\r\n{body}")
}

#[tokio::test]
async fn health_answers_with_the_backend_version() {
    let f = front("health", &[]).await;
    let out = send(f.addr, request("GET", "/api/health", &[], "")).await;
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert!(
        out.contains(&format!("\"version\":\"{}\"", env!("K7_VERSION"))),
        "{out}"
    );
    assert!(
        out.to_ascii_lowercase()
            .contains("x-content-type-options: nosniff"),
        "{out}"
    );
}

#[tokio::test]
async fn preferences_round_trip_into_the_backends_file() {
    let f = front("prefs", &[]).await;
    let c = format!("Cookie: {}", cookie("abc|1"));
    let json = "Content-Type: application/json".to_string();
    assert!(
        send(f.addr, request("GET", "/api/preferences", &[], ""))
            .await
            .starts_with("HTTP/1.1 401")
    );
    let empty = send(
        f.addr,
        request("GET", "/api/preferences", std::slice::from_ref(&c), ""),
    )
    .await;
    assert!(
        empty.starts_with("HTTP/1.1 200") && empty.ends_with("{}"),
        "{empty}"
    );
    let body = r#"{"favorites":["x"],"engineId":"e","commands":{"k":{"order":2.0,"junk":1}}}"#;
    let put = send(
        f.addr,
        request("PUT", "/api/preferences", &[c.clone(), json.clone()], body),
    )
    .await;
    assert!(put.ends_with(r#"{"ok":true}"#), "{put}");
    let file = std::fs::read_to_string(f.prefs.join(k7_front_file("abc|1"))).unwrap();
    assert_eq!(
        file,
        "{\n  \"engineId\": \"e\",\n  \"commands\": {\n    \"k\": {\n      \"order\": 2\n    }\n  },\n  \"favorites\": [\n    \"x\"\n  ]\n}\n"
    );
    let refused = send(
        f.addr,
        request(
            "PUT",
            "/api/preferences",
            &[c.clone(), json.clone()],
            r#"{"theme":"dark"}"#,
        ),
    )
    .await;
    assert!(
        refused.starts_with("HTTP/1.1 400") && refused.contains("does not store theme"),
        "{refused}"
    );
    let wrong = send(
        f.addr,
        request("PUT", "/api/preferences", &[c, json], r#"{"engineId":3}"#),
    )
    .await;
    assert!(wrong.starts_with("HTTP/1.1 400"), "{wrong}");
}

/// The file name `fileFor` in packages/user-prefs/store.ts gives `abc|1`.
fn k7_front_file(sub: &str) -> String {
    let slug: String = sub
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    use sha2::Digest;
    let hash = sha2::Sha256::digest(sub.as_bytes());
    format!(
        "{slug}-{}.json",
        hash.iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[tokio::test]
async fn a_cross_site_write_is_refused_and_a_preflight_answered() {
    let f = front("cors", &[]).await;
    let c = format!("Cookie: {}", cookie("a"));
    let evil = send(
        f.addr,
        request(
            "PUT",
            "/api/preferences",
            &[c, "Origin: https://evil.example".into()],
            "{}",
        ),
    )
    .await;
    assert!(
        evil.starts_with("HTTP/1.1 403") && evil.contains("cross-site request refused"),
        "{evil}"
    );
    let pre = send(
        f.addr,
        request(
            "OPTIONS",
            "/api/preferences",
            &["Origin: http://127.0.0.1:5173".into()],
            "",
        ),
    )
    .await;
    assert!(pre.starts_with("HTTP/1.1 204"), "{pre}");
    assert!(
        pre.to_ascii_lowercase()
            .contains("access-control-allow-origin: http://127.0.0.1:5173"),
        "{pre}"
    );
}

#[tokio::test]
async fn what_the_front_cannot_answer_ends_in_the_last_routes() {
    let f = front("methods", &[]).await;
    let post = send(f.addr, request("POST", "/api/preferences", &[], "")).await;
    assert!(
        post.starts_with("HTTP/1.1 401"),
        "a method the front does not own meets the `/api` guard: {post}"
    );
    let site = send(
        f.addr,
        request("GET", "/api/preferences", &["Host: k7.rocks".into()], "")
            .replace("Host: 127.0.0.1\r\n", ""),
    )
    .await;
    assert!(
        site.starts_with("HTTP/1.1 302"),
        "the root domain's host is sent to the home: {site}"
    );
}

#[tokio::test]
async fn a_device_answers_its_own_machine_only() {
    let f = front("device", &[("STUDIO_LOCAL_USER", "1")]).await;
    let foreign = send(
        f.addr,
        request("GET", "/api/health", &[], "").replace("Host: 127.0.0.1", "Host: rebound.example"),
    )
    .await;
    assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
    let c = format!("Cookie: {}", cookie("someone"));
    let own = send(f.addr, request("GET", "/api/preferences", &[c], "")).await;
    assert!(
        own.starts_with("HTTP/1.1 200")
            && own.to_ascii_lowercase().contains("set-cookie: studio.sid="),
        "the local user's session: {own}"
    );
}
