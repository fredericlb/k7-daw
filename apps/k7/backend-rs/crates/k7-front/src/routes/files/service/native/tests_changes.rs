//! `POST /changes` through a real account and `dispatch` (3d): a set lands whole as one journal step and one relay batch, the stale
//! rules, the refusals that write nothing, the limits and the admin-only libraries.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use k7_capture::Captures;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::text_hash::text_hash;
use serde_json::{Value, json};

use super::Tag;
use super::entries::calls;
use super::tests::until;
use super::tests_account::text;
use super::tests_capture::host;
use super::tests_entries::request;
use super::tests_journal::Bed;
use crate::routes::files::service::routes::{self, H};

const LABEL: &str = "track rename ge.lead lead";

impl Bed {
    /// The project's token, walked now: the base a browser would hold.
    pub(super) fn token(&self) -> String {
        Revisions::global().token(
            &self.home(),
            &self.gate_root(),
            Some(&self.gate_project()),
            true,
        )
    }

    pub(super) async fn changes(&self, admin: bool, body: Value) -> (u16, Value) {
        let lines = &self.lines;
        let c = request(&self.dir, Tag::Changes, body, admin, lines);
        let (status, t) = text(routes::dispatch(H::Changes, &c).await).await;
        (status, serde_json::from_str(&t).unwrap_or(Value::Null))
    }

    /// A body whose base is the token now.
    pub(super) fn set(&self, ops: Value) -> Value {
        json!({ "at": "p.k7", "base": self.token(), "label": LABEL, "ops": ops })
    }

    pub(super) fn put(&self, rel: &str, text: &str) {
        std::fs::write(Path::new(&self.project).join(rel), text).unwrap();
    }

    pub(super) fn has(&self, rel: &str) -> bool {
        Path::new(&self.project).join(rel).exists()
    }

    pub(super) async fn steps(&self) -> Value {
        self.ans(Tag::HistoryEntries, json!({})).await.1
    }
}

fn text_op(file: &str, text: &str, was: Value) -> Value {
    json!({ "kind": "text", "file": file, "text": text, "was": was })
}

// 3-D7, 3-B2: one set is one journal step labelled with the verb line, undo restores every file of it, and the answer's `rev`
// is the next set's base.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_set_is_one_step_that_undo_restores_whole() {
    let b = Bed::new("ch-step").await;
    b.put("a.k7x", "a(1)\n");
    b.put("gone.txt", "bye");
    let was = |t: &str| json!(text_hash(t));
    let body = b.set(json!([
        text_op("p.k7/a.k7x", "a(2)\n", was("a(1)\n")),
        text_op("p.k7/new/b.txt", "b", json!(null)),
        { "kind": "remove", "file": "p.k7/gone.txt", "was": was("bye") },
    ]));
    let before = body["base"].clone();
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(ans["revBefore"], before);
    assert_ne!(ans["rev"], before);
    let mut changed: Vec<String> = ans["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            format!(
                "{}:{}",
                c["path"].as_str().unwrap(),
                c["kind"].as_str().unwrap()
            )
        })
        .collect();
    changed.sort();
    assert_eq!(
        changed,
        ["a.k7x:edited", "gone.txt:removed", "new/b.txt:added"]
    );
    assert_eq!(
        (b.file("a.k7x"), b.file("new/b.txt"), b.has("gone.txt")),
        ("a(2)\n".into(), "b".into(), false)
    );
    let steps = b.steps().await;
    assert_eq!(steps.as_array().map(Vec::len), Some(1), "{steps}");
    assert_eq!(steps[0]["label"], json!(LABEL));
    // The answer's token is the project's token: the next set rests on it.
    assert_eq!(ans["rev"], json!(b.token()));
    // The memo a `GET /revision` reads agrees: the landing bumped it.
    let memo = Revisions::global().token(&b.home(), &b.gate_root(), Some(&b.gate_project()), false);
    assert_eq!(ans["rev"], json!(memo));
    let next = json!({ "at": "p.k7", "base": ans["rev"], "label": "again", "ops": [text_op("p.k7/a.k7x", "a(3)\n", was("a(2)\n"))] });
    assert_eq!(b.changes(false, next).await.0, 200);
    assert_eq!(b.ans(Tag::Undo, json!({})).await.1["applied"], json!(true));
    assert_eq!(b.file("a.k7x"), "a(2)\n");
    assert_eq!(b.ans(Tag::Undo, json!({})).await.1["applied"], json!(true));
    assert_eq!(
        (b.file("a.k7x"), b.has("new/b.txt"), b.file("gone.txt")),
        ("a(1)\n".into(), false, "bye".into())
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 3-B2: a base that is not the token now, and a `was` that moved, are each a 409 that names what moved; nothing is written or journalled.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stale_base_or_was_is_a_409_that_writes_and_journals_nothing() {
    let b = Bed::new("ch-stale").await;
    b.put("a.k7x", "disk\n");
    let ops = json!([
        text_op("p.k7/a.k7x", "mine(1)\n", json!(text_hash("disk\n"))),
        text_op("p.k7/z.txt", "z", json!(null))
    ]);
    let mut body = b.set(ops.clone());
    body["base"] = json!("not-a-token");
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 409, "{ans}");
    assert_eq!(
        (
            ans["stale"].clone(),
            ans["project"].clone(),
            ans["rev"].clone()
        ),
        (json!([]), json!(true), json!(b.token()))
    );
    // A file moved since it was read, under a token that is right: it is named.
    let body = b.set(json!([
        text_op("p.k7/a.k7x", "mine(1)\n", json!(text_hash("opened\n"))),
        text_op("p.k7/z.txt", "z", json!(null))
    ]));
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(
        (status, ans["stale"].clone(), ans["project"].clone()),
        (409, json!(["p.k7/a.k7x"]), json!(false))
    );
    assert_eq!((b.file("a.k7x"), b.has("z.txt")), ("disk\n".into(), false));
    assert_eq!(b.steps().await, json!([]));
    assert_eq!(b.changes(false, b.set(ops)).await.0, 200);
    assert_eq!(b.file("a.k7x"), "mine(1)\n");
    // Another project is not the open one.
    let mut body = b.set(json!([]));
    body["at"] = json!("other.k7");
    assert_eq!(b.changes(false, body).await.0, 409);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 3-D7: a set refused for any reason, or failing while it lands, writes nothing: no file, no folder, no step.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_or_a_failure_midway_writes_nothing() {
    let b = Bed::new("ch-atomic").await;
    b.put("block", "a file");
    let first = text_op("p.k7/ok/first.txt", "1", json!(null));
    for (second, want) in [
        (text_op("../escape.txt", "x", json!(null)), 403),
        (text_op("p.k7/.history/x", "x", json!(null)), 403),
        (
            text_op("p.k7/n.k7x", "extends(\"unterminated\n", json!(null)),
            422,
        ),
        (text_op("p.k7/a.wav", "x", json!(null)), 400),
        // A file in the way: ENOTDIR on posix, ENOENT (path not found) on Windows, as node says it.
        (
            text_op("p.k7/block/under.txt", "x", json!(null)),
            if cfg!(windows) { 404 } else { 400 },
        ),
    ] {
        let (status, ans) = b
            .changes(false, b.set(json!([first.clone(), second.clone()])))
            .await;
        assert_eq!(status, want, "{second}: {ans}");
        assert!(ans["error"].is_string());
        assert!(!b.has("ok") && !b.has("n.k7x"), "{second}");
    }
    assert_eq!(b.steps().await, json!([]));
    let (_, ans) = b
        .changes(
            false,
            b.set(json!([
                first.clone(),
                text_op("p.k7/n.k7x", "extends(\"unterminated\n", json!(null))
            ])),
        )
        .await;
    assert_eq!(ans["file"], json!("p.k7/n.k7x"));
    assert!(ans["error"].as_str().unwrap().starts_with("p.k7/n.k7x: "));
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 3-B8: 2,001 ops, and text past 8 MiB, are a 413 with a sentence; nothing is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_set_past_its_limits_is_a_413() {
    let b = Bed::new("ch-limits").await;
    let many: Vec<Value> = (0..=2_000)
        .map(|i| text_op(&format!("p.k7/m/{i}.txt"), "x", json!(null)))
        .collect();
    let (status, ans) = b.changes(false, b.set(Value::Array(many))).await;
    assert_eq!(status, 413, "{ans}");
    assert_eq!(ans["error"], json!("a change set carries at most 2000 ops"));
    let big = "x".repeat((8 << 20) + 1);
    let (status, _) = b
        .changes(
            false,
            b.set(json!([text_op("p.k7/big.txt", &big, json!(null))])),
        )
        .await;
    assert_eq!(status, 413);
    assert!(!b.has("m") && !b.has("big.txt"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 3-B8, the library gate: a library is written by an admin, and by no one else, inside a set.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_library_is_written_by_an_admin_alone() {
    let b = Bed::new("ch-libs").await;
    let lib = b.dir.join("libraries/k7-core/modules/audio/verb");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("module.k7x"), "m(1)\n").unwrap();
    let ops = json!([
        text_op("p.k7/in-project.txt", "p", json!(null)),
        text_op(
            "_libraries/k7-core/modules/audio/verb/module.k7x",
            "m(2)\n",
            json!(text_hash("m(1)\n"))
        ),
    ]);
    let (status, ans) = b.changes(false, b.set(ops.clone())).await;
    assert_eq!(status, 403, "{ans}");
    assert!(
        ans["error"]
            .as_str()
            .unwrap()
            .contains("only the origin's admins edit a library")
    );
    assert!(!b.has("in-project.txt"), "the whole set is refused");
    assert_eq!(
        std::fs::read_to_string(lib.join("module.k7x")).unwrap(),
        "m(1)\n"
    );
    let (status, ans) = b.changes(true, b.set(ops)).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(
        std::fs::read_to_string(lib.join("module.k7x")).unwrap(),
        "m(2)\n"
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

// The engine is told what the set did, and the engine is pushed only when the set asks and moved a file of the open project.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_engine_is_pushed_on_request_and_the_bundle_told_once() {
    let b = Bed::new("ch-push").await;
    let (status, _) = b
        .changes(
            false,
            b.set(json!([text_op("p.k7/a.k7x", "x(1)\n", json!(null))])),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(calls::of(&b.root), ["hold", "after", "release"]);
    let mut body = b.set(json!([text_op("p.k7/a.k7x", "x(2)\n", json!(null))]));
    body["base"] = json!(b.token());
    body["reload"] = json!(true);
    body["ops"][0]["was"] = json!(text_hash("x(1)\n"));
    assert_eq!(b.changes(false, body).await.0, 200);
    let all = calls::of(&b.root);
    assert_eq!(all.iter().filter(|c| *c == "push").count(), 1, "{all:?}");
    // A set that moves nothing pushes nothing.
    let mut same = b.set(json!([text_op("p.k7/a.k7x", "x(2)\n", json!(null))]));
    same["reload"] = json!(true);
    assert_eq!(b.changes(false, same).await.0, 200);
    assert_eq!(
        calls::of(&b.root).iter().filter(|c| *c == "push").count(),
        1
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 3-D7: the whole set is ONE batch of the capture's log, named with the verb line, never one per file.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_set_is_captured_as_one_batch_named_by_the_verb_line() {
    let b = Bed::new("ch-relay").await;
    let home = crate::routes::files::service::native::tests::slashed(&b.dir.join("home"));
    let root = b.gate_root();
    let rt = tokio::runtime::Handle::current();
    let acct = Captures::global()
        .start_account(Arc::new(|_| {}), host(&home), &rt)
        .await;
    acct.attach(&root, "songs").await.unwrap().unwrap();
    let tail = || {
        let a = acct.attached_at(&root)?;
        a.log.tail(100.0).ok()
    };
    let count = || tail().map_or(0, |l| l.len());
    let first = count();
    let ops = json!([
        text_op("p.k7/r1.txt", "one", json!(null)),
        text_op("p.k7/r2.txt", "two", json!(null)),
        text_op("p.k7/r3.txt", "three", json!(null)),
    ]);
    assert_eq!(b.changes(false, b.set(ops)).await.0, 200);
    until("the set's batch", || count() > first).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    let log = tail().unwrap();
    let mine: Vec<&Value> = log.iter().skip(first).collect();
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0]["label"], json!(LABEL));
    assert_eq!(
        mine[0]["ops"].as_array().map(Vec::len),
        Some(3),
        "{:?}",
        mine[0]
    );
    Captures::global().stop_account(&home).await;
    let _ = std::fs::remove_dir_all(&b.dir);
}

// The three routes are in the table, belong to the journal piece, and without the compiler the route says so rather than guess.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_routes_are_named_and_a_bundle_without_the_compiler_is_a_503() {
    use hyper::Method;
    assert_eq!(
        routes::route(&Method::POST, "/changes"),
        routes::Route::Handler(H::Changes)
    );
    assert_eq!(
        routes::route(&Method::GET, "/verb-bundle"),
        routes::Route::Handler(H::VerbBundle)
    );
    assert_eq!(
        routes::route(&Method::GET, "/verb-bundle/library"),
        routes::Route::Handler(H::VerbLibrary)
    );
    assert_eq!(
        routes::route(&Method::GET, "/changes"),
        routes::Route::Hooked
    );
    for t in [Tag::Changes, Tag::VerbBundle, Tag::VerbLibrary] {
        assert_eq!(t.piece(), super::Piece::Journal);
    }
    let b = Bed::new("ch-table").await;
    let lines = &b.lines;
    let c = request(&b.dir, Tag::VerbBundle, json!({}), false, lines);
    let (status, _) = text(routes::dispatch(H::VerbBundle, &c).await).await;
    assert_eq!(status, 503);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// A 200's `rev` is the token the project has afterwards: an empty set at it is fresh, whatever the set wrote (the Worker's rerun).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_answers_rev_is_still_the_token_after_the_journal_ends() {
    let b = Bed::new("ch-rev").await;
    // The apply bumps the revision after the landing, as the engine's push does.
    let root = Space::open(&b.home(), &b.gate_root()).root().to_string();
    let bump_root = root.clone();
    super::changes::tests_hooks::on_push(&root, move || Revisions::global().bump(&bump_root));
    for (n, (file, text)) in [
        ("p.k7/project.k7x", "project({ name: \"p\", tempo: 121 })\n"),
        ("p.k7/project.k7x", "project({ name: \"p\", tempo: 122 })\n"),
        ("p.k7/nodes/n.k7x", "extends(\"@meta/track\")\n"),
    ]
    .into_iter()
    .enumerate()
    {
        let on_disk = Path::new(&b.project).join(file.trim_start_matches("p.k7/"));
        let was = std::fs::read_to_string(on_disk).ok().map(|t| text_hash(&t));
        let mut body = b.set(json!([text_op(file, text, json!(was))]));
        body["reload"] = json!(true);
        let (status, ans) = b.changes(false, body).await;
        assert_eq!(status, 200, "set {n}: {ans}");
        let empty = json!({ "at": "p.k7", "base": ans["rev"], "label": "probe", "ops": [] });
        let (status, again) = b.changes(false, empty).await;
        assert_eq!(
            status, 200,
            "set {n}: an empty set at the answer's rev: {again}"
        );
    }
    let _ = std::fs::remove_dir_all(&b.dir);
}

// Z1-4: `journal: "skip"` lands the set and writes no step; the answer still carries the token (`graph pin`).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_skipped_set_lands_without_a_step() {
    let b = Bed::new("ch-skip").await;
    let mut body = b.set(json!([text_op("p.k7/a.k7x", "pin(1)\n", json!(null))]));
    body["journal"] = json!("skip");
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(b.file("a.k7x"), "pin(1)\n");
    assert_eq!(b.steps().await.as_array().map(Vec::len), Some(0));
    assert_eq!(ans["rev"], json!(b.token()));
    let mut bad = b.set(json!([]));
    bad["journal"] = json!("other");
    assert_eq!(b.changes(false, bad).await.0, 400);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// Z1-3: an empty `at` is the workspace alone (shared texts only), and `_libraries/…` names a chain the person writes (an admin).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_empty_at_writes_the_shared_texts_and_a_chain_is_the_admins() {
    let b = Bed::new("ch-targets").await;
    let none = || Revisions::global().token(&b.home(), &b.gate_root(), None, false);
    let alone = |ops: Value, base: String| json!({ "at": "", "base": base, "label": "library upgrade", "ops": ops });
    // The shared texts land, with no step, and the answer's rev is the next base.
    let ok = alone(
        json!([text_op("_playlists/a.txt", "one", json!(null))]),
        none(),
    );
    let (status, ans) = b.changes(false, ok).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(ans["rev"], json!(none()));
    assert_eq!(
        std::fs::read_to_string(Path::new(&b.project).join("../_playlists/a.txt"))
            .ok()
            .as_deref(),
        Some("one")
    );
    assert_eq!(b.steps().await.as_array().map(Vec::len), Some(0));
    // A project's file is not a shared text.
    let bad = alone(json!([text_op("p.k7/x.txt", "x", json!(null))]), none());
    let (status, ans) = b.changes(false, bad).await;
    assert_eq!(status, 403, "{ans}");
    assert!(!b.has("x.txt"));
    // A name that climbs out of a shared folder is judged where it lands, not as it is spelled.
    b.put("a.k7x", "keep(1)\n");
    for name in ["_playlists/../p.k7/a.k7x", "_modules\\..\\p.k7\\a.k7x"] {
        let climb = alone(json!([text_op(name, "a(2)\n", json!(null))]), none());
        let (status, ans) = b.changes(false, climb).await;
        assert_eq!(status, 403, "{name}: {ans}");
    }
    // A move deletes its source: the source must be a shared text too.
    let mv = alone(
        json!([{ "kind": "move", "from": "p.k7/a.k7x", "file": "_modules/x.k7x" }]),
        none(),
    );
    assert_eq!(b.changes(false, mv).await.0, 403);
    assert_eq!(b.file("a.k7x"), "keep(1)\n");
    // A chain of a library: forbidden to a person who is not its admin, written by the admin, and it is the chain's token.
    let chain = b.dir.join("libraries/k7-core/chains/c1");
    std::fs::create_dir_all(&chain).unwrap();
    std::fs::write(chain.join("chain.k7x"), "c(1)\n").unwrap();
    let at = "_libraries/k7-core/chains/c1";
    let chain_set = |base: String| {
        json!({ "at": at, "base": base, "label": "graph pin", "ops": [
        text_op("_libraries/k7-core/chains/c1/chain.k7x", "c(2)\n", json!(text_hash("c(1)\n")))] })
    };
    let tok = Revisions::global().token(
        &b.home(),
        &b.gate_root(),
        Some(&chain.display().to_string()),
        true,
    );
    assert_eq!(b.changes(false, chain_set(tok.clone())).await.0, 403);
    // The target alone is gated, whatever the ops: an empty set on the chain is the same 403.
    let empty = json!({ "at": at, "base": tok, "label": "graph pin", "ops": [] });
    assert_eq!(b.changes(false, empty).await.0, 403);
    let (status, ans) = b.changes(true, chain_set(tok)).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(
        std::fs::read_to_string(chain.join("chain.k7x")).unwrap(),
        "c(2)\n"
    );
    // A chain set may not write the open project either (admin or not).
    let out = json!({ "at": at, "base": "x", "label": "graph pin", "ops": [
        text_op("p.k7/a.k7x", "a(2)\n", json!(null))] });
    assert_eq!(b.changes(true, out).await.0, 403);
    assert_eq!(b.file("a.k7x"), "keep(1)\n");
    // A name that is neither the open project nor a library is the other project's 409.
    let other = json!({ "at": "other.k7", "base": b.token(), "label": "x", "ops": [] });
    assert_eq!(b.changes(false, other).await.0, 409);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// A second writer inside the push is not ours: the answer's rev stays the one the journal ended on, so a set based on it is stale.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_writer_inside_the_push_is_not_taken_into_the_answers_rev() {
    let b = Bed::new("ch-race").await;
    let root = Space::open(&b.home(), &b.gate_root()).root().to_string();
    let project = b.project.clone();
    let r2 = root.clone();
    super::changes::tests_hooks::on_push(&root, move || {
        // Another tab's commit lands while the push is awaited: a file, and two announcements.
        std::fs::write(Path::new(&project).join("other.k7x"), "other(1)\n").unwrap();
        Revisions::global().bump(&r2);
        Revisions::global().bump(&r2);
    });
    let mut body = b.set(json!([text_op("p.k7/a.k7x", "a(1)\n", json!(null))]));
    body["reload"] = json!(true);
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 200, "{ans}");
    assert_ne!(
        ans["rev"],
        json!(b.token()),
        "the other tab's commit is in the token now, not in our answer"
    );
    let empty = json!({ "at": "p.k7", "base": ans["rev"], "label": "probe", "ops": [] });
    assert_eq!(b.changes(false, empty).await.0, 409);
    let _ = std::fs::remove_dir_all(&b.dir);
}
