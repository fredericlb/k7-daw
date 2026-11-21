//! tests.rs — the door and the relay over fronts in this process: a device that serves a fake engine, and another that
//! lists it through a fixed peer list. What a reader gets wrong: the door's port is 0 and read back, never fixed.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use axum::extract::ConnectInfo;
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tower::ServiceExt;

use super::confine::Served;
use super::door::ServedInfo;
use crate::{AppState, Config};

const TOKEN: &str = "lan-test-token-0123456789abcdef0123456789abcdef";

struct Device {
    addr: SocketAddr,
    state: Weak<AppState>,
}

impl Device {
    fn lan(&self) -> Arc<super::Lan> {
        self.state
            .upgrade()
            .unwrap()
            .lan
            .clone()
            .expect("the family is on")
    }

    async fn door(&self) -> Option<SocketAddr> {
        let mut rx = self.lan().door_addr();
        tokio::time::timeout(Duration::from_secs(10), rx.wait_for(|a| a.is_some()))
            .await
            .ok()
            .and_then(|r| r.ok().and_then(|a| *a))
    }

    async fn door_closed(&self) -> bool {
        let mut rx = self.lan().door_addr();
        tokio::time::timeout(Duration::from_secs(10), rx.wait_for(|a| a.is_none()))
            .await
            .is_ok()
    }
}

/// A device's front: the engine core on, a local user, discovery as `peers` says, the door on a free port.
async fn device(name: &str, peers: &str) -> Device {
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("k7-lan-{name}-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let env: BTreeMap<String, String> = [
        ("K7_SESSION_TOKEN", TOKEN),
        ("STUDIO_LOCAL_USER", "1"),
        ("K7_ENGINE_CORE", "front"),
        ("K7_LAN_LISTEN", "127.0.0.1:0"),
        ("K7_LAN_PEERS", peers),
        ("STUDIO_AUTH_DIR", root.join("auth").to_str().unwrap()),
        ("STUDIO_ENGINES_DIR", root.join("engines").to_str().unwrap()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let config = Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx_addr, rx_addr) = tokio::sync::oneshot::channel();
    let (tx_state, rx_state) = tokio::sync::oneshot::channel();
    tokio::spawn(crate::serve_seen(
        config,
        Default::default(),
        std::future::pending(),
        move |a| tx_addr.send(a).unwrap(),
        move |w| {
            let _ = tx_state.send(w);
        },
    ));
    Device {
        addr: rx_addr.await.unwrap(),
        state: rx_state.await.unwrap(),
    }
}

struct Reply {
    status: u16,
    body: String,
}

impl Reply {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
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
    Reply {
        status: head[9..12].parse().unwrap(),
        body: body.to_string(),
    }
}

/// An engine's socket: answers `whoami` with an id and a token, echoes every other text.
async fn fake_engine(id: &'static str, token: &'static str) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((s, _)) = listener.accept().await {
            tokio::spawn(async move {
                let Ok(mut ws) = tokio_tungstenite::accept_async(s).await else {
                    return;
                };
                while let Some(Ok(Message::Text(t))) = ws.next().await {
                    let reply = if t.contains("whoami") {
                        format!(r#"{{"type":"whoami","id":"{id}","token":"{token}"}}"#)
                    } else {
                        format!("echo:{t}")
                    };
                    if ws.send(Message::text(reply)).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

/// A heartbeat from loopback: what makes `id` this machine's engine.
async fn register(d: &Device, id: &str, token: &str, engine_port: u16) {
    let beat = format!(
        r#"{{"id":"{id}","token":"{token}","hostname":"studio-{id}","addresses":{{"port":{engine_port}}}}}"#
    );
    let r = send(
        d.addr,
        "POST",
        "/api/registry/heartbeat",
        &[("content-type", "application/json")],
        &beat,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
}

fn cookie() -> String {
    format!("k7.sid={TOKEN}")
}

/// A on its own, engine registered, switch on and door bound.
async fn open_device(name: &str, id: &str, token: &'static str) -> (Device, SocketAddr, u16) {
    let engine = fake_engine(Box::leak(id.to_string().into_boxed_str()), token).await;
    let a = device(name, "none").await;
    register(&a, id, token, engine).await;
    a.lan().set_forced(true);
    let door = a
        .door()
        .await
        .expect("the door is bound once the switch is on");
    (a, door, engine)
}

#[tokio::test]
async fn the_door_follows_the_switch() {
    let (a, door, _) = open_device("switch", "A1", "tok-A").await;
    let asked = send(door, "GET", "/lan/hello", &[("x-k7-engine", "tok-A")], "").await;
    assert_eq!(asked.status, 200, "{}", asked.body);
    a.lan().set_forced(false);
    assert!(
        a.door_closed().await,
        "the door is unbound once the switch is off"
    );
    assert!(tokio::net::TcpStream::connect(door).await.is_err());
    a.lan().set_forced(true);
    assert!(
        a.door().await.is_some(),
        "and bound again when it is back on"
    );
}

#[tokio::test]
async fn a_bad_token_or_a_public_peer_is_refused() {
    let (a, door, _) = open_device("guard", "A1", "tok-A").await;
    for token in [None, Some("wrong"), Some("")] {
        let h: Vec<(&str, &str)> = token.map(|t| vec![("x-k7-engine", t)]).unwrap_or_default();
        let r = send(door, "GET", "/lan/hello", &h, "").await;
        assert_eq!(r.status, 401, "{token:?}");
    }
    // A public peer, with the right token: the router is called as the listener would, the address set.
    let state = a.state.upgrade().unwrap();
    let mut req = axum::http::Request::builder()
        .uri("/lan/hello")
        .header("x-k7-engine", "tok-A")
        .body(axum::body::Body::empty())
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("8.8.8.8:5555".parse::<SocketAddr>().unwrap()));
    let r = super::door::router(state).oneshot(req).await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn files_outside_the_served_project_and_every_open_are_refused() {
    let (a, door, _) = open_device("files", "A1", "tok-A").await;
    let h = |ws: &'static str| [("x-k7-engine", "tok-A"), ("x-studio-workspace", ws)];
    // Nothing is served on this device, so nothing is reachable.
    assert_eq!(
        send(door, "GET", "/lan/files/revision", &h("band"), "")
            .await
            .status,
        403
    );
    let served = ServedInfo {
        served: Served {
            workspace: "band".into(),
            path: "mix.k7".into(),
        },
        label: "band / mix.k7".into(),
        app: "k7".into(),
        surfaces: serde_json::json!([{ "id": "desk", "type": "turntables" }]),
    };
    *a.lan().served.lock().unwrap() =
        Some((Instant::now() + Duration::from_secs(600), Some(served)));
    for (m, p, ws) in [
        ("GET", "/lan/files/revision", "other"),
        ("POST", "/lan/files/open", "band"),
        ("POST", "/lan/files/engine/project", "band"),
        ("POST", "/lan/files/workspaces", "band"),
    ] {
        assert_eq!(
            send(door, m, p, &h(ws), "{}").await.status,
            403,
            "{m} {p} {ws}"
        );
    }
}

#[tokio::test]
async fn hello_has_its_shape_with_a_project_and_without() {
    let (a, door, _) = open_device("hello", "A1", "tok-A").await;
    let h = [("x-k7-engine", "tok-A")];
    let bare = send(door, "GET", "/lan/hello", &h, "").await.json();
    assert_eq!(bare["device"]["engineId"], "A1");
    assert_eq!(bare["device"]["name"], "studio-A1");
    assert_eq!(bare["device"]["api"], super::API);
    assert!(bare["project"].is_null());
    assert_eq!(bare["surfaces"], serde_json::json!([]));
    let served = ServedInfo {
        served: Served {
            workspace: "band".into(),
            path: "mix.k7".into(),
        },
        label: "band / mix.k7".into(),
        app: "k7".into(),
        surfaces: serde_json::json!([{ "id": "desk", "type": "turntables" }]),
    };
    *a.lan().served.lock().unwrap() =
        Some((Instant::now() + Duration::from_secs(600), Some(served)));
    let put = send(
        door,
        "PUT",
        "/lan/screens/s1",
        &[
            ("x-k7-engine", "tok-A"),
            ("content-type", "application/json"),
        ],
        r#"{"surface":"desk","device":"iPad"}"#,
    )
    .await;
    assert_eq!(put.status, 200, "{}", put.body);
    let full = send(door, "GET", "/lan/hello", &h, "").await.json();
    assert_eq!(full["project"]["workspace"], "band");
    assert_eq!(full["project"]["label"], "band / mix.k7");
    assert_eq!(full["surfaces"][0]["id"], "desk");
    assert_eq!(full["screens"][0]["device"], "iPad");
    // The main reads the same list on its own loopback, and a bye removes it at once.
    let own = send(
        a.addr,
        "GET",
        "/api/lan/screens",
        &[("cookie", &cookie())],
        "",
    )
    .await
    .json();
    assert_eq!(own["screens"][0]["surface"], "desk");
    send(door, "POST", "/lan/screens/s1/bye", &h, "").await;
    let own = send(
        a.addr,
        "GET",
        "/api/lan/screens",
        &[("cookie", &cookie())],
        "",
    )
    .await
    .json();
    assert!(own["screens"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn the_relay_lists_a_peer_and_carries_http_and_the_engine_socket() {
    let (_a, door, engine) = open_device("relay-a", "A1", "tok-A").await;
    let b = device("relay-b", &format!("127.0.0.1:{}:{engine}", door.port())).await;
    let signed = [("cookie", cookie())];
    let h: Vec<(&str, &str)> = signed.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let peers = send(b.addr, "GET", "/api/lan/peers", &h, "").await.json();
    let one = &peers["peers"][0];
    assert_eq!(one["engineId"], "A1");
    assert!(one["reached"] == true && one["trusted"] == true, "{peers}");
    assert!(
        one.get("token").is_none(),
        "the page never holds the peer's token"
    );

    let hello = send(b.addr, "GET", "/api/lan/A1/hello", &h, "").await;
    assert_eq!(hello.status, 200, "{}", hello.body);
    assert_eq!(hello.json()["device"]["engineId"], "A1");
    // A forged token from the page is dropped; the relay's own is what the door sees.
    let mut forged = h.clone();
    forged.push(("x-k7-engine", "forged"));
    assert_eq!(
        send(b.addr, "GET", "/api/lan/A1/hello", &forged, "")
            .await
            .status,
        200
    );

    assert_eq!(
        send(b.addr, "GET", "/api/lan/nobody/hello", &h, "")
            .await
            .status,
        404
    );
    assert_eq!(
        send(b.addr, "GET", "/api/lan/A1/hello", &[], "")
            .await
            .status,
        401
    );

    let mut req = format!("ws://{}/api/lan/A1/engine", b.addr)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("cookie", cookie().parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req)
        .await
        .expect("the relay accepts the page");
    ws.send(Message::text("hello engine")).await.unwrap();
    let Some(Ok(Message::Text(t))) = ws.next().await else {
        panic!("no echo")
    };
    assert_eq!(t.as_str(), "echo:hello engine");
}

#[tokio::test]
async fn a_device_never_lists_itself_and_a_dead_peer_is_listed_unreached() {
    let (_a, door, engine) = open_device("self-a", "A1", "tok-A").await;
    // B has A's engine as its own and points at A's door too, so what it hears is itself.
    let b = device(
        "self-b",
        &format!("127.0.0.1:{}:{engine},127.0.0.1:1:1", door.port()),
    )
    .await;
    register(&b, "A1", "tok-A", engine).await;
    let h = [("cookie", cookie())];
    let peers = send(
        b.addr,
        "GET",
        "/api/lan/peers",
        &[("cookie", h[0].1.as_str())],
        "",
    )
    .await
    .json();
    let list = peers["peers"].as_array().unwrap();
    assert!(list.iter().all(|p| p["engineId"] != "A1"), "{peers}");
    assert!(list.iter().any(|p| p["reached"] == false), "{peers}");
}
