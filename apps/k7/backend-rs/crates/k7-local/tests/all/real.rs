//! A REAL `k7-supervisor` run by the front, in front of a real TypeScript backend: ignored unless asked for, since it
//! needs the built supervisor and engine and a TypeScript backend behind the front (`docs/local.md`).
//! Run: `K7_REAL_SUPERVISOR=… K7_REAL_HOME=… cargo test -p k7-local --test all -- --ignored real::`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn var(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"))
}

async fn get(addr: &str, path: &str, cookie: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nCookie: {cookie}\r\n\r\n"
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
    (head[9..12].parse().unwrap_or(0), body.to_string())
}

async fn post(addr: &str, path: &str, cookie: &str, body: &str) -> u16 {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nCookie: {cookie}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out[9..12].parse().unwrap_or(0)
}

/// The snapshot frames an event stream sends until `done` holds of one, read as they arrive.
async fn watch_until(addr: &str, cookie: &str, done: impl Fn(&Value) -> bool) -> Value {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /api/local/engine/events HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: {cookie}\r\n\r\n"
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut text = String::new();
    let mut buf = [0u8; 65536];
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        assert!(Instant::now() < deadline, "no such snapshot within 120 s");
        let n = tokio::time::timeout(Duration::from_secs(60), s.read(&mut buf))
            .await
            .expect("a frame or a heartbeat within 60 s")
            .unwrap();
        text.push_str(&String::from_utf8_lossy(&buf[..n]));
        for line in text.lines().filter_map(|l| l.strip_prefix("data: ")) {
            if let Ok(v) = serde_json::from_str::<Value>(line)
                && done(&v["snapshot"])
            {
                return v;
            }
        }
    }
}

fn cookie(secret: &str) -> String {
    let user = serde_json::json!({ "sub": "dev-user", "name": "Local user", "email": "dev@example.com", "groups": [], "local": true });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    k7_auth::cookie_header(user.as_object().unwrap().clone(), "studio.sid", secret, now)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a built k7-supervisor and a TypeScript backend"]
async fn a_real_supervisor_is_run_snapshotted_commanded_and_stopped() {
    let home = PathBuf::from(var("K7_REAL_HOME"));
    let secret = var("K7_REAL_SESSION_SECRET");
    let listen = var("K7_REAL_LISTEN");
    let env = [
        ("SESSION_SECRET", secret.clone()),
        ("STUDIO_LOCAL_USER", "1".into()),
        ("K7_LOCAL_ENGINE", "front".into()),
        ("K7_SUPERVISOR_BIN", var("K7_REAL_SUPERVISOR")),
        ("K7_SUPERVISOR_HOME", home.display().to_string()),
        ("STUDIO_ENGINE", var("K7_REAL_ENGINE")),
        ("STUDIO_AUTH_DIR", var("K7_REAL_AUTH_DIR")),
        ("STUDIO_FILES_ROOT", var("K7_REAL_FILES_ROOT")),
        (
            "STUDIO_PORTS",
            format!("k7={}", listen.rsplit(':').next().unwrap()),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let config = k7_front::Config {
        listen: listen.parse().unwrap(),
        secret: var("K7_REAL_FRONT_SECRET"),
        trust_hops: 0,
        env,
    };
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let served = tokio::spawn(k7_front::serve(
        config,
        async {
            let _ = stopped.await;
        },
        |_| {},
    ));
    let c = cookie(&secret);
    let running = watch_until(&listen, &c, |s| s["status"] == "running").await;
    println!(
        "running: engine {} on :{}",
        running["snapshot"]["engineId"], running["snapshot"]["enginePort"]
    );
    let pid =
        k7_local::pidfile::read(&home.join("engine.pid")).expect("the supervisor wrote engine.pid");
    assert!(k7_local::pidfile::alive(pid.pid));

    assert_eq!(
        post(
            &listen,
            "/api/local/engine/cmd",
            &c,
            r#"{"cmd":"engine.stop"}"#
        )
        .await,
        200
    );
    watch_until(&listen, &c, |s| {
        s["userStopped"] == true && s["status"] != "running"
    })
    .await;
    assert_eq!(
        post(
            &listen,
            "/api/local/engine/cmd",
            &c,
            r#"{"cmd":"engine.start"}"#
        )
        .await,
        200
    );
    watch_until(&listen, &c, |s| {
        s["status"] == "running" && s["userStopped"] == false
    })
    .await;
    let (status, body) = get(&listen, "/api/local/engine", &c).await;
    assert_eq!(status, 200, "{body}");
    let id = running["snapshot"]["engineId"]
        .as_str()
        .unwrap()
        .to_string();

    // The ready mark lets a caller run the conformance suite through this front; the stop mark ends it.
    if let Ok(mark) = std::env::var("K7_REAL_READY_FILE") {
        std::fs::write(&mark, "ready").unwrap();
        let until = PathBuf::from(var("K7_REAL_STOP_FILE"));
        let deadline = Instant::now() + Duration::from_secs(900);
        while !until.exists() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    // A cold plugin scan holds the engine's own stop past the supervisor's grace; a settled engine stops at once.
    watch_until(&listen, &c, |s| s.get("pluginCount").is_some()).await;
    let (_, listed) = get(&listen, "/api/engines", &c).await;
    assert!(
        listed.contains(&id),
        "the registry lists the engine: {listed}"
    );
    let restarted = k7_local::pidfile::read(&home.join("engine.pid")).expect("engine.pid");
    println!(
        "engine pid {} at first, {} at the stop",
        pid.pid, restarted.pid
    );
    let _ = stop.send(());
    served.await.unwrap().unwrap();
    let log = std::fs::read_to_string(home.join(".studio.log")).unwrap_or_default();
    // Its own teardown takes about 4.1 s on Windows, the grace 4 s: an exit (1) is the grace's kill, after the engine stopped.
    let exit = log.lines().rfind(|l| l.contains("supervisor exited ("));
    println!("{}", exit.expect("the supervisor's exit is logged"));
    assert!(
        !log.contains("outlived the teardown"),
        "a clean stop needs no backstop: {log}"
    );
    assert!(
        !k7_local::pidfile::alive(restarted.pid),
        "the engine stopped with the supervisor"
    );
    assert!(
        !home.join("engine.pid").exists(),
        "the supervisor removed its pidfile"
    );
}
