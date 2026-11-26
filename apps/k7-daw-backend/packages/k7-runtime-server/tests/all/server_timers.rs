//! server_timers.rs — the socket's clocks, shortened: a ping each interval, a peer that answers none closed,
//! and a send to a peer that stopped reading ended at the send timeout instead of waiting for it for ever.

use crate::harness::{Server, eventually};
use futures_util::StreamExt;
use k7_runtime_server::server::ServeConfig;
use serde_json::json;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

fn cfg(ping_ms: u64, send_ms: u64) -> ServeConfig {
    let mut c = ServeConfig::new(0);
    c.timers.ping_interval = Duration::from_millis(ping_ms);
    c.timers.send_timeout = Duration::from_millis(send_ms);
    c
}

#[tokio::test]
async fn a_ping_arrives_each_interval_and_an_answering_peer_stays() {
    let s = Server::start(cfg(150, 2000));
    let mut w = s.connect().await;
    let mut pings = 0;
    // Reading is what answers: the client library pongs as it polls.
    while pings < 4 {
        match tokio::time::timeout(Duration::from_secs(2), w.0.next()).await {
            Ok(Some(Ok(Message::Ping(_)))) => pings += 1,
            other => panic!("expected a ping, got {other:?}"),
        }
    }
    assert_eq!(s.ctx.client_count(), 1);
}

#[tokio::test]
async fn a_peer_that_answers_no_ping_is_closed() {
    let s = Server::start(cfg(150, 2000));
    // Connected and never polled again: nothing answers the pings.
    let w = s.connect().await;
    eventually("one client", || s.ctx.client_count() == 1).await;
    eventually("the silent peer is dropped", || s.ctx.client_count() == 0).await;
    drop(w);
}

#[tokio::test]
async fn a_send_to_a_peer_that_stopped_reading_times_out_and_drops_only_that_socket() {
    let s = Server::start(cfg(60_000, 1000));
    let mut stuck = s.connect().await;
    let mut fine = s.connect().await;
    stuck.ask(r#"{"type":"whoami"}"#).await;
    fine.ask(r#"{"type":"whoami"}"#).await;
    // Far more than a socket's buffers hold, and `stuck` reads none of it.
    let big = json!({"type": "plugins", "pad": "x".repeat(1024 * 1024)});
    let drain = tokio::spawn(async move {
        for _ in 0..24 {
            assert!(fine.text().await.contains("\"pad\""));
        }
        fine
    });
    for _ in 0..24 {
        s.ctx.broadcast(&big);
    }
    eventually("the stuck socket is dropped", || s.ctx.client_count() == 1).await;
    // The other socket was behind it in no queue of its own: it heard everything and still works.
    let mut fine = drain.await.unwrap();
    assert!(fine.ask(r#"{"type":"whoami"}"#).await.contains("whoami"));
}
