//! The review's fixes to `POST /changes` (3d): the lock's scope, the memo's bump, the 409 of another project,
//! the check-only `read`, and the format a set that writes seconds raises.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use k7_file_service::changes::plan::plan;
use k7_file_service::changes::request::parse;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::text_hash::text_hash;
use k7_file_service::watch::Feed;
use serde_json::{Value, json};

use super::changes::land;
use super::entries::calls;
use super::tests::{ctx, until};
use super::tests_journal::Bed;
use super::{Ctx, Tag, locks};

const LABEL: &str = "track rename ge.lead lead";

/// A watcher that never hears anything: the memo is trusted for its whole age.
struct Still;

impl Feed for Still {
    fn generation(&self, _root: &str) -> Option<u64> {
        Some(7)
    }
}

fn op(file: &str, text: &str, was: Value) -> Value {
    json!({ "kind": "text", "file": file, "text": text, "was": was })
}

// B1: the account's write lock is free while the stamp runs, as in `PUT /file`; a set that raises the format level completes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_write_lock_is_free_while_the_format_is_stamped() {
    let b = Arc::new(Bed::new("chf-lock").await);
    let held = calls::hold(&b.root, "stamp");
    let body = json!({ "at": "p.k7", "base": "x", "label": LABEL,
        "ops": [op("p.k7/a.k7x", "clip({ start: 1.5 })\n", json!(null))] });
    let mut body = body;
    body["base"] = json!(b.token());
    let b2 = b.clone();
    let running = tokio::spawn(async move { b2.changes(body).await });
    until("the bundle was told", || {
        calls::of(&b.root).iter().any(|c| c == "after")
    })
    .await;
    // The lock goes after the answer, which a debug build's first runtime takes seconds to give; held across the stamp it would never go.
    let studio = b.home().studio;
    let got = tokio::time::timeout(Duration::from_secs(20), locks::write(&studio)).await;
    assert!(got.is_ok(), "the write lock is held across the stamp");
    drop(got);
    held.add_permits(1);
    let (status, ans) = tokio::time::timeout(Duration::from_secs(30), running)
        .await
        .expect("the set completes")
        .unwrap();
    assert_eq!(status, 200, "{ans}");
    assert!(calls::of(&b.root).iter().any(|c| c == "stamp"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

// F4: with the watcher held still, the memo a `GET /revision` reads moves when a set lands; the landing's bump is what moves it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_landing_moves_the_memo_even_when_the_watcher_is_still() {
    let b = Bed::new("chf-memo").await;
    let c: Ctx = ctx(&b.dir, Tag::Changes, "", &b.lines);
    let revs = Revisions::with_feed(Box::new(Still), Instant::now);
    let (home, root) = (b.home(), b.gate_root());
    let first = revs.token(&home, &root, Some(&b.gate_project()), true);
    let memo = || revs.token(&home, &root, Some(&b.gate_project()), false);
    assert_eq!(memo(), first, "the memo holds while nothing is heard");
    let req = parse(&json!({ "at": "p.k7", "base": first, "label": LABEL,
        "ops": [op("p.k7/a.txt", "one", json!(null))] }))
    .unwrap();
    let planned = plan(&Space::open(&c.home, &c.root), &req.ops).unwrap();
    land(
        &c,
        &revs,
        &planned,
        &first,
        0,
        &k7_file_service::changes::target::Target::Project(b.gate_project()),
    );
    assert_eq!(b.file("a.txt"), "one");
    assert_ne!(memo(), first, "the memo still answers the old token");
    let _ = std::fs::remove_dir_all(&b.dir);
}

// The wrong `at` has its own word, after the gates: forbidden beats it, and it names the project that is open.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_project_is_open_is_its_own_409_after_the_gates() {
    let b = Bed::new("chf-open").await;
    let mut body = b.set(json!([op("p.k7/a.txt", "x", json!(null))]));
    body["at"] = json!("other.k7");
    let (status, ans) = b.changes(body.clone()).await;
    assert_eq!((status, ans["open"].clone()), (409, json!("p.k7")), "{ans}");
    assert_eq!(
        (ans["project"].clone(), ans["stale"].clone()),
        (json!(false), json!([]))
    );
    assert_eq!(ans["rev"], json!(b.token()));
    assert!(!b.has("a.txt"));
    body["ops"] = json!([op("../escape.txt", "x", json!(null))]);
    assert_eq!(
        b.changes(body).await.0,
        403,
        "forbidden beats the wrong project"
    );
    // A stale base on the right project has no `open`.
    let mut body = b.set(json!([]));
    body["base"] = json!("not-a-token");
    let (status, ans) = b.changes(body).await;
    assert_eq!((status, ans.get("open").is_none()), (409, true));
    let _ = std::fs::remove_dir_all(&b.dir);
}

// A `read` op judges a file the verb only read: stale makes the set 409 and nothing of it lands; a fresh one writes nothing itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_read_op_makes_a_stale_set_a_409_and_writes_nothing() {
    let b = Bed::new("chf-read").await;
    b.put("m.k7x", "m(1)\n");
    let read = |was: &str| json!({ "kind": "read", "file": "p.k7/m.k7x", "was": text_hash(was) });
    let write = op("p.k7/w.txt", "w", json!(null));
    let (status, ans) = b
        .changes(b.set(json!([read("m(0)\n"), write.clone()])))
        .await;
    assert_eq!(
        (status, ans["stale"].clone()),
        (409, json!(["p.k7/m.k7x"])),
        "{ans}"
    );
    assert!(!b.has("w.txt"));
    assert_eq!(b.steps().await, json!([]));
    let (status, ans) = b.changes(b.set(json!([read("m(1)\n"), write]))).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(b.file("m.k7x"), "m(1)\n");
    let (status, ans) = b.changes(b.set(json!([read("m(1)\n")]))).await;
    assert_eq!((status, ans["changed"].clone()), (200, json!([])), "{ans}");
    assert_eq!(ans["rev"], ans["revBefore"]);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// O2: a change set that writes a lane in seconds raises the project's `format` natively.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_set_that_writes_seconds_raises_the_format() {
    let b = Bed::new("chf-seconds").await;
    std::fs::write(
        Path::new(&b.project).join("project.k7x"),
        "project({ name: \"p\" })\n",
    )
    .unwrap();
    let lane = "extends(\"@meta/track\")\nevents([ref(\"hit\").start(4.5)])\n";
    let mut body = json!({ "at": "p.k7", "base": "x", "label": LABEL,
        "ops": [op("p.k7/nodes/fx.k7x", lane, json!(null))] });
    body["base"] = json!(b.token());
    let (status, ans) = b.changes(body).await;
    assert_eq!(status, 200, "{ans}");
    assert!(b.file("project.k7x").contains("format(2)"));
    let _ = std::fs::remove_dir_all(&b.dir);
}
