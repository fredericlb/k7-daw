//! tunnel_agent.rs — the agent against a hub and an engine on 127.0.0.1: what its handshake says, how an engine
//! channel pipes frames, how a close follows the frames before it, how it redials, what it refuses, and the
//! frames' files moved before the frame naming them. `engine-tunnel`'s suites judge the same wire against the
//! real hub (`tunnel_harness`); these hold the agent's own behaviours where a failure names the line.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_supervisor::tunnel::agent::backoff;
use k7_supervisor::tunnel::{Jobs, Options, TunnelAgent};
use serde_json::{Value, json};

use crate::hub::{Engine, Got, Hub, until};
use crate::scratch::Scratch;

struct Rig {
    hub: Hub,
    engine: Engine,
    agent: TunnelAgent,
    log: Arc<Mutex<Vec<String>>>,
    cache: Scratch,
}

impl Rig {
    fn said(&self, needle: &str) -> usize {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.contains(needle))
            .count()
    }

    /// The `data` frame the hub gets next, its text parsed.
    async fn next_data(&self) -> Value {
        let m = self.hub.next_of("data").await;
        serde_json::from_str(m["text"].as_str().unwrap()).unwrap()
    }

    fn lib(&self, name: &str) -> String {
        self.cache
            .path()
            .join("lib")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }
}

async fn rig_with(jobs: Jobs) -> Rig {
    let (hub, engine) = (Hub::start().await, Engine::start().await);
    let (cache, log) = (Scratch::new("tunnel"), Arc::<Mutex<Vec<String>>>::default());
    let mut o = Options::new(&hub.url(), "k7 id", "tok", cache.path().to_path_buf());
    o.tunnel_secret = "sec".into();
    let port = engine.port;
    o.engine_port = Arc::new(move || port);
    o.jobs = jobs;
    let sink = log.clone();
    o.log = Arc::new(move |m| sink.lock().unwrap().push(m.to_string()));
    let agent = TunnelAgent::start(o);
    hub.until_accepted(1).await;
    until(|| agent.connected(), 3000).await;
    Rig {
        hub,
        engine,
        agent,
        log,
        cache,
    }
}

async fn rig() -> Rig {
    rig_with(Jobs::default()).await
}

fn roots() -> Value {
    json!([{"remote": "/srv/lib", "name": "lib", "writable": true}])
}

/// Opens engine channel `ch` and waits for the agent to say the engine answered.
async fn open(r: &Rig, ch: i64, roots: Value, synced: bool) {
    r.hub
        .send(json!({"t": "open", "ch": ch, "roots": roots, "kind": "engine", "synced": synced}));
    assert_eq!(r.hub.next_of("opened").await["ch"], ch);
}

#[tokio::test]
async fn the_handshake_carries_the_token_the_secret_and_the_protocol() {
    let r = rig().await;
    assert_eq!(
        r.hub.header(0, "x-studio-engine-token").as_deref(),
        Some("tok")
    );
    assert_eq!(
        r.hub.header(0, "x-studio-tunnel-secret").as_deref(),
        Some("sec")
    );
    assert_eq!(
        r.hub.header(0, "x-studio-engine-protocol").as_deref(),
        Some("1")
    );
    assert_eq!(
        r.hub.header(0, ":path").as_deref(),
        Some("/api/engine-tunnel?id=k7%20id")
    );
    assert_eq!(r.said("up to http://127.0.0.1"), 1);
}

#[tokio::test]
async fn an_engine_channel_pipes_text_and_binary_both_ways() {
    let r = rig().await;
    open(&r, 1, json!([]), false).await;
    r.hub
        .send(json!({"t": "data", "ch": 1, "text": "{\"type\":\"whoami\"}"}));
    r.hub.send_binary(json!({"t": "data", "ch": 1}), &[1, 2, 3]);
    until(|| r.engine.got.lock().unwrap().len() == 2, 3000).await;
    assert_eq!(r.engine.texts(), [json!({"type": "whoami"})]);
    assert!(matches!(&r.engine.got.lock().unwrap()[1], Got::Binary(_, b) if b == &[1, 2, 3]));
    r.engine.say(json!({"type": "whoami", "id": "x"}));
    assert_eq!(r.next_data().await["id"], "x");
    r.engine.say_binary(vec![9, 8]);
    assert!(
        matches!(r.hub.next().await, Got::Binary(h, b) if h["t"] == "data" && h["ch"] == 1 && b == [9, 8])
    );
}

#[tokio::test]
async fn a_root_name_that_could_climb_is_refused() {
    let r = rig().await;
    r.hub.send(json!({"t": "open", "ch": 4, "roots": [{"remote": "/s", "name": "../x", "writable": true}], "kind": "engine"}));
    let close = r.hub.next_of("close").await;
    assert_eq!(
        (close["ch"].as_i64(), close["reason"].as_str()),
        (Some(4), Some("refused a root name"))
    );
    assert_eq!(
        r.engine
            .connections
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn a_close_comes_after_the_frames_sent_before_it() {
    let r = rig().await;
    open(&r, 1, json!([]), false).await;
    for n in 1..=3 {
        r.hub
            .send(json!({"t": "data", "ch": 1, "text": json!({"n": n}).to_string()}));
    }
    r.hub.send(json!({"t": "close", "ch": 1}));
    until(|| r.engine.texts().len() == 3, 3000).await;
    assert_eq!(
        r.engine
            .texts()
            .iter()
            .map(|t| t["n"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[tokio::test]
async fn the_engines_last_frames_reach_the_hub_before_its_close() {
    let r = rig().await;
    open(&r, 1, json!([]), false).await;
    for n in 1..=3 {
        r.engine.say(json!({"n": n}));
    }
    r.engine.hang_up();
    for n in 1..=3 {
        assert_eq!(r.next_data().await["n"], n);
    }
    assert_eq!(r.hub.next_of("close").await["ch"], 1);
}

#[tokio::test]
async fn an_engine_that_cannot_be_reached_closes_the_channel_with_why() {
    let gone = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (hub, cache) = (Hub::start().await, Scratch::new("tunnel"));
    let mut o = Options::new(&hub.url(), "e1", "tok", cache.path().to_path_buf());
    o.engine_port = Arc::new(move || gone);
    let _agent = TunnelAgent::start(o);
    hub.until_accepted(1).await;
    hub.send(json!({"t": "open", "ch": 2, "roots": [], "kind": "engine"}));
    let close = hub.next_of("close").await;
    assert_eq!(close["ch"], 2);
    assert!(!close["reason"].as_str().unwrap_or("").is_empty());
}

#[tokio::test]
async fn a_frame_the_agent_does_not_read_is_said_once_per_type() {
    let r = rig().await;
    for _ in 0..3 {
        r.hub.send(json!({"t": "mystery", "ch": 1}));
    }
    r.hub.send(json!([1, 2]));
    r.hub.send_binary(json!({"t": "wonder"}), &[0]);
    // Frames are read in order: the channel's answer says the ones before it were.
    open(&r, 1, json!([]), false).await;
    assert_eq!(r.said("type 'mystery'"), 1);
    assert_eq!(r.said("type '(not a JSON object)'"), 1);
    assert_eq!(r.said("binary frame of type 'wonder'"), 1);
}

#[tokio::test]
async fn a_jobs_channel_is_answered_by_the_agent_itself() {
    let mut jobs = Jobs::default();
    jobs.add("echo", |req, emit, _| async move {
        emit.send(json!({"type": "progress", "n": 1}));
        emit.send(json!({"type": "done", "said": req["say"]}));
        Ok(())
    });
    jobs.add("fails", |_, _, _| async { Err("no disk".to_string()) });
    jobs.add("panics", |_, _, _| async { panic!("boom") });
    let r = rig_with(jobs).await;
    r.hub
        .send(json!({"t": "open", "ch": 3, "roots": [], "kind": "jobs"}));
    r.hub.next_of("opened").await;
    for req in [
        json!({"type": "echo", "id": "a", "say": "hi"}),
        json!({"type": "fails", "id": "b"}),
        json!({"type": "panics", "id": "c"}),
        json!({"type": "nope"}),
    ] {
        r.hub
            .send(json!({"t": "data", "ch": 3, "text": req.to_string()}));
    }
    let mut seen: Vec<Value> = Vec::new();
    while seen.len() < 5 {
        seen.push(r.next_data().await);
    }
    let of = |id: &str| {
        seen.iter()
            .filter(|m| m["id"] == id)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        of("a")
            .iter()
            .map(|m| m["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["progress", "done"]
    );
    assert_eq!(of("a")[1]["said"], "hi");
    assert_eq!(
        of("b")[0],
        json!({"id": "b", "type": "error", "error": "no disk"})
    );
    assert_eq!(of("c")[0]["type"], "error");
    let nope = seen.iter().find(|m| m.get("id").is_none()).unwrap();
    assert!(
        nope["error"]
            .as_str()
            .unwrap()
            .contains("does not do 'nope'")
    );
    assert_eq!(r.said("refused job 'nope'"), 1);
    assert_eq!(r.said("job 'fails' failed: no disk"), 1);
}

#[tokio::test]
async fn a_file_is_fetched_before_the_frame_that_names_it() {
    let r = rig().await;
    r.hub.tree.lock().unwrap().insert(
        "/srv/lib/a.wav".into(),
        (b"la la la".to_vec(), 1_600_000_000_000.0),
    );
    open(&r, 1, roots(), false).await;
    let named = json!({"type": "project", "clip": "/srv/lib/a.wav", "n": 1.5}).to_string();
    r.hub.send(json!({"t": "data", "ch": 1, "text": named}));
    until(|| r.engine.texts().len() == 1, 3000).await;
    let local = r.lib("a.wav");
    assert_eq!(
        r.engine.texts()[0],
        json!({"type": "project", "clip": local, "n": 1.5})
    );
    assert_eq!(std::fs::read(&local).unwrap(), b"la la la");
    let mtime = k7_supervisor::tunnel::filetime::stat(std::path::Path::new(&local))
        .unwrap()
        .mtime_ms;
    assert!(
        (mtime - 1_600_000_000_000.0).abs() < 2.0,
        "stamped with the server's mtime: {mtime}"
    );
    // A delta trusts what the document settled; the next document asks again and finds it current.
    let stats = r.hub.stats.load(std::sync::atomic::Ordering::SeqCst);
    r.hub.send(json!({"t": "data", "ch": 1, "text": json!({"type": "set", "clip": "/srv/lib/a.wav"}).to_string()}));
    until(|| r.engine.texts().len() == 2, 3000).await;
    assert_eq!(r.hub.stats.load(std::sync::atomic::Ordering::SeqCst), stats);
    r.hub.send(json!({"t": "data", "ch": 1, "text": named}));
    until(|| r.engine.texts().len() == 3, 3000).await;
    assert_eq!(
        r.hub.stats.load(std::sync::atomic::Ordering::SeqCst),
        stats + 1
    );
    assert_eq!(r.hub.gets.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_synced_channel_only_rewrites() {
    let r = rig().await;
    open(&r, 1, roots(), true).await;
    r.hub.send(json!({"t": "data", "ch": 1, "text": json!({"type": "project", "clip": "/srv/lib/b.wav"}).to_string()}));
    until(|| r.engine.texts().len() == 1, 3000).await;
    assert_eq!(r.engine.texts()[0]["clip"], r.lib("b.wav"));
    assert_eq!(r.hub.stats.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_file_the_engine_wrote_goes_up_before_the_frame_naming_it() {
    let r = rig().await;
    open(&r, 1, roots(), false).await;
    let take = r.lib("take.wav");
    std::fs::create_dir_all(r.cache.path().join("lib")).unwrap();
    std::fs::write(&take, vec![7u8; 600_000]).unwrap();
    r.engine
        .say(json!({"type": "recorded", "file": take, "note": "kept"}));
    let sent = r.next_data().await;
    assert_eq!(
        sent,
        json!({"type": "recorded", "file": "/srv/lib/take.wav", "note": "kept"})
    );
    let tree = r.hub.tree.lock().unwrap();
    assert_eq!(tree["/srv/lib/take.wav"].0.len(), 600_000);
}

#[tokio::test]
async fn a_jobs_sync_copies_what_is_out_of_date_and_says_how_far() {
    let r = rig_with(Jobs::default()).await;
    for (n, name) in ["x.wav", "y.wav"].iter().enumerate() {
        r.hub.tree.lock().unwrap().insert(
            format!("/srv/lib/{name}"),
            (vec![n as u8; 10], 1_500_000_000_000.0),
        );
    }
    r.hub
        .send(json!({"t": "open", "ch": 5, "roots": roots(), "kind": "jobs"}));
    r.hub.next_of("opened").await;
    let req = json!({"type": "sync", "id": "s", "files": ["/srv/lib/x.wav", "/srv/lib/y.wav", "/srv/lib/gone.wav"]});
    r.hub
        .send(json!({"t": "data", "ch": 5, "text": req.to_string()}));
    let mut steps = Vec::new();
    loop {
        let m = r.next_data().await;
        steps.push(m.clone());
        if m["type"] == "synced" {
            break;
        }
    }
    assert_eq!(steps.first().unwrap()["files"], 2);
    assert_eq!(
        steps
            .iter()
            .filter(|m| m["type"] == "syncing")
            .map(|m| m["filesDone"].as_i64().unwrap())
            .max(),
        Some(2)
    );
    assert_eq!(
        steps.last().unwrap(),
        &json!({"id": "s", "type": "synced", "failed": 0, "failures": []})
    );
    assert_eq!(std::fs::read(r.lib("y.wav")).unwrap(), vec![1u8; 10]);
}

#[tokio::test]
async fn the_agent_redials_after_the_hub_hangs_up() {
    let r = rig().await;
    r.hub.kick();
    until(|| !r.agent.connected(), 3000).await;
    r.hub.until_accepted(2).await;
    until(|| r.agent.connected(), 3000).await;
    assert_eq!(r.said("down from"), 1);
    r.agent.stop();
    until(|| !r.agent.connected() || r.said("down from") == 2, 3000).await;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(r.hub.accepted(), 2, "a stopped agent does not redial");
}

#[tokio::test]
async fn a_refusal_is_said_with_its_status_and_its_words() {
    let hub = Hub::start().await;
    *hub.refuse.lock().unwrap() = Some(426);
    let (cache, log) = (Scratch::new("tunnel"), Arc::<Mutex<Vec<String>>>::default());
    let mut o = Options::new(&hub.url(), "e1", "tok", cache.path().to_path_buf());
    let sink = log.clone();
    o.log = Arc::new(move |m| sink.lock().unwrap().push(m.to_string()));
    let _agent = TunnelAgent::start(o);
    until(|| !log.lock().unwrap().is_empty(), 3000).await;
    let said = log.lock().unwrap()[0].clone();
    assert!(
        said.contains("HTTP 426") && said.contains("update k7 on that machine"),
        "{said}"
    );
}

#[tokio::test]
async fn plain_http_past_loopback_is_never_dialled() {
    let log = Arc::<Mutex<Vec<String>>>::default();
    let mut o = Options::new("http://example.com", "e1", "tok", std::env::temp_dir());
    let sink = log.clone();
    o.log = Arc::new(move |m| sink.lock().unwrap().push(m.to_string()));
    let _agent = TunnelAgent::start(o);
    until(|| !log.lock().unwrap().is_empty(), 3000).await;
    assert!(
        log.lock().unwrap()[0].starts_with("tunnel not dialled: http://example.com is plain http")
    );
}

#[test]
fn the_wait_before_a_redial_is_a_jittered_exponential_step() {
    for (attempt, lo, hi) in [
        (0u32, 500, 1000),
        (1, 1000, 2000),
        (3, 4000, 8000),
        (5, 16000, 30000),
        (9, 15000, 30000),
    ] {
        for random in [0u32, 1, 12345, u32::MAX] {
            let ms = backoff(attempt, random).as_millis() as u64;
            assert!(
                (lo.min(15000)..=hi).contains(&ms),
                "attempt {attempt}: {ms}"
            );
        }
    }
}
