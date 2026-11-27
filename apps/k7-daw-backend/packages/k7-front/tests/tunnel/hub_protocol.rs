//! hub_protocol.rs — a supervisor states the engine wire's version on the upgrade and a hub refuses one below its
//! minimum with a 426 whose body says to update; a socket that sends a protocol fault is ended, and a peer that leaves
//! pings unanswered is dropped (`apps/k7-daw-backend/docs/engine.md`, "The testbed").

use std::time::Duration;

use futures_util::StreamExt;
use k7_supervisor::tunnel::TunnelAgent;
use tokio::net::TcpStream;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::bed::{AgentSpec, Dir, Hub, HubOptions, TUNNEL_PATH, agent, send_bad_frame, upgrade};

async fn listen(min: Option<u32>) -> Hub {
    Hub::start(HubOptions {
        min_protocol: min,
        ..Default::default()
    })
    .await
}

fn dial(
    hub: &Hub,
    id: &str,
    protocol: i64,
    cache: &Dir,
) -> (TunnelAgent, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    agent(AgentSpec {
        url: &hub.agent_url(),
        id,
        token: "t",
        secret: "",
        engine_port: 1,
        cache: cache.0.clone(),
        protocol: Some(protocol),
    })
}

#[tokio::test]
async fn serves_an_agent_at_the_minimum() {
    let hub = listen(Some(2)).await;
    let cache = Dir::new("protocol");
    let _a = dial(&hub, "current", 2, &cache);
    hub.until_has("current").await;
}

#[tokio::test]
async fn refuses_an_agent_below_it_and_says_so_once_however_often_it_redials() {
    let hub = listen(Some(2)).await;
    let cache = Dir::new("protocol");
    let _old = dial(&hub, "old", 1, &cache);
    let _current = dial(&hub, "current", 2, &cache);
    hub.until_has("current").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!hub.has("old").await);
    let said: Vec<String> = hub
        .lines()
        .into_iter()
        .filter(|l| l.starts_with("old refused"))
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("update k7 on this machine"), "{said:?}");
}

#[tokio::test]
async fn answers_426_with_the_reason_in_the_body() {
    let hub = listen(Some(3)).await;
    let (status, body) = upgrade(hub.port(), &[("x-studio-engine-protocol", "2")]).await;
    assert_eq!(status, 426);
    assert!(body.contains("protocol 2"), "{body}");
    assert!(body.contains("needs 3"), "{body}");
}

#[tokio::test]
async fn reads_a_supervisor_that_states_no_protocol_as_protocol_1() {
    let hub = listen(Some(2)).await;
    let (status, body) = upgrade(hub.port(), &[]).await;
    assert_eq!(status, 426);
    assert!(body.contains("protocol 1"), "{body}");
    hub.stop().await;
    let hub = listen(None).await;
    assert_eq!(upgrade(hub.port(), &[]).await.0, 101);
}

#[tokio::test]
async fn ends_a_socket_that_sent_an_rsv1_frame_even_one_already_closing() {
    let hub = listen(None).await;
    let status = send_bad_frame(
        hub.port(),
        &format!("{TUNNEL_PATH}?id=bad"),
        &[("x-studio-engine-token", "t")],
    )
    .await;
    assert!(status.contains("101"), "{status}");
    // The fault ends the tunnel: nothing of `bad` stays listed.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!hub.has("bad").await);
}

/// A client of the tunnel path that reads its socket only while `answer` is true: tungstenite answers a ping when polled.
async fn tunnel_client(port: u16, id: &str) -> tokio_tungstenite::WebSocketStream<TcpStream> {
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut req = format!("ws://127.0.0.1:{port}{TUNNEL_PATH}?id={id}")
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("x-studio-engine-token", "t".parse().unwrap());
    client_async(req, stream).await.unwrap().0
}

#[tokio::test]
async fn drops_a_peer_that_leaves_a_ping_unanswered() {
    let hub = Hub::start(HubOptions {
        ping_ms: Some(100),
        ..Default::default()
    })
    .await;
    // Never polled, so no pong goes back.
    let _mute = tunnel_client(hub.port(), "mute").await;
    crate::bed::until_async(|| async { hub.has("mute").await }, 5000).await;
    crate::bed::until_async(|| async { !hub.has("mute").await }, 5000).await;
}

#[tokio::test]
async fn keeps_a_peer_that_answers() {
    let hub = Hub::start(HubOptions {
        ping_ms: Some(100),
        ..Default::default()
    })
    .await;
    let mut talker = tunnel_client(hub.port(), "talker").await;
    let reading = tokio::spawn(async move { while talker.next().await.is_some() {} });
    hub.until_has("talker").await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(hub.has("talker").await);
    reading.abort();
}
