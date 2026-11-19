//! engines_tests.rs — a fake supervisor's end of a tunnel served by an in-process hub, and what a `Link` does over it;
//! the helpers are the other families' tests' too (`job.rs`).

use super::engines::*;
use futures_util::{SinkExt, StreamExt};
use k7_engine::tunnel::frames::{Kind, Root};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::Role;

/// A fake supervisor's end of a tunnel served by `core`'s hub, for engine `id`.
pub async fn agent(
    core: &Arc<k7_engine::Core>,
    id: &str,
    renders: bool,
) -> WebSocketStream<tokio::io::DuplexStream> {
    let (a, b) = tokio::io::duplex(1 << 20);
    let server = WebSocketStream::from_raw_socket(a, Role::Server, None).await;
    let client = WebSocketStream::from_raw_socket(b, Role::Client, None).await;
    tokio::spawn(
        core.hub
            .clone()
            .serve_tunnel(id.to_string(), renders, server),
    );
    for _ in 0..100 {
        if core.hub.has(id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    client
}

/// The next control message the agent reads.
pub async fn read(agent: &mut WebSocketStream<tokio::io::DuplexStream>) -> Value {
    loop {
        match agent.next().await {
            Some(Ok(Message::Text(t))) => return serde_json::from_str(&t).unwrap(),
            Some(Ok(_)) => {}
            other => panic!("the tunnel ended: {other:?}"),
        }
    }
}

pub async fn say(agent: &mut WebSocketStream<tokio::io::DuplexStream>, v: Value) {
    agent
        .send(Message::Text(v.to_string().into()))
        .await
        .unwrap();
}

pub fn core() -> Arc<k7_engine::Core> {
    k7_engine::Core::new(
        k7_engine::CoreOptions {
            render_key: Some("rk".into()),
            ..Default::default()
        },
        k7_engine::Clock::default(),
    )
}

#[tokio::test]
async fn an_in_process_link_holds_what_is_sent_until_the_engine_answers() {
    let core = core();
    let mut agent = agent(&core, "e1", true).await;
    let reach = Reach(&core);
    let roots = vec![Root {
        remote: "/srv/lib".into(),
        name: "lib".into(),
        writable: false,
        per_child: None,
    }];
    let mut link = reach
        .open(None, roots, Kind::Engine, Duration::from_secs(1))
        .await
        .unwrap()
        .expect("the render machine");
    link.tx.text(r#"{"type":"hello"}"#.into());
    let open = read(&mut agent).await;
    assert_eq!(open["t"], "open");
    assert_eq!(open["kind"], "engine");
    let ch = open["ch"].clone();
    say(&mut agent, json!({ "t": "opened", "ch": ch })).await;
    assert_eq!(
        read(&mut agent).await,
        json!({ "t": "data", "ch": ch, "text": r#"{"type":"hello"}"# })
    );
    say(
        &mut agent,
        json!({ "t": "data", "ch": ch, "text": r#"{"type":"state"}"# }),
    )
    .await;
    match link.rx.recv().await {
        Some(LinkEvent::Frame(f)) => assert_eq!(&f.data[..], br#"{"type":"state"}"#),
        other => panic!("{other:?}"),
    }
    link.tx.close();
    assert_eq!(read(&mut agent).await, json!({ "t": "close", "ch": ch }));
    assert!(
        reach
            .open(Some("nobody"), vec![], Kind::Jobs, Duration::from_secs(1))
            .await
            .unwrap()
            .is_none()
    );
}
