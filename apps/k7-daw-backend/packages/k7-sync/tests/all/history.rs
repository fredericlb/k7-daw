//! history.rs — the history's cases that need no socket: the history's answers, a share's checkpoint and files,
//! the orphan sweep.
//! What a reader gets wrong: the relay here is driven through its live rooms, as the history routes drive it.

use crate::common;

use std::sync::Arc;

use common::{FakeJs, options, temp};
use k7_sync::history::{add_checkpoint, diff, page, restore};
use k7_sync::relay::Relay;
use serde_json::{Map, Value, json};

fn batch(id: &str, ops: Value) -> Map<String, Value> {
    json!({ "id": id, "replica": "r", "local": 1, "ts": 1, "label": "edit", "ops": ops })
        .as_object()
        .cloned()
        .unwrap()
}

fn text(file: &str, t: &str) -> Value {
    json!({ "kind": "text", "file": file, "text": t })
}

fn relay(root: &std::path::Path, every: f64) -> Relay {
    let mut o = options(root);
    o.snapshot_every = every;
    o.history_every = every * 2.0;
    Relay::new(o, Arc::new(FakeJs))
}

#[test]
fn pages_through_many_segments_and_rebuilds_a_missing_index() {
    let root = temp("pages");
    let r = relay(&root, 3.0);
    let live = r.room("acct", "w").unwrap();
    for i in 1..=20 {
        live.submit(batch(
            &format!("b{i}"),
            json!([text("a.txt", &format!("{i}\n"))]),
        ))
        .unwrap();
    }
    let (s, v) = page(&r, Some("acct"), "w", Some(&json!("12")), Some(&json!("4")));
    assert_eq!(s, 200);
    let seqs: Vec<_> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["seq"].as_f64().unwrap())
        .collect();
    assert_eq!(seqs, [11.0, 10.0, 9.0, 8.0]);
    let dir = root.join("acct/.sync-relay/w");
    std::fs::remove_file(dir.join("log/index.json")).unwrap();
    let (_, v) = page(&r, Some("acct"), "w", None, Some(&json!(0)));
    assert_eq!(v["entries"][0]["seq"], 20);
    assert!(dir.join("log/index.json").exists());
    let (s, v) = diff(&r, Some("acct"), "w", Some(&json!("3")), Some(&json!(20)));
    assert_eq!(
        (s, v["files"].clone()),
        (
            200,
            json!([{ "file": "a.txt", "before": "3\n", "after": "20\n" }])
        )
    );
    assert_eq!(page(&r, None, "w", None, None).0, 401);
    assert_eq!(page(&r, Some("acct"), "a/b", None, None).0, 400);
    assert_eq!(
        page(&r, Some("acct"), "x", None, None),
        (404, json!({ "error": "no history for x" }))
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_restore_settles_the_conflicts_on_the_files_it_rewrites_and_only_those() {
    let root = temp("restore");
    let r = relay(&root, 200.0);
    let live = r.room("acct", "w").unwrap();
    live.submit(batch(
        "b1",
        json!([text("a.md", "1\n"), text("b.md", "1\n")]),
    ))
    .unwrap();
    live.submit(batch(
        "b2",
        json!([text("a.md", "2\n"), text("b.md", "2\n")]),
    ))
    .unwrap();
    let stale = |f: &str| json!({ "kind": "remove", "file": f, "was": "nothash" });
    live.submit(batch("b3", json!([stale("a.md"), stale("b.md")])))
        .unwrap();
    live.submit(batch("b4", json!([text("b.md", "1\n")])))
        .unwrap();
    assert_eq!(live.lock().room().conflicts.len(), 2);
    let (_, cp) = add_checkpoint(
        &r,
        Some("acct"),
        "w",
        &json!({ "name": "first", "seq": 1 }),
        "a@b.c",
    );
    assert_eq!(cp["by"], "a@b.c");
    // At 1 b.md is as now: only a.md is rewritten, and only its alternative settled.
    let (s, v) = restore(&r, Some("acct"), "w", &json!({ "seq": 1 }));
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["entry"]["label"], "restore to first");
    assert_eq!(v["entry"]["files"], json!(["a.md"]));
    let open: Vec<Value> = live
        .lock()
        .room()
        .conflicts
        .list()
        .map(|c| c["op"]["file"].clone())
        .collect();
    assert_eq!(open, [json!("b.md")]);
    assert_eq!(
        restore(&r, Some("acct"), "w", &json!({ "seq": 99 })).1,
        json!({ "error": "no such point" })
    );
    let _ = std::fs::remove_dir_all(&root);
}
