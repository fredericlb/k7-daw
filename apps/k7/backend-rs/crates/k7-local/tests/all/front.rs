//! The front running the local engine against `k7-fake-supervisor`: the `/api/local*` family,
//! the internal seam's local half and the stop order, started with `STUDIO_LOCAL_USER=1` and no sign-in provider.
//! Also a device whose engine the app hosts (Android): the probe and the tray's line stay the front's.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const FAKE: &str = env!("CARGO_BIN_EXE_k7-fake-supervisor");
const SECRET: &str = "front-secret";
const SESSION: &str = "session-secret";

struct Front {
    addr: SocketAddr,
    home: PathBuf,
    stop: tokio::sync::oneshot::Sender<()>,
    served: tokio::task::JoinHandle<std::io::Result<()>>,
}

async fn front(name: &str) -> Front {
    front_with(name, true).await
}

/// A device's front; `engine` false is a device whose app hosts the supervisor, the switch off.
async fn front_with(name: &str, engine: bool) -> Front {
    let home = std::env::temp_dir().join(format!("k7-local-front-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let env = [
        ("SESSION_SECRET", SESSION),
        ("STUDIO_LOCAL_USER", "1"),
        ("K7_ENGINE_CORE", "front"),
        ("K7_LOCAL_ENGINE", if engine { "front" } else { "" }),
        ("K7_SUPERVISOR_BIN", FAKE),
        ("K7_SUPERVISOR_HOME", home.to_str().unwrap()),
        ("STUDIO_AUTH_DIR", home.join("auth").to_str().unwrap()),
        ("K7_RELEASE", "v9"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        secret: SECRET.into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let served = tokio::spawn(k7_front::serve(
        config,
        async {
            let _ = stopped.await;
        },
        move |a| tx.send(a).unwrap(),
    ));
    Front {
        addr: rx.await.unwrap(),
        home,
        stop,
        served,
    }
}

fn cookie() -> String {
    let user = json!({ "sub": "dev-user", "name": "Local user", "email": "dev@example.com", "groups": [], "local": true });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    k7_auth::cookie_header(
        user.as_object().unwrap().clone(),
        "studio.sid",
        SESSION,
        now,
    )
}

struct Reply {
    status: u16,
    head: String,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

async fn send(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    let out = String::from_utf8_lossy(&out).to_string();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(body)
    } else {
        body.to_string()
    };
    Reply {
        status: head[9..12].parse().unwrap(),
        head: head.to_ascii_lowercase(),
        body,
    }
}

fn dechunk(mut s: &str) -> String {
    let mut out = String::new();
    while let Some((size, rest)) = s.split_once("\r\n") {
        let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        if n == 0 {
            break;
        }
        out.push_str(&rest[..n]);
        s = &rest[n + 2..];
    }
    out
}

async fn until<T>(mut read: impl AsyncFnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(v) = read().await {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn log_of(home: &Path) -> String {
    std::fs::read_to_string(home.join(".studio.log")).unwrap_or_default()
}

/// The first `n` bytes an event stream sends, read without waiting for its end.
async fn first_frames(addr: SocketAddr, cookie: &str) -> String {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /api/local/engine/events HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: {cookie}\r\n\r\n"
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    let deadline = Instant::now() + Duration::from_secs(5);
    while !String::from_utf8_lossy(&got).contains("\"snapshot\":") {
        assert!(Instant::now() < deadline, "no frame within 5 s: buffered");
        let n = tokio::time::timeout(Duration::from_secs(5), s.read(&mut buf))
            .await
            .unwrap()
            .unwrap();
        got.extend_from_slice(&buf[..n]);
    }
    String::from_utf8_lossy(&got).to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_front_runs_the_engine_its_routes_and_its_stop() {
    let f = front("runs").await;
    let c = cookie();
    let auth = [("cookie", c.as_str())];

    let info = send(f.addr, "GET", "/api/local", &[], "").await;
    assert_eq!(info.json(), json!({ "engine": true, "version": "v9" }));
    assert_eq!(
        send(f.addr, "GET", "/api/local/engine", &[], "")
            .await
            .status,
        401
    );
    let snap = until(async || {
        let r = send(f.addr, "GET", "/api/local/engine", &auth, "").await;
        let s = r.json()["snapshot"].clone();
        s.is_object().then_some(s)
    })
    .await;
    assert_eq!(snap["status"], "running");
    assert_eq!(
        snap["registryUrl"],
        format!("http://127.0.0.1:{}", f.addr.port())
    );
    let pinned = std::fs::read_to_string(f.home.join("engine-host.json")).unwrap();
    assert!(pinned.contains(&format!("http://127.0.0.1:{}", f.addr.port())));

    let sse = first_frames(f.addr, &c).await;
    assert!(sse.contains("text/event-stream; charset=utf-8"), "{sse}");
    assert!(sse.contains("retry: 2000\n"), "{sse}");

    let json_auth = [("cookie", c.as_str()), ("content-type", "application/json")];
    let cmd = |b: &'static str| send(f.addr, "POST", "/api/local/engine/cmd", &json_auth, b);
    assert_eq!(cmd(r#"{"cmd":"engine.restart"}"#).await.status, 200);
    assert_eq!(
        cmd(r#"{"cmd":"audio.priority","arg":"A\u001fB"}"#)
            .await
            .status,
        200
    );
    assert_eq!(
        cmd(r#"{"cmd":"shutdown"}"#).await.json()["error"],
        "command not allowed"
    );
    assert_eq!(
        cmd(r#"{"cmd":"engine.start","arg":3}"#).await.json()["error"],
        "arg must be a string"
    );
    let slot = send(
        f.addr,
        "POST",
        "/api/local/engine/slots",
        &[("cookie", c.as_str()), ("content-type", "application/json")],
        r#"{"id":"warm","doc":{"a":1}}"#,
    )
    .await;
    assert_eq!(slot.json(), json!({ "ok": true, "id": "warm" }));

    let seam = "/api/internal/engine/local";
    assert_eq!(send(f.addr, "GET", seam, &[], "").await.status, 401);
    assert_eq!(
        send(f.addr, "GET", seam, &[("x-k7-internal", "nope")], "")
            .await
            .status,
        401
    );
    let answered = send(f.addr, "GET", seam, &[("x-k7-internal", SECRET)], "").await;
    assert_eq!(answered.json(), json!({ "answered": true }));
    assert!(answered.head.contains("cache-control: no-store"));

    let _ = f.stop.send(());
    f.served.await.unwrap().unwrap();
    let log = log_of(&f.home);
    assert!(log.contains("supervisor exited (0)"), "{log}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_whose_app_hosts_the_engine_keeps_its_probe() {
    let f = front_with("no-engine", false).await;
    let info = send(f.addr, "GET", "/api/local", &[], "").await;
    assert_eq!(info.json(), json!({ "engine": false, "version": "v9" }));
    assert!(!log_of(&f.home).contains("starting supervisor"));
    let _ = f.stop.send(());
    f.served.await.unwrap().unwrap();
}
