//! hub_tunnel.rs — the tunnel end to end on real sockets: the hub, the machine's agent dialling it and a fake engine.
//! What it pins is the contract of `docs/tunnel.md`: the engine only ever sees local paths, a file is present BEFORE
//! the frame naming it arrives, and a file the engine wrote is on the server BEFORE the frame naming it does.

use k7_supervisor::tunnel::TunnelAgent;
use serde_json::{Value, json};

use crate::bed::{AgentSpec, Client, Dir, FakeEngine, Hub, HubOptions, TUNNEL_PATH, agent, until};

struct Rig {
    hub: Hub,
    engine: FakeEngine,
    _agent: TunnelAgent,
    remote: Dir,
    cache: Dir,
}

async fn rig() -> Rig {
    let (remote, cache) = (Dir::new("remote"), Dir::new("cache"));
    let engine = FakeEngine::start().await;
    let hub = Hub::start(HubOptions {
        verify: Some(("e1", "tok", None)),
        ..Default::default()
    })
    .await;
    let (agent, _) = agent(AgentSpec {
        url: &hub.agent_url(),
        id: "e1",
        token: "tok",
        secret: "",
        engine_port: engine.port,
        cache: cache.0.clone(),
        protocol: None,
    });
    hub.until_has("e1").await;
    Rig {
        hub,
        engine,
        _agent: agent,
        remote,
        cache,
    }
}

impl Rig {
    async fn open_shared(&self) -> Client {
        let roots = json!([{ "remote": self.remote.s(), "name": "ws", "writable": true }]);
        self.hub
            .open_ready("e1", roots, "engine")
            .await
            .expect("a channel")
    }

    /// The `n`th frame the engine heard, parsed.
    async fn frame(&self, n: usize) -> Value {
        until(|| self.engine.got().len() > n, 10_000).await;
        serde_json::from_str(&self.engine.got()[n]).unwrap()
    }
}

#[tokio::test]
async fn refuses_a_supervisor_with_the_wrong_token() {
    let r = rig().await;
    let status = Client::dial_with(
        r.hub.port(),
        &format!("{TUNNEL_PATH}?id=e1"),
        None,
        &[("x-studio-engine-token", "nope")],
    )
    .await
    .err();
    assert_eq!(status, Some(403));
}

#[tokio::test]
async fn passes_frames_untouched_on_a_channel_that_shares_nothing() {
    let r = rig().await;
    let ch = r.hub.open_ready("e1", json!([]), "engine").await.unwrap();
    let file = format!("{}/a.wav", r.remote.s());
    ch.send(&json!({ "type": "transport", "action": "play", "file": file }).to_string());
    assert_eq!(r.frame(0).await["file"], file);
}

#[tokio::test]
async fn downloads_a_file_before_the_frame_naming_it_reaches_the_engine() {
    let r = rig().await;
    std::fs::create_dir_all(r.remote.join("song/audio")).unwrap();
    std::fs::write(r.remote.join("song/audio/kick.wav"), "RIFF-kick").unwrap();
    let ch = r.open_shared().await;
    let remote = r.remote.s();
    ch.send(
        &json!({
            "type": "project",
            "tracks": [{ "clips": [{ "file": format!("{remote}/song/audio/kick.wav") }] }],
            "recordDir": format!("{remote}/song/clips"),
        })
        .to_string(),
    );
    let doc = r.frame(0).await;
    let local = doc["tracks"][0]["clips"][0]["file"].as_str().unwrap();
    assert_eq!(
        local,
        r.cache.join("ws/song/audio/kick.wav").to_string_lossy()
    );
    assert_eq!(std::fs::read_to_string(local).unwrap(), "RIFF-kick");
    assert_eq!(
        doc["recordDir"].as_str().unwrap(),
        r.cache.join("ws/song/clips").to_string_lossy()
    );
}

#[tokio::test]
async fn uploads_a_file_the_engine_wrote_before_the_frame_naming_it_reaches_the_server() {
    let r = rig().await;
    let ch = r.open_shared().await;
    ch.send(
        &json!({ "type": "project", "recordDir": format!("{}/song/clips", r.remote.s()) })
            .to_string(),
    );
    let local_dir = r.frame(0).await["recordDir"].as_str().unwrap().to_string();
    std::fs::create_dir_all(&local_dir).unwrap();
    let take = format!("{local_dir}/take-1.wav");
    std::fs::write(&take, "RIFF-take").unwrap();
    r.engine
        .say(&json!({ "type": "recorded", "file": take }).to_string());
    until(|| ch.heard().len() == 1, 10_000).await;
    let file = serde_json::from_str::<Value>(&ch.heard()[0]).unwrap()["file"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(file, format!("{}/song/clips/take-1.wav", r.remote.s()));
    assert_eq!(std::fs::read_to_string(file).unwrap(), "RIFF-take");
}

#[tokio::test]
async fn moves_files_larger_than_one_chunk_both_ways_byte_for_byte() {
    let r = rig().await;
    let mut big = vec![0u8; 1_500_000];
    getrandom::fill(&mut big).unwrap();
    std::fs::write(r.remote.join("big.wav"), &big).unwrap();
    let ch = r.open_shared().await;
    ch.send(&json!({ "type": "project", "file": format!("{}/big.wav", r.remote.s()) }).to_string());
    let local = r.frame(0).await["file"].as_str().unwrap().to_string();
    assert!(std::fs::read(&local).unwrap() == big);
    let mut back = vec![0u8; 700_000];
    getrandom::fill(&mut back).unwrap();
    let up = r.cache.join("ws/back.wav");
    std::fs::write(&up, &back).unwrap();
    r.engine
        .say(&json!({ "type": "recorded", "file": up.to_string_lossy() }).to_string());
    until(|| ch.heard().len() == 1, 10_000).await;
    assert!(std::fs::read(r.remote.join("back.wav")).unwrap() == back);
}

#[tokio::test]
async fn does_not_fetch_a_file_again_that_it_already_holds() {
    let r = rig().await;
    std::fs::write(r.remote.join("a.wav"), "one").unwrap();
    let ch = r.open_shared().await;
    let file = format!("{}/a.wav", r.remote.s());
    ch.send(&json!({ "type": "project", "file": file }).to_string());
    let local = r.frame(0).await["file"].as_str().unwrap().to_string();
    // A re-download would restore "one".
    std::fs::write(&local, "tampered-but-same-len").unwrap();
    ch.send(&json!({ "type": "tracks", "file": file }).to_string());
    r.frame(1).await;
    assert_eq!(
        std::fs::read_to_string(&local).unwrap(),
        "tampered-but-same-len"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn serves_nothing_through_a_symlink_that_leaves_the_shared_tree() {
    let r = rig().await;
    let outside = Dir::new("outside");
    std::fs::write(outside.join("secret"), "nope").unwrap();
    std::os::unix::fs::symlink(outside.join("secret"), r.remote.join("link.wav")).unwrap();
    let ch = r.open_shared().await;
    ch.send(
        &json!({ "type": "project", "file": format!("{}/link.wav", r.remote.s()) }).to_string(),
    );
    let local = r.frame(0).await["file"].as_str().unwrap().to_string();
    assert!(!std::path::Path::new(&local).exists());
}

#[tokio::test]
async fn reaches_the_engine_from_a_bound_loopback_path() {
    let r = rig().await;
    let path = r.hub.bind_local("e1", json!([]), "engine", false).await;
    let client = Client::dial(r.hub.port(), &path, None).await.unwrap();
    client.send(&json!({ "type": "whoami" }).to_string());
    assert_eq!(r.frame(0).await["type"], "whoami");
    client.close();
}
