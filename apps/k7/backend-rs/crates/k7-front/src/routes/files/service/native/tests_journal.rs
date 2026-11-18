//! The journal's five routes through a real account and `dispatch` (1700 to 1705, 1730 to 1736, 1760 to 1773, 1796):
//! a save and its step, the order of the lock, a client that leaves, the audio's sentences, and the counts.

use std::sync::Arc;
use std::time::Duration;

use k7_file_service::journal::{Journals, Mode as Bracket, Owner};
use k7_file_service::paths::resolve_all;
use k7_file_service::text_hash::text_hash;
use serde_json::{Value, json};

use super::Tag;
use super::entries::calls;
use super::tests::{Lines, counted, scratch};
use super::tests_account::text;
use super::tests_entries::request;
use crate::routes::files::service::routes::{self, H};

fn handler(tag: Tag) -> H {
    match tag {
        Tag::FilePut => H::FilePut,
        Tag::Undo => H::Undo,
        Tag::Redo => H::Redo,
        Tag::History => H::History,
        _ => H::HistoryEntries,
    }
}

pub(super) struct Bed {
    pub(super) dir: std::path::PathBuf,
    pub(super) lines: Lines,
    pub(super) root: String,
    pub(super) project: String,
}

impl Bed {
    pub(super) async fn new(name: &str) -> Bed {
        let dir = scratch(name);
        let songs = dir.join("home/workspace/songs");
        std::fs::create_dir_all(songs.join("p.k7")).unwrap();
        std::fs::write(songs.join("p.k7/project.k7x"), "project: p\n").unwrap();
        std::fs::create_dir_all(dir.join("home/workspace/.active-projects")).unwrap();
        std::fs::write(dir.join("home/workspace/.active-projects/songs"), "p.k7").unwrap();
        let root = songs.display().to_string();
        let project = super::tests::slashed(&songs.join("p.k7"));
        Bed {
            dir,
            lines: Lines::default(),
            root,
            project,
        }
    }

    pub(super) fn call(&self, tag: Tag, body: Value) -> routes::Call {
        request(&self.dir, tag, body, false, &self.lines)
    }

    pub(super) async fn go(&self, tag: Tag, body: Value) -> (u16, String) {
        text(routes::dispatch(handler(tag), &self.call(tag, body)).await).await
    }

    /// An answer of `tag` with `engine` as the account's link.
    pub(super) async fn ans_with(
        &self,
        tag: Tag,
        body: Value,
        engine: &Option<Arc<super::engine::routes::Engine>>,
    ) -> (u16, Value) {
        let mut call = self.call(tag, body);
        call.engine = engine.clone();
        let (s, t) = text(routes::dispatch(handler(tag), &call).await).await;
        (s, serde_json::from_str(&t).unwrap_or(Value::Null))
    }

    pub(super) async fn ans(&self, tag: Tag, body: Value) -> (u16, Value) {
        let (s, t) = self.go(tag, body).await;
        (s, serde_json::from_str(&t).unwrap_or(Value::Null))
    }

    pub(super) async fn save(&self, path: &str, content: &str) -> (u16, Value) {
        self.ans(Tag::FilePut, json!({ "path": path, "content": content }))
            .await
    }

    pub(super) fn file(&self, rel: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(&self.project).join(rel)).unwrap_or_default()
    }

    pub(super) fn gate_root(&self) -> String {
        resolve_all(&[&self.root])
    }

    /// The project as the handler spells it (slashes), the key its revision epoch is made under.
    pub(super) fn gate_project(&self) -> String {
        resolve_all(&[&self.project])
    }

    pub(super) fn home(&self) -> k7_file_service::libraries::Home {
        super::tests::ctx(&self.dir, Tag::History, "", &self.lines).home
    }

    pub(super) fn store(&self) -> String {
        k7_file_service::journal::store::store_for(&self.home(), &self.project)
    }

    pub(super) fn plant(&self, journal: &[u8]) {
        std::fs::create_dir_all(self.store()).unwrap();
        std::fs::write(format!("{}/journal.json", self.store()), journal).unwrap();
    }
}

fn journal_of(entry: &str) -> Vec<u8> {
    format!(r#"{{"nextId":2,"cursor":1,"entries":[{entry}]}}"#).into_bytes()
}

const PROCESS: &str = r#"{"id":1,"ts":1,"kind":"process","label":"bounce","changes":[],"blobs":[{"path":"p.k7/r.wav","before":"1-b0","after":"1-a0"}],"keptOn":{"id":"e1","name":"Studio"}}"#;

// 1760 to 1762, 1770, 1771, 1735: a save is a step, the engine is told in the TypeScript's order, undo and redo move it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_save_is_a_step_that_undo_and_redo_move_and_the_reads_follow() {
    let b = Bed::new("j-flow").await;
    assert_eq!(
        b.ans(Tag::History, json!({})).await,
        (200, json!({ "canUndo": false, "canRedo": false }))
    );
    assert_eq!(
        b.save("p.k7/a.k7x", "v1").await,
        (200, json!({ "ok": true }))
    );
    // The window, the memos, the window let go, the push: no format stamp for a text with no `start`.
    assert_eq!(calls::of(&b.root), ["hold", "after", "release", "push"]);
    assert_eq!(b.save("p.k7/a.k7x", "v2!").await.0, 200);
    let state = b.ans(Tag::History, json!({})).await.1;
    assert_eq!(state["canUndo"], json!(true));
    assert_eq!(state["nextUndo"]["label"], json!("edit p.k7/a.k7x"));
    let list = b.ans(Tag::HistoryEntries, json!({})).await.1;
    assert_eq!(list.as_array().map(Vec::len), Some(2));
    assert_eq!(list[1]["paths"], json!(["a.k7x"]), "{list}");
    assert_eq!(list[1]["undone"], json!(false));
    let undone = b.ans(Tag::Undo, json!({})).await;
    assert_eq!(undone.1["applied"], json!(true), "{undone:?}");
    assert_eq!(b.file("a.k7x"), "v1");
    assert_eq!(b.ans(Tag::Redo, json!({})).await.1["applied"], json!(true));
    assert_eq!(b.file("a.k7x"), "v2!");
    assert_eq!(
        b.ans(Tag::Redo, json!({})).await.1,
        json!({ "applied": false })
    );
    // A body the schema refuses is the route's 400.
    assert_eq!(b.ans(Tag::Undo, json!({ "confirm": "yes" })).await.0, 400);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1760: a stale base is a 409 with nothing written and nothing journalled; the right base writes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stale_base_is_a_409_and_a_right_one_writes() {
    let b = Bed::new("j-base").await;
    std::fs::write(std::path::Path::new(&b.project).join("a.k7x"), "disk").unwrap();
    let put = |content: &str, base: &str| {
        b.ans(
            Tag::FilePut,
            json!({ "path": "p.k7/a.k7x", "content": content, "base": base }),
        )
    };
    let (status, body) = put("mine", &text_hash("opened")).await;
    assert_eq!(status, 409);
    assert_eq!(body["stale"], json!(true));
    assert_eq!(b.file("a.k7x"), "disk");
    assert_eq!(
        b.ans(Tag::History, json!({})).await.1["canUndo"],
        json!(false)
    );
    assert_eq!(put("mine", &text_hash("disk")).await.0, 200);
    assert_eq!(b.file("a.k7x"), "mine");
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1764, mutation "the format stamp asked on every save": only a `.k7x` that may place an event in seconds asks.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_format_stamp_is_asked_only_for_a_lane_that_may_need_it() {
    let b = Bed::new("j-stamp").await;
    let asked = || calls::of(&b.root).iter().filter(|c| *c == "stamp").count();
    b.save("p.k7/a.k7x", "events: []").await;
    b.save("p.k7/notes.txt", "start").await;
    assert_eq!(asked(), 0);
    b.save("p.k7/a.k7x", "events: [{start: 1.5}]").await;
    assert_eq!(asked(), 1);
    std::fs::write(
        std::path::Path::new(&b.project).join("project.k7x"),
        "format(2)\n",
    )
    .unwrap();
    b.save("p.k7/a.k7x", "events: [{start: 2.5}]").await;
    assert_eq!(asked(), 1, "a project at the level asks nothing");
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1733: an undo waiting on a record while a save waits on the undo ends: the gate first, the account's lock after it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_undo_waiting_on_a_record_and_a_save_waiting_on_the_undo_all_end() {
    let b = Arc::new(Bed::new("j-order").await);
    b.save("p.k7/a.k7x", "v1").await;
    let j = Journals::global();
    let open = j
        .begin(
            &b.home(),
            &b.gate_root(),
            Some(&b.project),
            "verb",
            Bracket::Skip,
            Owner::mint(),
        )
        .await;
    let undo = tokio::spawn({
        let b = b.clone();
        async move { b.ans(Tag::Undo, json!({})).await }
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    let save = tokio::spawn({
        let b = b.clone();
        async move { b.save("p.k7/b.k7x", "w").await }
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!undo.is_finished() && !save.is_finished());
    j.end_now(open.id, None);
    let both = tokio::time::timeout(Duration::from_secs(20), async { (undo.await, save.await) })
        .await
        .expect("a deadlock");
    assert_eq!(both.0.unwrap().1["applied"], json!(true));
    assert_eq!(both.1.unwrap().0, 200);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1736: a client that leaves after the write began still has its step journalled.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_whose_client_left_is_still_journalled() {
    let b = Bed::new("j-hangup").await;
    let release = calls::hold(&b.root, "after");
    let call = b.call(
        Tag::FilePut,
        json!({ "path": "p.k7/a.k7x", "content": "gone" }),
    );
    let mut put = Box::pin(routes::dispatch(H::FilePut, &call));
    let t = std::time::Instant::now();
    loop {
        tokio::select! {
            _ = &mut put => panic!("the write waits on the held memo call"),
            () = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        if b.file("a.k7x") == "gone" {
            break;
        }
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "the file was never written"
        );
    }
    drop(put);
    release.add_permits(1);
    let (j, home, root, project) = (
        Journals::global(),
        b.home(),
        b.gate_root(),
        b.project.clone(),
    );
    // A loaded host serves a whole suite's accounts: the wait is a backstop, the step is what is judged.
    let t = std::time::Instant::now();
    while j.state_of(&home, &root, &project)["canUndo"] != json!(true) {
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "the step was never journalled"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(b.file("a.k7x"), "gone");
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1744, 1790: an audio no longer kept, or kept by an engine not bound, stops the undo as a 409 with its sentence.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_undo_stopped_for_its_audio_is_a_409_with_the_sentence() {
    let b = Bed::new("j-audio").await;
    // The engine in use is not the one that keeps the audio.
    let a = super::engine::tests_real::account("j-audio-link").await;
    a.bind(&json!({ "url": a.fake.url, "engineId": "e2" }));
    let link = a.engine.clone();
    b.plant(&journal_of(PROCESS));
    let (status, body) = b.ans(Tag::Undo, json!({})).await;
    assert_eq!((status, &body["needsConfirm"]), (200, &json!(true)));
    let (status, body) = b
        .ans_with(Tag::Undo, json!({ "confirm": true }), &link)
        .await;
    assert_eq!(status, 409);
    assert_eq!(
        body["error"],
        json!(
            "The audio of p.k7/r.wav for this step is kept by the engine \"Studio\", not by the one in use. Switch back to \"Studio\" to go past this step."
        )
    );
    b.plant(&journal_of(
        &PROCESS.replace(r#","keptOn":{"id":"e1","name":"Studio"}"#, ""),
    ));
    let (status, body) = b
        .ans_with(Tag::Undo, json!({ "confirm": true }), &link)
        .await;
    assert_eq!(
        (
            status,
            body["error"]
                .as_str()
                .map(|s| s.ends_with("is no longer kept."))
        ),
        (409, Some(true))
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1743: a fetch that hangs holds no gate, so a record begins while it waits.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_fetch_that_hangs_does_not_hold_the_gate_a_record_begins_on() {
    let b = Arc::new(Bed::new("j-fetch").await);
    b.plant(&journal_of(PROCESS));
    // The machine that keeps the audio takes the connection and never answers the job.
    let hang = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let jobs = format!("ws://{}", hang.local_addr().unwrap());
    let held = tokio::spawn(async move {
        let mut kept = Vec::new();
        while let Ok((sock, _)) = hang.accept().await {
            kept.push(sock);
        }
    });
    let a = super::engine::tests_real::account("j-fetch-link").await;
    a.bind(&json!({ "url": a.fake.url, "engineId": "e1", "jobsUrl": jobs }));
    let link = a.engine.clone();
    let undo = tokio::spawn({
        let b = b.clone();
        async move {
            b.ans_with(Tag::Undo, json!({ "confirm": true }), &link)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !undo.is_finished(),
        "the fetch did not wait for its machine"
    );
    let j = Journals::global();
    let begun = tokio::time::timeout(
        Duration::from_secs(10),
        j.begin(
            &b.home(),
            &b.gate_root(),
            Some(&b.project),
            "verb",
            Bracket::Skip,
            Owner::mint(),
        ),
    )
    .await
    .expect("the fetch holds the gate");
    j.abort(begun.id);
    undo.abort();
    held.abort();
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1796: a journal read that repaired bad text is counted once per read of the file, not once per use.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lossy_journal_read_is_counted_once_per_read_of_the_file() {
    let b = Bed::new("j-lossy").await;
    let entry =
        b"{\"id\":1,\"ts\":1,\"kind\":\"text\",\"label\":\"a\xffb\",\"changes\":[],\"blobs\":[]}";
    let journal = [
        &b"{\"nextId\":2,\"cursor\":1,\"entries\":["[..],
        entry,
        b"]}",
    ]
    .concat();
    b.plant(&journal);
    let list = b.ans(Tag::HistoryEntries, json!({})).await.1;
    assert_eq!(list[0]["label"], json!("a\u{fffd}b"));
    assert_eq!(counted(&super::stats::global().lossy), 1);
    b.ans(Tag::History, json!({})).await;
    b.ans(Tag::HistoryEntries, json!({})).await;
    assert_eq!(
        counted(&super::stats::global().lossy),
        1,
        "the same read, not counted again"
    );
    // A longer file is a new read.
    b.plant(&[&journal[..], b" "].concat());
    b.ans(Tag::History, json!({})).await;
    assert_eq!(counted(&super::stats::global().lossy), 2);
    assert!(super::stats::global().line().contains(" lossy=2 "));
    let _ = std::fs::remove_dir_all(&b.dir);
}
