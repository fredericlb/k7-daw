//! A native write's edges: run to its end when its client is gone, one at a time across an account's workspaces.

use std::time::Duration;

use serde_json::json;

use super::Tag;
use super::entries::calls;
use super::tests::{Lines, scratch, until};
use super::tests_account::text;
use super::tests_entries::request;
use crate::routes::files::service::routes::{self, H};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_whose_client_left_still_tells_the_bundle() {
    let dir = scratch("e-gone");
    let songs = dir.join("home/workspace/songs");
    std::fs::write(songs.join("a.txt"), "a").unwrap();
    let lines = Lines::default();
    let root = songs.display().to_string();
    let gate = calls::hold(&root, "after");
    let c = request(
        &dir,
        Tag::Rename,
        json!({ "path": "a.txt", "name": "b.txt" }),
        false,
        &lines,
    );
    let task = tokio::spawn(async move { routes::dispatch(H::Rename, &c).await });
    until("the rename landed", || songs.join("b.txt").is_file()).await;
    until("the write reached its held answer", || {
        calls::of(&root) == ["before"]
    })
    .await;
    // The client hangs up: the request's future is dropped, as hyper drops it.
    task.abort();
    let _ = task.await;
    gate.add_permits(1);
    until("the bundle told after the client left", || {
        calls::of(&root) == ["before", "after"]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_workspaces_of_one_account_write_one_at_a_time() {
    let dir = scratch("e-account");
    let songs = dir.join("home/workspace/songs");
    let other = dir.join("home/workspace/other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(songs.join("a.txt"), "a").unwrap();
    std::fs::write(other.join("c.txt"), "c").unwrap();
    let lines = Lines::default();
    let gate = calls::hold(&songs.display().to_string(), "before");
    let first = request(
        &dir,
        Tag::Rename,
        json!({ "path": "a.txt", "name": "b.txt" }),
        false,
        &lines,
    );
    let first =
        tokio::spawn(async move { text(routes::dispatch(H::Rename, &first).await).await.0 });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut second = request(
        &dir,
        Tag::Rename,
        json!({ "path": "c.txt", "name": "d.txt" }),
        false,
        &lines,
    );
    let root = other.display().to_string();
    (second.root, second.workspace) = (root.clone(), "other".into());
    second.native.as_mut().unwrap().root = root;
    let second =
        tokio::spawn(async move { text(routes::dispatch(H::Rename, &second).await).await.0 });
    tokio::time::sleep(Duration::from_millis(300)).await;
    // A library both reach is one folder: the account's writes wait for each other, as one queue would.
    assert!(!second.is_finished(), "the account's lock is held");
    assert!(other.join("c.txt").is_file());
    gate.add_permits(1);
    assert_eq!((first.await.unwrap(), second.await.unwrap()), (200, 200));
    assert!(other.join("d.txt").is_file());
}
