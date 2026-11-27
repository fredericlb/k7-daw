//! hub_lifecycle.rs — a tunnel over its lifetime, on real sockets: the agent dialling a hub, a fake engine behind it,
//! and local clients on bound paths. A drop ends every channel the tunnel carried and the agent's redial serves new
//! ones; the newest verified tunnel wins; a refused one changes nothing; a bound path answers 503 with no tunnel and
//! 403 off loopback.

use std::sync::atomic::Ordering;
use std::time::Duration;

use k7_supervisor::tunnel::TunnelAgent;
use serde_json::json;

use crate::bed::{
    AgentSpec, Client, Dir, FakeEngine, Hub, HubOptions, TUNNEL_PATH, agent, lan_address, until,
    until_async,
};

const TOKEN: &str = "tok";
const SECRET: &str = "sssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssss";

struct Rig {
    hub: Hub,
    cache: Dir,
}

async fn rig() -> Rig {
    let hub = Hub::start(HubOptions {
        verify: Some(("e1", TOKEN, Some(SECRET))),
        ..Default::default()
    })
    .await;
    Rig {
        hub,
        cache: Dir::new("hub-lifecycle"),
    }
}

impl Rig {
    /// The machine's agent for e1, in front of a fresh fake engine.
    async fn start_agent(&self) -> (TunnelAgent, FakeEngine) {
        let engine = FakeEngine::start().await;
        let (a, _) = agent(AgentSpec {
            url: &self.hub.agent_url(),
            id: "e1",
            token: TOKEN,
            secret: SECRET,
            engine_port: engine.port,
            cache: self.cache.0.clone(),
            protocol: None,
        });
        (a, engine)
    }

    async fn channel(&self) -> Client {
        self.hub
            .open_ready("e1", json!([]), "engine")
            .await
            .expect("a channel")
    }

    async fn local(&self, path: &str, from: Option<std::net::Ipv4Addr>) -> Result<Client, u16> {
        Client::dial(self.hub.port(), path, from).await
    }
}

#[tokio::test]
async fn ends_every_channel_of_a_tunnel_that_dropped_and_serves_new_ones_once_the_agent_redialled()
{
    let r = rig().await;
    let (_agent, engine) = r.start_agent().await;
    r.hub.until_has("e1").await;
    let before = r.channel().await;
    before.send("one");
    until(|| engine.got().len() == 1, 10_000).await;
    r.hub.relay.cut();
    before.until_closed(10_000).await;
    until(|| engine.peer_closed(0), 10_000).await;
    assert!(!r.hub.has("e1").await);
    assert!(r.hub.open_ready("e1", json!([]), "engine").await.is_none());
    r.hub.until_has("e1").await;
    before.send("lost");
    let after = r.channel().await;
    after.send("two");
    until(|| engine.got().len() == 2, 10_000).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(engine.got(), ["one", "two"]);
}

#[tokio::test]
async fn closes_a_local_client_when_its_tunnel_drops_and_serves_the_same_bound_path_again_after_the_redial()
 {
    let r = rig().await;
    let (_agent, engine) = r.start_agent().await;
    r.hub.until_has("e1").await;
    let path = r.hub.bind_local("e1", json!([]), "engine", false).await;
    let first = r.local(&path, None).await.expect("open");
    r.hub.relay.cut();
    first.until_closed(10_000).await;
    assert_eq!(r.local(&path, None).await.err(), Some(503));
    r.hub.until_has("e1").await;
    let again = r.local(&path, None).await.expect("open again");
    again.send("back");
    until(|| engine.got().iter().any(|g| g == "back"), 10_000).await;
}

#[tokio::test]
async fn lets_the_newest_verified_tunnel_win_the_incumbents_channels_end_new_ones_reach_the_newcomers_engine()
 {
    let r = rig().await;
    let (a, engine_a) = r.start_agent().await;
    r.hub.until_has("e1").await;
    let old = r.channel().await;
    let (b, engine_b) = r.start_agent().await;
    old.until_closed(10_000).await;
    // The displaced agent would redial and win back in turn; a real machine runs one supervisor.
    a.stop();
    until_async(|| async { r.hub.has("e1").await && b.connected() }, 10_000).await;
    let fresh = r.channel().await;
    fresh.send("to-b");
    until(|| engine_b.got().len() == 1, 10_000).await;
    assert!(engine_a.got().is_empty());
}

#[tokio::test]
async fn refuses_a_tunnel_with_another_secret_and_leaves_the_incumbents_channels_working() {
    let r = rig().await;
    let (_agent, engine) = r.start_agent().await;
    r.hub.until_has("e1").await;
    let ch = r.channel().await;
    let other = "o".repeat(64);
    let refused = Client::dial_with(
        r.hub.port(),
        &format!("{TUNNEL_PATH}?id=e1"),
        None,
        &[
            ("x-studio-engine-token", TOKEN),
            ("x-studio-tunnel-secret", &other),
        ],
    )
    .await;
    assert_eq!(refused.err(), Some(403));
    ch.send("still");
    until(|| engine.got().iter().any(|g| g == "still"), 10_000).await;
    assert!(!ch.closed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn answers_503_on_a_bound_path_while_its_engine_has_no_tunnel_and_403_on_a_key_never_bound() {
    let r = rig().await;
    let path = r.hub.bind_local("e1", json!([]), "engine", false).await;
    assert_eq!(r.local(&path, None).await.err(), Some(503));
    assert_eq!(
        r.local(&format!("{TUNNEL_PATH}/local/never-bound"), None)
            .await
            .err(),
        Some(403)
    );
    let (_agent, _engine) = r.start_agent().await;
    r.hub.until_has("e1").await;
    assert!(r.local(&path, None).await.is_ok());
}

#[tokio::test]
async fn answers_403_on_a_bound_path_to_a_peer_that_is_not_loopback_even_with_the_tunnel_up() {
    // Set where a firewall drops a LAN peer's connection to the unsigned testbed (docs/release.md, macOS).
    let Some(lan) =
        lan_address().filter(|_| std::env::var("K7_TEST_NO_LAN_PEER").as_deref() != Ok("1"))
    else {
        return;
    };
    let r = rig().await;
    let (_agent, _engine) = r.start_agent().await;
    r.hub.until_has("e1").await;
    let path = r.hub.bind_local("e1", json!([]), "engine", false).await;
    // A firewall that holds the LAN connection (an unsigned binary's prompt) is no answer: the case is skipped, not hung.
    let Ok(from_lan) =
        tokio::time::timeout(Duration::from_secs(5), r.local(&path, Some(lan))).await
    else {
        eprintln!("skipped: the LAN connection to {lan} got no answer in 5 s (a firewall prompt?)");
        return;
    };
    assert_eq!(from_lan.err(), Some(403));
    assert!(r.local(&path, None).await.is_ok());
}

#[tokio::test]
async fn answers_403_on_a_path_whose_binding_was_undone() {
    let r = rig().await;
    let (_agent, _engine) = r.start_agent().await;
    r.hub.until_has("e1").await;
    let path = r.hub.bind_local("e1", json!([]), "engine", false).await;
    r.hub.unbind_local(&path).await;
    assert_eq!(r.local(&path, None).await.err(), Some(403));
}
