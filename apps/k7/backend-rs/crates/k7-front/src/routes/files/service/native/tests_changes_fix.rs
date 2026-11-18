//! The review's fixes to `POST /changes` and the bundle routes (3d): the lock's scope, the memo's bump, the 409 of another project,
//! the check-only `read`, and the library route's cache scope, lengths and virtual paths.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hyper::header::CACHE_CONTROL;
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
use super::tests_account::{call, text};
use super::tests_docs::{answering, world};
use super::tests_journal::Bed;
use super::{Ctx, Tag, locks};
use crate::routes::files::service::routes::{self, H};

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
    let running = tokio::spawn(async move { b2.changes(false, body).await });
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
    let (status, ans) = b.changes(false, body.clone()).await;
    assert_eq!((status, ans["open"].clone()), (409, json!("p.k7")), "{ans}");
    assert_eq!(
        (ans["project"].clone(), ans["stale"].clone()),
        (json!(false), json!([]))
    );
    assert_eq!(ans["rev"], json!(b.token()));
    assert!(!b.has("a.txt"));
    body["ops"] = json!([op("../escape.txt", "x", json!(null))]);
    assert_eq!(
        b.changes(false, body).await.0,
        403,
        "forbidden beats the wrong project"
    );
    // A stale base on the right project has no `open`.
    let mut body = b.set(json!([]));
    body["base"] = json!("not-a-token");
    let (status, ans) = b.changes(false, body).await;
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
        .changes(false, b.set(json!([read("m(0)\n"), write.clone()])))
        .await;
    assert_eq!(
        (status, ans["stale"].clone()),
        (409, json!(["p.k7/m.k7x"])),
        "{ans}"
    );
    assert!(!b.has("w.txt"));
    assert_eq!(b.steps().await, json!([]));
    let (status, ans) = b
        .changes(false, b.set(json!([read("m(1)\n"), write])))
        .await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(b.file("m.k7x"), "m(1)\n");
    let (status, ans) = b.changes(false, b.set(json!([read("m(1)\n")]))).await;
    assert_eq!((status, ans["changed"].clone()), (200, json!([])), "{ans}");
    assert_eq!(ans["rev"], ans["revBefore"]);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// F1, F3, B2 through the routes: the library answers `private`, with its lengths, under a virtual root; the bundle shows no folder.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_library_route_is_private_serves_lengths_and_the_bundle_is_virtual() {
    let (dir, acct) = world("chf-lib").await;
    let lines = super::tests::Lines::default();
    let home = ctx(&dir, Tag::VerbBundle, "", &lines).home;
    let wav = Path::new(&home.account_home()).join("samples/kick.wav");
    std::fs::create_dir_all(wav.parent().unwrap()).unwrap();
    std::fs::write(&wav, tiny_wav()).unwrap();
    let ask = |tag: Tag, query: &str| {
        let c = answering(&acct, &dir, tag, &lines);
        call(&dir, query, json!({}), Some(c))
    };
    let resp = routes::dispatch(H::VerbBundle, &ask(Tag::VerbBundle, "at=p.k7")).await;
    let (status, body) = text(resp).await;
    assert_eq!(status, 200, "{body}");
    assert!(
        !body.contains(&dir.display().to_string()),
        "a server folder reached the browser"
    );
    let bundle: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(bundle["workspaceRoot"], json!("/w"));
    let lib = bundle["libraries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "account")
        .unwrap()
        .clone();
    assert_eq!(lib["root"], json!("/l/account"));
    let hash = lib["hash"].as_str().unwrap();
    let q = format!("at=p.k7&name=account&hash={hash}");
    let resp = routes::dispatch(H::VerbLibrary, &ask(Tag::VerbLibrary, &q)).await;
    let cache = resp
        .headers()
        .get(CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let (status, body) = text(resp).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        cache.as_deref(),
        Some("private, max-age=31536000, immutable")
    );
    let served: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(served["lengths"]["samples/kick.wav"], json!(0.1));
    assert_eq!(served["files"]["samples/kick.wav"]["binary"], json!(true));
    let q = "at=p.k7&name=account&hash=nope";
    let resp = routes::dispatch(H::VerbLibrary, &ask(Tag::VerbLibrary, q)).await;
    assert_eq!(text(resp).await.0, 404);
    let _ = std::fs::remove_dir_all(&dir);
}

// Z1-3: the bundle of the workspace alone holds the shared texts and no project; a chain of another account's library is a 403.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_bundle_of_the_workspace_alone_and_of_a_chain() {
    let (dir, acct) = world("chf-targets").await;
    let lines = super::tests::Lines::default();
    let ask = |query: &str| {
        let c = answering(&acct, &dir, Tag::VerbBundle, &lines);
        call(&dir, query, json!({}), Some(c))
    };
    let (status, body) = text(routes::dispatch(H::VerbBundle, &ask("at=")).await).await;
    assert_eq!(status, 200, "{body}");
    let bundle: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(bundle["at"], json!(""));
    assert!(bundle["rev"].as_str().unwrap().starts_with("none:"));
    let files = bundle["source"]["files"].as_object().unwrap();
    assert!(
        files.keys().all(|k| !k.starts_with("/w/p.k7/")),
        "the project's own files are not in a workspace-alone bundle"
    );
    let (status, _) =
        text(routes::dispatch(H::VerbBundle, &ask("at=_libraries/nobody/chains/x")).await).await;
    assert_eq!(status, 403);
    let (status, _) = text(routes::dispatch(H::VerbBundle, &ask("at=other.k7")).await).await;
    assert_eq!(status, 409);
    let (status, _) = text(routes::dispatch(H::VerbBundle, &ask("")).await).await;
    assert_eq!(status, 400);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A 16-bit mono 8 kHz WAV of 800 samples: a tenth of a second.
fn tiny_wav() -> Vec<u8> {
    let data = 1600u32;
    let mut b = b"RIFF".to_vec();
    b.extend((36 + data).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(8000u32.to_le_bytes());
    b.extend(16000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(data.to_le_bytes());
    b.extend(vec![0u8; data as usize]);
    b
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
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 200, "{ans}");
    assert!(b.file("project.k7x").contains("format(2)"));
    let _ = std::fs::remove_dir_all(&b.dir);
}
