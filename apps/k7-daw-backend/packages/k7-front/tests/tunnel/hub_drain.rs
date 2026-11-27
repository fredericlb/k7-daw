//! hub_drain.rs — a channel's close delivers what was queued ahead of it: the hub's last frames reach the engine, in
//! order, and the engine's last frames reach the hub. A stuck engine still lets the channel go within the bound
//! (`CLOSE_DRAIN`), and a loopback client gone before its channel opened has its frames delivered too.

use std::time::{Duration, Instant};

use k7_supervisor::tunnel::TunnelAgent;
use k7_supervisor::tunnel::wire::CLOSE_DRAIN;
use serde_json::{Value, json};

use crate::bed::{AgentSpec, Dir, FakeEngine, Hub, HubOptions, StuckEngine, agent, until};

const TOKEN: &str = "tok";

async fn start_agent(engine_port: u16, cache: &Dir) -> (Hub, TunnelAgent) {
    let hub = Hub::start(HubOptions {
        verify: Some(("e1", TOKEN, None)),
        ..Default::default()
    })
    .await;
    let (a, _) = agent(AgentSpec {
        url: &hub.agent_url(),
        id: "e1",
        token: TOKEN,
        secret: "",
        engine_port,
        cache: cache.0.clone(),
        protocol: None,
    });
    hub.until_has("e1").await;
    (hub, a)
}

fn within() -> u64 {
    CLOSE_DRAIN.as_millis() as u64 + 2000
}

fn types(frames: &[String]) -> Vec<String> {
    frames
        .iter()
        .map(|g| {
            serde_json::from_str::<Value>(g).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn delivers_the_hubs_last_frames_to_the_engine_in_order_before_closing_it() {
    let (remote, cache) = (Dir::new("drain-remote"), Dir::new("drain-cache"));
    std::fs::create_dir_all(remote.join("song")).unwrap();
    std::fs::write(remote.join("song/a.wav"), "RIFF-a".repeat(50_000)).unwrap();
    let engine = FakeEngine::start().await;
    let (hub, _agent) = start_agent(engine.port, &cache).await;
    let roots = json!([{ "remote": remote.s(), "name": "ws", "writable": false }]);
    let ch = hub
        .open_ready("e1", roots, "engine")
        .await
        .expect("a channel");
    // The first frame names a file, so it waits on a download while the rest queue behind it.
    ch.send(
        &json!({ "type": "project", "file": format!("{}/song/a.wav", remote.s()) }).to_string(),
    );
    ch.send(&json!({ "type": "transport", "action": "stop" }).to_string());
    ch.send(&json!({ "type": "drop-session" }).to_string());
    ch.close();
    until(|| engine.closed_with.lock().unwrap().is_some(), within()).await;
    assert_eq!(
        types(&engine.got()),
        ["project", "transport", "drop-session"]
    );
    assert_eq!(*engine.closed_with.lock().unwrap(), Some(3));
}

#[tokio::test]
async fn delivers_the_engines_last_frames_to_the_hub_before_the_channels_close() {
    let (remote, cache) = (Dir::new("drain-remote"), Dir::new("drain-cache"));
    let engine = FakeEngine::start().await;
    let (hub, _agent) = start_agent(engine.port, &cache).await;
    let roots = json!([{ "remote": remote.s(), "name": "ws", "writable": true }]);
    let ch = hub
        .open_ready("e1", roots, "engine")
        .await
        .expect("a channel");
    until(|| !engine.peers.lock().unwrap().is_empty(), 10_000).await;
    let local = cache.join("ws/take.wav");
    std::fs::create_dir_all(cache.join("ws")).unwrap();
    std::fs::write(&local, "RIFF-take".repeat(50_000)).unwrap();
    // The first frame names a file the engine wrote, so it waits on an upload while the engine closes.
    engine.say(&json!({ "type": "recorded", "file": local.to_string_lossy() }).to_string());
    engine.say(&json!({ "type": "stopped" }).to_string());
    engine.hang_up();
    ch.until_closed(within()).await;
    let got = ch.heard();
    assert_eq!(types(&got), ["recorded", "stopped"]);
    assert_eq!(
        serde_json::from_str::<Value>(&got[0]).unwrap()["file"],
        format!("{}/take.wav", remote.s())
    );
}

#[tokio::test]
async fn delivers_what_a_loopback_client_sent_before_its_channel_opened_then_closes() {
    let cache = Dir::new("drain-cache");
    let engine = FakeEngine::start().await;
    let (hub, _agent) = start_agent(engine.port, &cache).await;
    let path = hub.bind_local("e1", json!([]), "engine", false).await;
    let client = crate::bed::Client::dial(hub.port(), &path, None)
        .await
        .unwrap();
    client.send(&json!({ "type": "transport", "action": "stop" }).to_string());
    client.send(&json!({ "type": "drop-session" }).to_string());
    client.close();
    until(|| engine.closed_with.lock().unwrap().is_some(), within()).await;
    assert_eq!(types(&engine.got()), ["transport", "drop-session"]);
    assert_eq!(*engine.closed_with.lock().unwrap(), Some(2));
}

#[tokio::test]
async fn lets_go_of_an_engine_that_never_answers_the_close_within_the_bound() {
    let cache = Dir::new("drain-cache");
    let engine = StuckEngine::start().await;
    let (hub, _agent) = start_agent(engine.port, &cache).await;
    let ch = hub
        .open_ready("e1", json!([]), "engine")
        .await
        .expect("a channel");
    ch.send(&json!({ "type": "transport", "action": "stop" }).to_string());
    ch.send(&json!({ "type": "drop-session" }).to_string());
    let closed_at = Instant::now();
    ch.close();
    until(
        || engine.end_at.lock().unwrap().is_some(),
        CLOSE_DRAIN.as_millis() as u64 + 3000,
    )
    .await;
    assert_eq!(engine.types(), ["transport", "drop-session"]);
    assert!(engine.close_at.lock().unwrap().is_some());
    let end = engine.end_at.lock().unwrap().unwrap();
    assert!(end - closed_at < CLOSE_DRAIN + Duration::from_secs(1));
}
