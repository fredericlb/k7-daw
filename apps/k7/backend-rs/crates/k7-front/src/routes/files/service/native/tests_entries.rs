//! The entries through `dispatch` (1200 to 1215): the writes answered natively with the bridge's calls
//! in order, the write lock, and the retry that sleeps on no thread.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use k7_file_service::entries::EntryError;
use k7_file_service::entries::apply::Renamer;
use serde_json::{Value, json};

use super::entries::{calls, rename_retrying};
use super::tests::{Lines, ctx, scratch};
use super::tests_account::{call, text};
use super::{Tag, locks};
use crate::routes::files::service::routes::{self, H};

/// A call of `h` with `body` answered natively, for the workspace of `dir`.
pub(super) fn request(dir: &std::path::Path, tag: Tag, body: Value, lines: &Lines) -> routes::Call {
    let c = ctx(dir, tag, "", lines);
    call(dir, "", body, Some(c))
}

pub(super) fn handler(tag: Tag) -> H {
    match tag {
        Tag::Folder => H::Folder,
        Tag::Rename => H::Rename,
        Tag::Move => H::Move,
        _ => H::EntryDelete,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn answering_writes_the_disk_and_tells_the_bundle_before_and_after() {
    let dir = scratch("e-answer");
    let songs = dir.join("home/workspace/songs");
    std::fs::write(songs.join("a.txt"), "a").unwrap();
    std::fs::create_dir_all(songs.join("sub")).unwrap();
    let lines = Lines::default();
    let go = |tag: Tag, body: Value| {
        let c = request(&dir, tag, body, &lines);
        async move { text(routes::dispatch(handler(tag), &c).await).await }
    };
    assert_eq!(
        go(Tag::Folder, json!({ "dir": "sub", "name": "new" })).await,
        (200, r#"{"path":"sub/new"}"#.to_string())
    );
    assert!(songs.join("sub/new").is_dir());
    let root = songs.display().to_string();
    assert_eq!(calls::of(&root), ["after"]);
    assert_eq!(
        go(Tag::Rename, json!({ "path": "a.txt", "name": "b.txt" })).await,
        (200, r#"{"path":"b.txt"}"#.to_string())
    );
    assert_eq!(
        go(Tag::Move, json!({ "path": "b.txt", "toDir": "sub" })).await,
        (200, r#"{"path":"sub/b.txt"}"#.to_string())
    );
    assert_eq!(
        go(Tag::Delete, json!({ "path": "sub/b.txt" })).await,
        (200, r#"{"recycled":"_recycle_bin/b.txt"}"#.to_string())
    );
    assert!(songs.join("_recycle_bin/b.txt").is_file());
    // The bridge: nothing before a folder's make, a `before` and an `after` around each rename, move and recycle.
    assert_eq!(
        calls::of(&root),
        [
            "after", "before", "after", "before", "after", "before", "after"
        ]
    );
    // Refusals are the route's own words, and tell the bundle nothing.
    assert_eq!(
        go(Tag::Rename, json!({ "path": "none", "name": "x" })).await,
        (
            404,
            r#"{"error":"no such file or folder: none"}"#.to_string()
        )
    );
    assert_eq!(
        go(Tag::Folder, json!({ "dir": "sub", "name": "new" })).await,
        (400, r#"{"error":"already exists: sub/new"}"#.to_string())
    );
    assert_eq!(go(Tag::Folder, json!({ "dir": "sub" })).await.0, 400);
    assert_eq!(calls::of(&root).len(), 7);
    // A write to the disk that fails in the OS is worded without the server's path (1233).
    std::fs::write(songs.join("plain"), "x").unwrap();
    let (status, body) = go(Tag::Folder, json!({ "dir": "plain/x", "name": "y" })).await;
    assert_eq!(
        (status, body.contains(&dir.display().to_string())),
        (400, false),
        "{body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_library_module_is_not_renamed_and_its_refs_stay() {
    let dir = scratch("e-repoint");
    let songs = dir.join("home/workspace/songs");
    let module = dir.join("libraries/k7-core/modules/audio/verb");
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(module.join("module.k7x"), "extends(\"@engine/reverb\")\n").unwrap();
    std::fs::create_dir_all(songs.join("song.k7/nodes")).unwrap();
    std::fs::write(
        songs.join("song.k7/nodes/v.k7x"),
        "extends(\"@k7-core/audio/verb\")\n",
    )
    .unwrap();
    std::fs::create_dir_all(songs.join("song.k7")).unwrap();
    let marker = dir.join("home/workspace/.active-projects");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("songs"), "song.k7").unwrap();
    let lines = Lines::default();
    let c = request(
        &dir,
        Tag::Rename,
        json!({ "path": "_libraries/k7-core/modules/audio/verb", "name": "hall" }),
        &lines,
    );
    let (status, body) = text(routes::dispatch(H::Rename, &c).await).await;
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("a library is not edited from here"), "{body}");
    assert_eq!(
        std::fs::read_to_string(songs.join("song.k7/nodes/v.k7x")).unwrap(),
        "extends(\"@k7-core/audio/verb\")\n"
    );
    assert!(module.join("module.k7x").is_file());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn sixty_four_renames_of_one_workspace_are_served_one_at_a_time() {
    let dir = scratch("e-64");
    let songs = dir.join("home/workspace/songs");
    for i in 0..64 {
        std::fs::write(songs.join(format!("a{i}")), "x").unwrap();
    }
    let lines = Lines::default();
    let mut all = Vec::new();
    for i in 0..64 {
        let c = request(
            &dir,
            Tag::Rename,
            json!({ "path": format!("a{i}"), "name": format!("b{i}") }),
            &lines,
        );
        all.push(tokio::spawn(async move {
            text(routes::dispatch(H::Rename, &c).await).await.0
        }));
    }
    for t in all {
        assert_eq!(t.await.unwrap(), 200);
    }
    assert!((0..64).all(|i| songs.join(format!("b{i}")).is_file()));
    // Served in order: no write's `before` ever sits between another's `before` and `after`.
    let seen = calls::of(&songs.display().to_string());
    assert_eq!(seen.len(), 128);
    assert!(seen.chunks(2).all(|p| p == ["before", "after"]), "{seen:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_write_waits_for_the_lock_and_the_table_forgets_what_nobody_holds() {
    let dir = scratch("e-lock");
    let lines = Lines::default();
    // The lock is the account's: its studio home (1210).
    let held = locks::write(&crate::routes::files::service::native::tests::slashed(
        &dir.join("home"),
    ))
    .await;
    let c = request(
        &dir,
        Tag::Folder,
        json!({ "dir": ".", "name": "later" }),
        &lines,
    );
    let task = tokio::spawn(async move { text(routes::dispatch(H::Folder, &c).await).await.0 });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished(), "the lock is held");
    assert!(!dir.join("home/workspace/songs/later").exists());
    drop(held);
    assert_eq!(task.await.unwrap(), 200);
    for i in 0..300 {
        drop(locks::write(&format!("/r/{i}")).await);
    }
    assert!(locks::remembered() < 300 + 10);
}

struct Flaky(AtomicU32);

impl Renamer for Flaky {
    fn rename(&self, _: &str, _: &str) -> io::Result<()> {
        if self.0.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    }
}

#[test]
fn a_rename_that_keeps_failing_sleeps_on_no_blocking_thread() {
    // Two blocking threads in all: eight renames waiting out their retries must leave them free.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        // 24 failures then success; 26 failures: the code is answered after about 2.5 s.
        let ok = rename_retrying(
            Arc::new(Flaky(AtomicU32::new(24))),
            "a",
            "b",
            Duration::from_millis(5),
        )
        .await;
        assert_eq!(ok, Ok(()));
        let t0 = Instant::now();
        let tasks: Vec<_> = (0..16)
            .map(|_| {
                tokio::spawn(async {
                    rename_retrying(
                        Arc::new(Flaky(AtomicU32::new(26))),
                        "a",
                        "b",
                        Duration::from_millis(100),
                    )
                    .await
                })
            })
            .collect();
        let mut slowest = Duration::ZERO;
        while tasks.iter().any(|t| !t.is_finished()) {
            let t = Instant::now();
            tokio::task::spawn_blocking(|| ()).await.unwrap();
            slowest = slowest.max(t.elapsed());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        for t in tasks {
            assert!(matches!(
                t.await.unwrap(),
                Err(EntryError::Io { code: "EACCES", .. })
            ));
        }
        assert!(
            t0.elapsed() >= Duration::from_millis(2500),
            "{:?}",
            t0.elapsed()
        );
        assert!(
            slowest < Duration::from_millis(300),
            "a blocking thread waited {slowest:?} for a free one"
        );
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_native_move_cancels_the_prewarm_itself_with_no_workspace_in_the_runtime() {
    use super::media::tests_retired::Gated;
    let dir = scratch("e-prewarm");
    let songs = dir.join("home/workspace/songs");
    std::fs::write(songs.join("a.txt"), "a").unwrap();
    for i in 0..2 {
        super::media::testkit::wav(&songs.join(format!("c{i}.wav")), 1.0, 3.0 + f64::from(i));
    }
    let env = [(
        "STUDIO_HOME".to_string(),
        crate::routes::files::service::native::tests::slashed(&dir.join("home")),
    )]
    .into();
    let media = Arc::new(super::media::Media::new(
        super::media::Config::of(&env, &dir),
        Arc::new(|| ()),
    ));
    let root = songs.display().to_string();
    let run = media.prewarms.begin(&root);
    let lines = Lines::default();
    let mut c = request(
        &dir,
        Tag::Rename,
        json!({ "path": "a.txt", "name": "b.txt" }),
        &lines,
    );
    c.media = Some(media.clone());
    let done = text(routes::dispatch(handler(Tag::Rename), &c).await).await;
    assert_eq!(done.0, 200);
    // The run the move retired starts nothing, whatever the runtime held.
    let gate = Arc::new(tokio::sync::Notify::new());
    gate.notify_one();
    let locator = Arc::new(Gated {
        dir: songs.clone(),
        gate,
    });
    let status = media
        .prewarms
        .run(
            &root,
            run,
            media.waves.clone(),
            locator,
            media.counts.clone(),
        )
        .await;
    assert_eq!(status, super::media::prewarm::Status::default());
}
