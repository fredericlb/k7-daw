//! Tests of the revision piece (1330 to 1335): the route answered, a put moving the token, a decline refused.

use std::time::{Duration, SystemTime};

use k7_file_service::revisions::Revisions;
use serde_json::Value;

use super::tests::{Lines, counted, ctx, scratch};
use super::{Tag, revision};

/// The process's watcher restarts its stream when a root comes or goes, which can drop an event: these tests take turns.
static TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A workspace `songs` holding the project `p.k7` with `n` sources, opened by the marker.
fn project(name: &str, n: usize) -> std::path::PathBuf {
    let dir = scratch(name);
    let p = dir.join("home/workspace/songs/p.k7");
    std::fs::create_dir_all(&p).unwrap();
    for i in 0..n {
        std::fs::write(p.join(format!("s{i}.k7x")), "x").unwrap();
    }
    std::fs::create_dir_all(dir.join("home/workspace/.active-projects")).unwrap();
    std::fs::write(dir.join("home/workspace/.active-projects/songs"), "p.k7").unwrap();
    dir
}

async fn rev_of(c: &super::Ctx) -> String {
    let r = revision::answer(c).await.expect("answered");
    assert_eq!(r.status(), 200);
    let bytes = http_body_util::BodyExt::collect(r.into_body())
        .await
        .unwrap()
        .to_bytes();
    serde_json::from_slice::<Value>(&bytes).unwrap()["rev"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn answers_a_token_that_holds_until_a_source_moves() {
    let _turn = TURN.lock().await;
    let dir = project("rev-answer", 2);
    let lines = Lines::default();
    let c = ctx(&dir, Tag::Revision, "", &lines);
    let first = rev_of(&c).await;
    assert!(!first.starts_with("none:"), "{first}");
    assert_eq!(rev_of(&c).await, first);
    std::fs::write(dir.join("home/workspace/songs/p.k7/new.k7x"), "y").unwrap();
    let mut now = first.clone();
    // Past the 10 s ceiling, so a lost event on a loaded host is still found.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        now = rev_of(&c).await;
        if now != first {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_ne!(now, first, "a new source moves the token");
    // No marker: the none spelling, its own token.
    std::fs::remove_file(dir.join("home/workspace/.active-projects/songs")).unwrap();
    assert!(rev_of(&c).await.starts_with("none:"));
    Revisions::global().forget(&c.root);
    std::fs::remove_dir_all(dir).unwrap();
}

// Silenced and a non-source file: the walk finds nothing new, so only the put's bump moves the token.
#[tokio::test]
async fn a_put_moves_the_token_without_waiting_for_the_watcher() {
    let _turn = TURN.lock().await;
    let dir = project("rev-put", 2);
    k7_file_service::watch::Watch::global().silence(&dir.display().to_string());
    let lines = Lines::default();
    let c = ctx(&dir, Tag::Revision, "", &lines);
    let first = rev_of(&c).await;
    let done = super::journal::write_now(&c, "p.k7/notes.txt", "y", None);
    assert!(matches!(done, super::journal::Done::Written { .. }));
    assert_ne!(rev_of(&c).await, first, "a put bumps the revision");
    Revisions::global().forget(&c.root);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn declines_a_library_project() {
    let _turn = TURN.lock().await;
    // 1334.
    let dir = project("rev-declined", 1);
    let lines = Lines::default();
    let c = ctx(&dir, Tag::Revision, "", &lines);
    let before = counted(&super::stats::global().declined);
    std::fs::write(
        dir.join("home/workspace/.active-projects/songs"),
        "_libraries/x/p.k7",
    )
    .unwrap();
    assert!(revision::answer(&c).await.is_none());
    std::fs::remove_file(dir.join("home/workspace/.active-projects/songs")).unwrap();
    assert!(counted(&super::stats::global().declined) > before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_mtime_is_read_as_the_host_reads_it() {
    // host/fs.rs' `ms`, written out: seconds times 1000 plus nanoseconds over 1e6, in Windows' 100 ns ticks.
    let t = SystemTime::UNIX_EPOCH + Duration::new(1_731_234_567, 890_123_400);
    let by_hand = 1_731_234_567_f64 * 1e3 + 890_123_400_f64 / 1e6;
    assert_eq!(k7_file_service::revision::ms(Ok(t)), by_hand);
    assert_eq!(
        k7_file_service::revision::ms(Err(std::io::Error::other("x"))),
        0.0
    );
}
