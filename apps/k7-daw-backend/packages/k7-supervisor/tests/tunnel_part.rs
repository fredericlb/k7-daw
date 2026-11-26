//! tunnel_part.rs — the tunnel part inside the whole core, against a hub on 127.0.0.1: the agent dials the
//! registry the settings name with this machine's identity, a jobs channel reaches the jobs table, and a path
//! outside this machine's copies of the server's folders is refused. It owns its process because it moves k7's
//! loopback port with `$STUDIO_PORTS`, which every loopback address then means.

#![allow(dead_code)]

#[path = "support/hub.rs"]
mod hub;
#[path = "all/scratch.rs"]
mod scratch;

use k7_supervisor::core::{self, Boot, Input};
use k7_supervisor::frames::Frames;
use k7_supervisor::paths::Paths;
use k7_supervisor::settings::Store;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use hub::Hub;
use scratch::Scratch;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn the_part_dials_the_registry_and_answers_the_jobs_table() {
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
    let hub = Hub::start_on(k7).await;
    let dir = Scratch::new("tunnel-part");
    let paths = Paths::at(dir.path(), None);
    std::fs::write(
        paths.settings(),
        format!(
            r#"{{"port": {}, "registryUrls": ["http://127.0.0.1:{k7}"]}}"#,
            free_port()
        ),
    )
    .unwrap();
    let (ftx, _frames) = mpsc::unbounded_channel();
    let boot = Boot {
        store: Store::new(paths.settings()),
        paths: paths.clone(),
        frames: Frames::channel(ftx),
        engine_binary: Some(env!("CARGO_BIN_EXE_k7-fake-engine").into()),
        parts: k7_supervisor::parts::all(),
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let running = tokio::spawn(core::run(boot, rx));
    hub.until_accepted(1).await;

    let who: Value =
        serde_json::from_str(&std::fs::read_to_string(paths.engine_id()).unwrap()).unwrap();
    assert_eq!(
        hub.header(0, "x-studio-engine-token").as_deref(),
        who["token"].as_str()
    );
    assert_eq!(
        hub.header(0, "x-studio-tunnel-secret").as_deref(),
        who["tunnelSecret"].as_str()
    );
    assert_eq!(
        hub.header(0, ":path").unwrap(),
        format!("/api/engine-tunnel?id={}", who["id"].as_str().unwrap())
    );

    hub.send(json!({"t": "open", "ch": 1, "kind": "jobs", "roots": [{"remote": "/srv/lib", "name": "lib", "writable": true}]}));
    hub.next_of("opened").await;
    let ask = |req: Value| hub.send(json!({"t": "data", "ch": 1, "text": req.to_string()}));
    let reply = async |id: &str| -> Value {
        loop {
            let m = hub.next_of("data").await;
            let body: Value = serde_json::from_str(m["text"].as_str().unwrap()).unwrap();
            if body["id"] == id {
                return body;
            }
        }
    };

    // A job nobody on this machine does is said, by name.
    ask(json!({"type": "teleport", "id": "a"}));
    assert_eq!(
        reply("a").await["error"],
        "this machine does not do 'teleport'"
    );

    // `separate` is in the table, and its paths are checked before a cut starts.
    ask(
        json!({"type": "separate", "id": "b", "source": "/etc/hosts", "model": "/srv/lib/m.onnx", "outDir": "/srv/lib/stems"}),
    );
    assert!(
        reply("b").await["error"]
            .as_str()
            .unwrap()
            .contains("outside this machine's copies")
    );
    ask(
        json!({"type": "separate", "id": "c", "source": "/srv/lib/mix.wav", "model": "/srv/lib/m.onnx", "outDir": "/elsewhere/stems"}),
    );
    assert!(
        reply("c").await["error"]
            .as_str()
            .unwrap()
            .contains("outside this server's writable folders")
    );

    tx.send(Input::Shutdown).unwrap();
    running.await.unwrap().unwrap();
}
