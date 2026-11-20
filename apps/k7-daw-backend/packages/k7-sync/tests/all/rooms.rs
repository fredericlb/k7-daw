//! rooms.rs — the relay's room lifecycle without a socket: an idle room's automatic checkpoint and eviction, a log a
//! newer k7 wrote refused, a crash's torn line closed off. Each room is driven through `Relay::room().submit()`.
//! What a reader gets wrong: the timers are real and short here; every wait ends in a panic, never a hang.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use common::{FakeJs, options, temp};
use k7_sync::relay::Relay;
use serde_json::{Map, Value, json};

fn batch(id: &str, text: &str) -> Map<String, Value> {
    json!({ "id": id, "replica": "r", "local": 1, "ts": 1, "label": "edit",
        "ops": [{ "kind": "text", "file": "a.txt", "text": text }] })
    .as_object()
    .cloned()
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn drops_an_idle_room_after_its_automatic_checkpoint_and_loads_it_back_with_every_entry() {
    let root = temp("evict");
    let mut o = options(&root);
    o.evict_idle = Duration::from_millis(150);
    o.checkpoint_idle = Duration::from_secs(3600);
    let r = Relay::new(o, Arc::new(FakeJs));
    r.start();
    let live = r.room("acct", "w").unwrap();
    for i in 0..3 {
        live.submit(batch(&format!("b{i}"), &format!("{i}\n")))
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        r.loaded(),
        1,
        "an edit still waiting for its checkpoint keeps the room"
    );
    r.stop();

    let mut o = options(&root);
    o.evict_idle = Duration::from_millis(150);
    o.checkpoint_idle = Duration::from_millis(50);
    let r = Relay::new(o, Arc::new(FakeJs));
    r.start();
    let live = r.room("acct", "w").unwrap();
    assert_eq!(live.lock().room().head, 3.0);
    assert_eq!(live.lock().room().files()["a.txt"], "2\n");
    live.submit(batch("b3", "3\n")).unwrap();
    drop(live);
    let end = tokio::time::Instant::now() + Duration::from_secs(5);
    while r.loaded() > 0 {
        assert!(
            tokio::time::Instant::now() < end,
            "the idle room is never dropped"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let dir = k7_sync::path::room_dir(&r.account_dir("acct"), "w");
    let cps: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("checkpoints.json")).unwrap())
            .unwrap();
    assert_eq!(cps.as_array().unwrap().len(), 1);
    assert_eq!(cps[0]["seq"], 4);
    assert_eq!(cps[0]["name"], "auto");
    let live = r.room("acct", "w").unwrap();
    assert_eq!(live.lock().room().head, 4.0);
    assert_eq!(live.lock().room().files()["a.txt"], "3\n");
    r.stop();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn does_not_load_a_room_a_newer_k7_wrote() {
    let root = temp("newer");
    let r = Relay::new(options(&root), Arc::new(FakeJs));
    let dir = k7_sync::path::room_dir(&r.account_dir("acct"), "w");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("entries.jsonl"),
        "{\"v\":2,\"seq\":1,\"batch\":{\"ops\":[]},\"parked\":[]}\n",
    )
    .unwrap();
    let e = r
        .room("acct", "w")
        .err()
        .expect("a newer log must not load");
    assert_eq!(
        e,
        "entry 1 written by log version 2, this k7 reads up to 1: update k7"
    );
    assert_eq!(r.loaded(), 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_crash_s_torn_line_is_closed_off_before_the_next_entry() {
    let root = temp("restart");
    let r = Relay::new(options(&root), Arc::new(FakeJs));
    r.room("acct", "w")
        .unwrap()
        .submit(batch("b1", "1\n"))
        .unwrap();
    // No stop, as after a crash: the entries are on disk already, and a torn line follows them.
    let entries = k7_sync::path::room_dir(&r.account_dir("acct"), "w").join("entries.jsonl");
    let mut text = std::fs::read_to_string(&entries).unwrap();
    assert_eq!(text.lines().count(), 1);
    text.push_str("{\"v\":1,\"seq\":2,");
    std::fs::write(&entries, &text).unwrap();
    drop(r);
    let r = Relay::new(options(&root), Arc::new(FakeJs));
    let live = r.room("acct", "w").unwrap();
    assert_eq!(live.lock().room().head, 1.0);
    let e = live.submit(batch("b2", "2\n")).unwrap();
    assert_eq!(e["seq"].as_f64(), Some(2.0));
    let text = std::fs::read_to_string(&entries).unwrap();
    assert!(
        text.contains("\"seq\":2,\n{\"v\":1,\"seq\":2,\"batch\""),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
