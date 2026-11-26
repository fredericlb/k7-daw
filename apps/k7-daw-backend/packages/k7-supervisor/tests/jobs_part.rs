//! jobs_part.rs — the machine's jobs inside the whole core, over a jobs channel from a hub on 127.0.0.1: the log,
//! a set list counted, an undo's audio sent back, the settings that are jobs, the slot jobs and a render. It owns
//! its process because it moves k7's loopback port with `$STUDIO_PORTS` and sets `$STUDIO_ENGINE` for the render.

#![allow(dead_code)]

#[path = "support/hub.rs"]
mod hub;
#[path = "all/scratch.rs"]
mod scratch;

use std::path::Path;
use std::time::{Duration, SystemTime};

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
fn the_machines_jobs_answer_over_a_jobs_channel() {
    let k7 = free_port();
    let dir = Scratch::new("jobs-part");
    // SAFETY: set before any thread of this process starts one that reads the environment.
    unsafe {
        std::env::set_var("STUDIO_PORTS", format!("k7={k7}"));
        #[cfg(unix)]
        std::env::set_var("STUDIO_ENGINE", fake_runtime(dir.path()));
    }
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(flow(k7, dir.path()));
}

#[cfg(unix)]
fn fake_runtime(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("k7-runtime");
    let script = "#!/bin/sh\nout=$4; cat > /dev/null; echo RIFF > \"$out/mix.wav\"; echo '{\"mix\":\"mix.wav\"}'\n";
    std::fs::write(&p, script).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

async fn flow(k7: u16, home: &Path) {
    let hub = Hub::start_on(k7).await;
    let paths = Paths::at(&home.join("state"), None);
    std::fs::create_dir_all(paths.studio_dir()).unwrap();
    let registry = format!("http://127.0.0.1:{k7}");
    std::fs::write(
        paths.settings(),
        json!({"port": free_port(), "registryUrls": [registry]}).to_string(),
    )
    .unwrap();
    std::fs::write(paths.merged_log(), "a\nb\nc\n").unwrap();
    // Undo audio a server asked this machine to keep, a month and a day old, before the part starts.
    let kept = paths.remote_cache("elsewhere").join("history-fred/blob");
    std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
    std::fs::write(&kept, "x").unwrap();
    let old = SystemTime::now() - Duration::from_secs(31 * 24 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&kept)
        .unwrap()
        .set_modified(old)
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
    hub::until(|| !kept.exists(), 5000).await;

    hub.send(json!({"type": "x", "t": "open", "ch": 1, "kind": "jobs", "roots": [{"remote": "/srv/lib", "name": "lib", "writable": true}]}));
    hub.next_of("opened").await;
    let local = paths
        .remote_cache(&k7_supervisor::urls::cache_key(&registry))
        .join("lib");
    let ask = async |req: Value| -> Vec<Value> {
        let id = req["id"].as_str().unwrap().to_string();
        hub.send(json!({"t": "data", "ch": 1, "text": req.to_string()}));
        let mut replies = Vec::new();
        loop {
            let m = hub.next_of("data").await;
            let body: Value = serde_json::from_str(m["text"].as_str().unwrap()).unwrap();
            if body["id"] != id.as_str() {
                continue;
            }
            let last = body["type"] != "rendering";
            replies.push(body);
            if last {
                return replies;
            }
        }
    };
    let one = async |req: Value| ask(req).await.pop().unwrap();

    // The log, seeded by its tail and continued by offset; `raw` crosses unmapped.
    let r = one(json!({"type": "log", "id": "l1", "tail": 2})).await;
    assert_eq!(r["raw"], json!({"offset": 6, "lines": ["b", "c"]}));
    let r = one(json!({"type": "log", "id": "l2", "offset": 2})).await;
    assert_eq!(r["raw"], json!({"offset": 6, "lines": ["b", "c"]}));

    // A set list is counted by what is on this disk and is a copy of a server's: a path elsewhere counts for nothing.
    hub.tree
        .lock()
        .unwrap()
        .insert("/srv/lib/one.flac".into(), (b"x".to_vec(), 1000.0));
    let r = one(json!({"type": "prefetch", "id": "p", "files": ["/srv/lib/one.flac", "/srv/lib/two.flac", "/etc/hosts"]})).await;
    assert_eq!(
        (r["type"].as_str(), r["count"].clone()),
        (Some("prefetched"), json!(1))
    );

    // Undo audio only this machine keeps is stamped fresh and named, so the channel uploads it.
    let blob = local.join("kept");
    std::fs::write(&blob, "undo").unwrap();
    let stale = SystemTime::now() - Duration::from_secs(48 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&blob)
        .unwrap()
        .set_modified(stale)
        .unwrap();
    let r = one(json!({"type": "send-up", "id": "s", "file": "/srv/lib/kept"})).await;
    assert_eq!(
        (r["type"].as_str(), r["file"].as_str()),
        (Some("sent"), Some("/srv/lib/kept"))
    );
    assert!(
        std::fs::metadata(&blob).unwrap().modified().unwrap() > stale + Duration::from_secs(3600)
    );
    hub::until(
        || hub.tree.lock().unwrap().contains_key("/srv/lib/kept"),
        5000,
    )
    .await;
    let r = one(json!({"type": "send-up", "id": "s2", "file": "/srv/lib/never-kept"})).await;
    assert_eq!(r["error"], "not kept on this machine");

    // The providers and the chains are the machine's own settings, written through the core.
    let r = one(json!({"type": "onnx-providers", "id": "o", "set": ["DML", "cpu", "dml"]})).await;
    assert_eq!(r["providers"], json!(["dml", "cpu"]));
    let r = one(json!({"type": "onnx-providers", "id": "o2"})).await;
    assert_eq!(r["providers"], json!(["dml", "cpu"]));
    let r = one(json!({"type": "chains", "id": "c1", "set": {"stems": "@k7-core/chains/stems.k7x", "instrumental": "@k7-core/chains/i.k7x"}})).await;
    assert_eq!(
        r["chains"],
        json!({"stems": "@k7-core/chains/stems.k7x", "instrumental": "@k7-core/chains/i.k7x"})
    );
    // One key named: the other stays; named blank, it is unsaid.
    let r = one(json!({"type": "chains", "id": "c2", "set": {"stems": "@mine/four.k7x"}})).await;
    assert_eq!(
        r["chains"],
        json!({"stems": "@mine/four.k7x", "instrumental": "@k7-core/chains/i.k7x"})
    );
    let r = one(json!({"type": "chains", "id": "c3", "set": {"instrumental": "  "}})).await;
    assert_eq!(r["chains"], json!({"stems": "@mine/four.k7x"}));
    let saved = std::fs::read_to_string(paths.settings()).unwrap();
    assert!(
        saved.contains("@mine/four.k7x") && saved.contains("dml"),
        "{saved}"
    );

    // The slots: saved from a doc naming a copy of a server's file, listed, enabled, bound, disabled, removed.
    hub.tree
        .lock()
        .unwrap()
        .insert("/srv/lib/amp.nam".into(), (b"model".to_vec(), 1000.0));
    let rig = |id: &str, file: &str| {
        json!({"id": id, "name": "Rig", "chain": "@a/rig", "savedAt": "2026-11-01T18:00:00Z", "protocol": 7,
        "doc": {"name": "rig", "tracks": [{"plugins": [{"type": "nam", "name": "amp", "namFile": file}]}]}})
    };
    let r = one(json!({"type": "slots", "id": "k0"})).await;
    assert_eq!(
        r,
        json!({"id": "k0", "type": "slots", "slots": [], "enabledSlot": null})
    );
    let r =
        one(json!({"type": "slot-save", "id": "k1", "slot": rig("rig", "/srv/lib/amp.nam")})).await;
    assert_eq!(r["slots"][0]["id"], "rig", "{r}");
    let carried = paths.slots_dir().join("rig/files/1-amp.nam");
    assert_eq!(std::fs::read_to_string(carried).unwrap(), "model");
    // A file of this machine named by a server is not copied in.
    let r = one(json!({"type": "slot-save", "id": "k2", "slot": rig("evil", "/etc/hosts")})).await;
    assert!(
        r["error"]
            .as_str()
            .unwrap()
            .contains("outside this machine's copies"),
        "{r}"
    );
    assert!(!paths.slots_dir().join("evil").exists());
    let r = one(json!({"type": "slot-enable", "id": "k3", "slotId": "rig"})).await;
    assert_eq!(r["enabledSlot"], "rig");
    let r = one(json!({"type": "slot-enable", "id": "k3b", "slotId": "ghost"})).await;
    assert_eq!(r["error"], "no slot 'ghost'");
    let r = one(json!({"type": "slot-footswitch", "id": "k4", "slotId": "rig", "cc": 64})).await;
    assert_eq!(r["slots"][0]["footswitch"], 64);
    let r = one(json!({"type": "slot-footswitch", "id": "k5", "slotId": "rig", "cc": 300})).await;
    assert_eq!(r["error"], "no slot 'rig', or no CC 300");
    let r = one(json!({"type": "slot-disable", "id": "k6"})).await;
    assert_eq!(r["enabledSlot"], Value::Null);
    let r = one(json!({"type": "slot-remove", "id": "k7", "slotId": "rig"})).await;
    assert_eq!(r["slots"], json!([]));
    let r = one(json!({"type": "slot-save", "id": "k8"})).await;
    assert_eq!(r["error"], "slot-save wants a slot");

    // A write outside the channel's roots is refused before anything runs.
    let r = one(
        json!({"type": "render", "id": "r0", "doc": {"tracks": []}, "out": "/elsewhere/mix.wav"}),
    )
    .await;
    assert!(
        r["error"]
            .as_str()
            .unwrap()
            .contains("outside this server's writable folders"),
        "{r}"
    );
    #[cfg(unix)]
    {
        let replies = ask(
            json!({"type": "render", "id": "r1", "doc": {"tracks": []}, "out": "/srv/lib/mix.wav"}),
        )
        .await;
        assert_eq!(replies[0]["type"], "rendering");
        assert_eq!(
            replies[1],
            json!({"id": "r1", "type": "rendered", "file": "/srv/lib/mix.wav"})
        );
        hub::until(
            || hub.tree.lock().unwrap().contains_key("/srv/lib/mix.wav"),
            5000,
        )
        .await;
    }
    let r = one(json!({"type": "render", "id": "r2", "out": "/srv/lib/m.wav"})).await;
    assert_eq!(
        r["error"],
        "render: a doc and an out path or folder are required"
    );

    tx.send(Input::Shutdown).unwrap();
    running.await.unwrap().unwrap();
}
