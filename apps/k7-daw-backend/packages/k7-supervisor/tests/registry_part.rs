//! registry_part.rs — the registry part inside the whole core, against a registry on 127.0.0.1: the engine's
//! start brings the heartbeat, a studio is added only once its hello proves it one, a pairing keeps its token
//! for that studio alone, a removal leaves it, and a stop or a quit deregisters before the reply. It owns its
//! process because it moves k7's loopback port with `$STUDIO_PORTS`, which every loopback address then means.

#![allow(dead_code)]

#[path = "all/scratch.rs"]
mod scratch;
#[path = "support/server.rs"]
mod server;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use k7_supervisor::core::{self, Boot, Input};
use k7_supervisor::frames::Frames;
use k7_supervisor::paths::Paths;
use k7_supervisor::protocol::Command;
use k7_supervisor::settings::Store;
use serde_json::Value;
use tokio::sync::mpsc;

use scratch::Scratch;
use server::{Reply, Seen, Server};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Every frame the supervisor wrote, kept: a result and the snapshot beside it come in either order.
struct Feed {
    rx: mpsc::UnboundedReceiver<String>,
    all: Vec<Value>,
}

impl Feed {
    /// The first frame at or after `from` that `want` accepts, waiting for it.
    async fn find(&mut self, from: usize, want: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(15);
        let mut at = from;
        loop {
            while at < self.all.len() {
                at += 1;
                if want(&self.all[at - 1]) {
                    return self.all[at - 1].clone();
                }
            }
            let left = end.saturating_duration_since(Instant::now());
            let line = tokio::time::timeout(left, self.rx.recv())
                .await
                .unwrap_or_else(|_| {
                    panic!("a frame in time; since the mark: {:?}", &self.all[from..])
                })
                .expect("frames");
            self.all.push(serde_json::from_str(&line).unwrap());
        }
    }

    /// The mark to search from: everything written after now.
    fn mark(&self) -> usize {
        self.all.len()
    }

    async fn result(&mut self, from: usize, id: i64) -> Value {
        self.find(from, |f| f["t"] == "result" && f["id"] == id)
            .await
    }
}

fn cmd(id: i64, name: &str, arg: &str) -> Input {
    Input::Command(Command {
        id,
        cmd: name.into(),
        arg: arg.into(),
    })
}

fn to_host(s: &Server, path: &str, host: &str) -> Vec<Seen> {
    s.to(path)
        .into_iter()
        .filter(|r| r.header("host").is_some_and(|h| h.starts_with(host)))
        .collect()
}

#[test]
fn the_part_lists_pairs_removes_and_leaves() {
    let k7 = free_port();
    // SAFETY: set before any thread of this process starts one that reads the environment.
    unsafe { std::env::set_var("STUDIO_PORTS", format!("k7={k7}")) };
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(flow(k7));
}

async fn flow(k7: u16) {
    let hello_ok = Arc::new(AtomicBool::new(true));
    let flag = hello_ok.clone();
    let server = server::serve_on(k7, move |s| match s.path.as_str() {
        "/api/registry/hello" if flag.load(Ordering::SeqCst) => {
            Reply::Status(200, r#"{"service":"k7-registry"}"#.into())
        }
        "/api/registry/hello" => Reply::Status(200, r#"{"service":"other"}"#.into()),
        _ => Reply::Status(200, "{}".into()),
    })
    .await;
    let dir = Scratch::new("registry-part");
    let paths = Paths::at(dir.path(), None);
    let origin = format!("http://127.0.0.1:{k7}");
    std::fs::write(
        paths.settings(),
        format!(
            r#"{{"port": {}, "registryUrls": ["{origin}"]}}"#,
            free_port()
        ),
    )
    .unwrap();
    let (ftx, rx0) = mpsc::unbounded_channel();
    let mut feed = Feed {
        rx: rx0,
        all: Vec::new(),
    };
    let boot = Boot {
        store: Store::new(paths.settings()),
        paths: paths.clone(),
        frames: Frames::channel(ftx),
        engine_binary: Some(env!("CARGO_BIN_EXE_k7-fake-engine").into()),
        parts: k7_supervisor::parts::all(),
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let running = tokio::spawn(core::run(boot, rx));

    // The engine's start brings the first heartbeat, and the snapshot's light.
    let up = feed
        .find(0, |f| {
            f["status"] == "running" && f["lastHeartbeatOk"] == true
        })
        .await;
    assert_eq!(up["registryUrl"], origin.as_str());
    assert_eq!(
        up["instances"],
        serde_json::json!([{"url": "http://127.0.0.1", "removable": true}])
    );
    let beat: Value = serde_json::from_str(&server.to("/api/registry/heartbeat")[0].body).unwrap();
    assert_eq!(beat["id"], up["engineId"]);
    assert_eq!(beat["libraryLevel"], 2);
    assert!(beat["performance"]["score"].as_i64().unwrap() > 0);
    assert_eq!(beat["addresses"]["port"], up["enginePort"]);
    assert!(beat["tunnelSecret"].as_str().unwrap().len() == 64 && beat.get("safeMode").is_none());

    // A studio whose hello is not a registry's is refused, and nothing is written.
    hello_ok.store(false, Ordering::SeqCst);
    let m = feed.mark();
    tx.send(cmd(1, "weburi.add", "http://localhost:5223"))
        .unwrap();
    let refused = feed
        .find(m, |f| f["instanceCheck"]["state"] == "refused")
        .await;
    assert!(
        refused["instanceCheck"]["message"]
            .as_str()
            .unwrap()
            .ends_with("— nothing was added")
    );
    assert_eq!(feed.result(m, 1).await["ok"], false);
    assert!(
        !std::fs::read_to_string(paths.settings())
            .unwrap()
            .contains("localhost")
    );
    let m = feed.mark();
    tx.send(cmd(2, "weburi.add", "https://bad host")).unwrap();
    let r = feed.result(m, 2).await;
    assert_eq!(
        (r["ok"].clone(), r["message"].clone()),
        (false.into(), "not a valid web UI address".into())
    );

    // One that answers is added with its token, and its first heartbeat carries the token.
    hello_ok.store(true, Ordering::SeqCst);
    let m = feed.mark();
    tx.send(cmd(3, "weburi.pair", "http://localhost:5223\u{1f}tok-1"))
        .unwrap();
    let r = feed.result(m, 3).await;
    assert_eq!(
        (r["ok"].clone(), r["message"].clone()),
        (true.into(), "http://localhost:5223".into())
    );
    let both = feed
        .find(m, |f| {
            f["registryUrl"] == format!("{origin}, http://localhost:{k7}").as_str()
        })
        .await;
    assert!(both["instanceCheck"].is_null());
    let sent = to_host(&server, "/api/registry/heartbeat", "localhost");
    assert_eq!(sent[0].header("authorization"), Some("Bearer tok-1"));
    assert_eq!(
        to_host(&server, "/api/registry/heartbeat", "127.0.0.1")
            .last()
            .unwrap()
            .header("authorization"),
        None
    );
    let saved = std::fs::read_to_string(paths.settings()).unwrap();
    assert!(saved.contains("tok-1") && saved.contains(&format!("http://localhost:{k7}")));

    // Removing it tells it at once and forgets its token.
    let m = feed.mark();
    tx.send(cmd(4, "weburi.remove", "http://localhost:5223"))
        .unwrap();
    assert_eq!(feed.result(m, 4).await["ok"], true);
    assert_eq!(
        to_host(&server, "/api/registry/deregister", "localhost").len(),
        1
    );
    assert!(
        !std::fs::read_to_string(paths.settings())
            .unwrap()
            .contains("tok-1")
    );
    feed.find(m, |f| f["registryUrl"] == origin.as_str()).await;

    // A stop leaves before its reply; the light goes out, and the next start lists it again.
    let m = feed.mark();
    tx.send(cmd(5, "engine.stop", "")).unwrap();
    assert_eq!(feed.result(m, 5).await["ok"], true);
    assert_eq!(
        to_host(&server, "/api/registry/deregister", "127.0.0.1").len(),
        1
    );
    feed.find(m, |f| {
        f["status"] == "stopped" && f["lastHeartbeatOk"] == false
    })
    .await;
    let m = feed.mark();
    tx.send(cmd(6, "engine.start", "")).unwrap();
    feed.find(m, |f| {
        f["status"] == "running" && f["lastHeartbeatOk"] == true
    })
    .await;

    // A quit leaves too.
    tx.send(Input::Shutdown).unwrap();
    assert_eq!(running.await.unwrap(), Ok(()));
    assert_eq!(
        to_host(&server, "/api/registry/deregister", "127.0.0.1").len(),
        2
    );
}
