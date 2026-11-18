//! The remote cameras' hub the front owns: the list and the ticket behind the session, the socket behind the
//! ticket, and the signalling carried between a camera and the viewers of its own account.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

const PING: Duration = Duration::from_millis(300);

async fn front(name: &str) -> SocketAddr {
    let root = std::env::temp_dir().join(format!("k7-front-cameras-{name}-{}", std::process::id()));
    let gone = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    let env = [
        ("SESSION_SECRET", "test-secret".to_string()),
        (
            "STUDIO_AUTH_DIR",
            root.join("auth").to_str().unwrap().to_string(),
        ),
        ("K7_CAMERA_ICE_SERVERS", "stun:a:3478, stun:b".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: gone.to_string(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let limits = k7_front::CameraLimits {
        cameras: 2,
        viewers: 32,
        ping: PING,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve_with(
        config,
        limits,
        std::future::pending(),
        move |a| tx.send(a).unwrap(),
    ));
    rx.await.unwrap()
}

fn cookie(sub: &str) -> String {
    let user = json!({ "sub": sub, "email": "a@b.c" })
        .as_object()
        .unwrap()
        .clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    k7_auth::cookie_header(user, "studio.sid", "test-secret", now)
}

async fn http(addr: SocketAddr, method: &str, path: &str, sub: Option<&str>) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let c = sub
        .map(|s| format!("Cookie: {}\r\n", cookie(s)))
        .unwrap_or_default();
    let raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n{c}\r\n"
    );
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let status = out[9..12].parse().unwrap();
    let body = out
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

async fn list(addr: SocketAddr, sub: &str) -> Vec<String> {
    let (_, body) = http(addr, "GET", "/api/cameras", Some(sub)).await;
    let v: Value = serde_json::from_str(&body).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

async fn ticket(addr: SocketAddr, sub: &str) -> String {
    let (status, body) = http(addr, "POST", "/api/cameras/ticket", Some(sub)).await;
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    v["ticket"].as_str().unwrap().to_string()
}

async fn open(addr: SocketAddr, query: &str) -> Result<Ws, u16> {
    match tokio_tungstenite::connect_async(format!("ws://{addr}/api/cameras/ws?{query}")).await {
        Ok((ws, _)) => Ok(ws),
        Err(WsError::Http(r)) => Err(r.status().as_u16()),
        Err(e) => panic!("{e}"),
    }
}

async fn connect(addr: SocketAddr, sub: &str, query: &str) -> Ws {
    open(addr, &format!("ticket={}&{query}", ticket(addr, sub).await))
        .await
        .unwrap()
}

/// The next text frame, as JSON; a close frame reads as `{"close": code}`.
async fn next(ws: &mut Ws) -> Value {
    loop {
        let got = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("a frame in time");
        match got {
            Some(Ok(Message::Text(t))) => return serde_json::from_str(t.as_str()).unwrap(),
            Some(Ok(Message::Close(c))) => return json!({ "close": c.map(|c| u16::from(c.code)) }),
            Some(Ok(_)) => continue,
            other => return json!({ "ended": format!("{other:?}") }),
        }
    }
}

async fn say(ws: &mut Ws, frame: Value) {
    ws.send(Message::text(frame.to_string())).await.unwrap();
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test]
async fn the_list_and_the_ticket_sit_behind_the_session() {
    let addr = front("session").await;
    assert_eq!(http(addr, "GET", "/api/cameras", None).await.0, 401);
    assert_eq!(http(addr, "POST", "/api/cameras/ticket", None).await.0, 401);
    assert_eq!(
        http(addr, "GET", "/api/cameras", Some("ana")).await,
        (200, "[]".to_string())
    );
    let (_, body) = http(addr, "POST", "/api/cameras/ticket", Some("ana")).await;
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["iceServers"],
        json!([{ "urls": "stun:a:3478" }, { "urls": "stun:b" }])
    );
}

#[tokio::test]
async fn an_upgrade_is_refused_without_a_ticket_a_role_or_room() {
    let addr = front("deny").await;
    assert_eq!(open(addr, "role=viewer").await.err(), Some(401));
    assert_eq!(
        open(addr, "ticket=forged.x&role=viewer").await.err(),
        Some(401)
    );
    let t = ticket(addr, "ana").await;
    assert_eq!(
        open(addr, &format!("ticket={t}&role=boss")).await.err(),
        Some(400)
    );
    let mut a = connect(addr, "ana", "role=camera&name=iPad").await;
    assert_eq!(
        next(&mut a).await,
        json!({ "type": "hello", "name": "iPad" })
    );
    let mut b = connect(addr, "ana", "role=camera&name=ipad").await;
    assert_eq!(
        next(&mut b).await,
        json!({ "type": "hello", "name": "ipad (2)" })
    );
    assert_eq!(list(addr, "ana").await, ["iPad", "ipad (2)"]);
    assert!(list(addr, "bob").await.is_empty());
    assert_eq!(
        open(addr, &format!("ticket={t}&role=camera&name=x"))
            .await
            .err(),
        Some(429)
    );
    a.close(None).await.unwrap();
    b.close(None).await.unwrap();
    settle().await;
    assert!(list(addr, "ana").await.is_empty());
}

#[tokio::test]
async fn a_viewers_negotiation_reaches_the_camera_it_named_and_back() {
    let addr = front("relay").await;
    let mut cam = connect(addr, "ana", "role=camera&name=Stage%20%20left").await;
    assert_eq!(next(&mut cam).await["name"], "Stage left");
    let mut viewer = connect(addr, "ana", "role=viewer").await;
    say(
        &mut viewer,
        json!({ "type": "want", "label": "remote:stage" }),
    )
    .await;
    let want = next(&mut cam).await;
    assert_eq!(
        (want["type"].as_str(), want["label"].as_str()),
        (Some("want"), Some("remote:stage"))
    );
    let from = want["from"].as_str().unwrap().to_string();
    assert_eq!(
        next(&mut cam).await,
        json!({ "type": "viewers", "count": 1 })
    );
    say(
        &mut cam,
        json!({ "type": "offer", "to": from, "sdp": { "type": "offer", "sdp": "o" } }),
    )
    .await;
    assert_eq!(
        next(&mut viewer).await,
        json!({ "type": "offer", "sdp": { "type": "offer", "sdp": "o" }, "problem": null })
    );
    say(
        &mut cam,
        json!({ "type": "ice", "to": "someone-else", "candidate": {} }),
    )
    .await;
    say(
        &mut viewer,
        json!({ "type": "answer", "sdp": { "type": "answer", "sdp": "a" } }),
    )
    .await;
    assert_eq!(
        next(&mut cam).await,
        json!({ "type": "answer", "sdp": { "type": "answer", "sdp": "a" }, "from": from })
    );
    say(
        &mut viewer,
        json!({ "type": "ice", "candidate": { "sdpMLineIndex": 0 } }),
    )
    .await;
    assert_eq!(
        next(&mut cam).await,
        json!({ "type": "ice", "candidate": { "sdpMLineIndex": 0 }, "from": from })
    );
    viewer.close(None).await.unwrap();
    assert_eq!(
        next(&mut cam).await,
        json!({ "type": "gone", "from": from })
    );
    assert_eq!(
        next(&mut cam).await,
        json!({ "type": "viewers", "count": 0 })
    );
}

#[tokio::test]
async fn a_camera_going_off_closes_its_viewers_and_another_accounts_is_never_paired() {
    let addr = front("off").await;
    let mut bob = connect(addr, "bob", "role=camera&name=Bob%20cam").await;
    next(&mut bob).await;
    let mut viewer = connect(addr, "ana", "role=viewer").await;
    say(
        &mut viewer,
        json!({ "type": "want", "label": "remote:Bob cam" }),
    )
    .await;
    assert_eq!(
        next(&mut viewer).await,
        json!({ "type": "problem", "text": "no remote camera is on" })
    );
    let mut cam = connect(addr, "ana", "role=camera&name=Drums").await;
    next(&mut cam).await;
    say(
        &mut viewer,
        json!({ "type": "want", "label": "remote:Keys" }),
    )
    .await;
    let from = next(&mut cam).await["from"].as_str().unwrap().to_string();
    next(&mut cam).await;
    say(
        &mut cam,
        json!({ "type": "offer", "to": from, "sdp": { "type": "offer" } }),
    )
    .await;
    let offer = next(&mut viewer).await;
    assert!(
        offer["problem"].as_str().unwrap().contains("\"Keys\""),
        "{offer}"
    );
    cam.close(None).await.unwrap();
    assert_eq!(next(&mut viewer).await, json!({ "close": 1001 }));
    settle().await;
    assert!(list(addr, "ana").await.is_empty());
    assert_eq!(list(addr, "bob").await, ["Bob cam"]);
}

#[tokio::test]
async fn a_socket_that_stops_answering_pings_is_dropped() {
    let addr = front("ping").await;
    let t = ticket(addr, "ana").await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    let raw = format!(
        "GET /api/cameras/ws?ticket={t}&role=camera&name=mute HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
    );
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut head = [0u8; 12];
    s.read_exact(&mut head).await.unwrap();
    assert_eq!(&head, b"HTTP/1.1 101");
    settle().await;
    assert_eq!(list(addr, "ana").await, ["mute"]);
    tokio::time::sleep(PING * 3).await;
    assert!(list(addr, "ana").await.is_empty());
}

#[tokio::test]
async fn a_frame_past_the_cap_drops_its_socket() {
    let addr = front("cap").await;
    let mut cam = connect(addr, "ana", "role=camera&name=big").await;
    next(&mut cam).await;
    let _ = cam.send(Message::text("x".repeat(64 * 1024 + 1))).await;
    settle().await;
    assert!(list(addr, "ana").await.is_empty());
}

#[tokio::test]
async fn a_request_to_the_socket_that_is_no_upgrade_goes_to_the_backend() {
    let addr = front("plain").await;
    assert_eq!(
        http(addr, "GET", "/api/cameras/ws", Some("ana")).await.0,
        502
    );
}
