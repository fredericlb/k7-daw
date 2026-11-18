//! Tests of the rooms-to-folders pass: each rule fails when it alone is reverted.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use k7_sync::relay::{Options, Relay};
use k7_sync::settle::Native;
use serde_json::{Map, Value, json};

use super::*;

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("k7-rooms-out-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn batch(id: &str, files: &[(&str, &str)]) -> Map<String, Value> {
    let ops: Vec<Value> = files
        .iter()
        .map(|(f, t)| json!({"kind": "text", "file": f, "text": t}))
        .collect();
    json!({"id": id, "replica": "r", "ts": 1, "ops": ops})
        .as_object()
        .unwrap()
        .clone()
}

/// A room of `ws` in `account` holding `files`, saved and dropped as a server's relay left it.
fn room(account: &Path, ws: &str, files: &[(&str, &str)]) {
    let dir = account.to_path_buf();
    let relay = Relay::new(
        Options::new(Arc::new(move |_| dir.clone())),
        Arc::new(Native),
    );
    relay
        .room("a", ws)
        .unwrap()
        .submit(batch("b1", files))
        .unwrap();
    relay.stop();
}

fn log_of(account: &Path, ws: &str) -> PathBuf {
    k7_sync::path::room_dir(account, ws)
}

fn read(p: PathBuf) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn a_room_with_no_folder_gets_one() {
    let a = scratch("nofolder");
    room(&a, "Band", &[("song.txt", "hello"), ("deep/x.txt", "x")]);
    let r = write_account(&a);
    assert_eq!(read(a.join("workspace/Band/song.txt")), "hello");
    assert_eq!(read(a.join("workspace/Band/deep/x.txt")), "x");
    assert_eq!((r.rooms, r.files), (1, 2));
}

#[test]
fn a_differing_folder_file_keeps_its_content_whatever_its_time() {
    let a = scratch("differs");
    room(&a, "Band", &[("song.txt", "room"), ("other.txt", "o")]);
    std::fs::create_dir_all(a.join("workspace/Band")).unwrap();
    let f = a.join("workspace/Band/song.txt");
    std::fs::write(&f, "folder").unwrap();
    let old = SystemTime::now() - Duration::from_secs(36000);
    std::fs::File::options()
        .write(true)
        .open(&f)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let r = write_account(&a);
    assert_eq!(read(f), "folder");
    assert_eq!(
        read(a.join("workspace/Band/song (server room).txt")),
        "room"
    );
    assert_eq!(
        (r.kept_both, read(a.join("workspace/Band/other.txt"))),
        (1, "o".into())
    );
}

#[test]
fn a_legacy_room_whose_loading_writes_its_log_still_keeps_the_folder_file() {
    let a = scratch("legacy");
    room(&a, "Band", &[("song.txt", "room")]);
    let log = log_of(&a, "Band");
    let _ = std::fs::remove_dir_all(log.join("log"));
    let _ = std::fs::remove_dir_all(log.join("snapshots"));
    std::fs::create_dir_all(a.join("workspace/Band")).unwrap();
    std::fs::write(a.join("workspace/Band/song.txt"), "folder").unwrap();
    write_account(&a);
    assert_eq!(read(a.join("workspace/Band/song.txt")), "folder");
}

#[test]
fn a_file_at_the_beside_name_is_never_replaced() {
    let a = scratch("beside");
    room(&a, "Band", &[("song.txt", "room")]);
    let w = a.join("workspace/Band");
    std::fs::create_dir_all(&w).unwrap();
    std::fs::write(w.join("song.txt"), "folder").unwrap();
    std::fs::write(w.join("song (server room).txt"), "mine").unwrap();
    write_account(&a);
    assert_eq!(read(w.join("song (server room).txt")), "mine");
    assert_eq!(read(w.join("song (server room 2).txt")), "room");
}

#[test]
fn a_user_file_named_like_a_temp_is_never_touched() {
    let a = scratch("temp");
    room(&a, "Band", &[("song.txt", "room")]);
    let w = a.join("workspace/Band");
    std::fs::create_dir_all(&w).unwrap();
    std::fs::write(w.join("song.room-tmp"), "mine").unwrap();
    write_account(&a);
    assert_eq!(read(w.join("song.room-tmp")), "mine");
    assert_eq!(read(w.join("song.txt")), "room");
}

#[test]
fn a_second_run_writes_nothing_and_the_log_and_blobs_stay() {
    let a = scratch("twice");
    room(&a, "Band", &[("song.txt", "hello")]);
    std::fs::create_dir_all(a.join(".sync-blobs/ab")).unwrap();
    std::fs::write(a.join(".sync-blobs/ab/keep"), "blob").unwrap();
    assert_eq!(write_account(&a).files, 1);
    std::fs::remove_file(a.join("workspace/Band/song.txt")).unwrap();
    assert_eq!(write_account(&a), Report::default());
    assert!(!a.join("workspace/Band/song.txt").exists());
    let log = log_of(&a, "Band");
    assert!(log.join("entries.jsonl").exists() || log.join("room.json").exists());
    assert_eq!(read(a.join(".sync-blobs/ab/keep")), "blob");
}

#[test]
fn a_blob_ref_is_written_as_the_blob_bytes() {
    let a = scratch("blob");
    let hash = "a".repeat(64);
    let r = format!("\u{0}k7fs-blob {hash} 4\n");
    room(&a, "Band", &[("kick.wav", &r)]);
    std::fs::create_dir_all(a.join(".sync-blobs/aa")).unwrap();
    std::fs::write(a.join(format!(".sync-blobs/aa/{hash}")), [1u8, 2, 3, 4]).unwrap();
    write_account(&a);
    assert_eq!(
        std::fs::read(a.join("workspace/Band/kick.wav")).unwrap(),
        [1, 2, 3, 4]
    );
}

#[test]
fn a_room_whose_blob_is_missing_writes_no_marker_and_is_tried_again() {
    let a = scratch("noblob");
    let r = format!("\u{0}k7fs-blob {} 4\n", "b".repeat(64));
    room(&a, "Band", &[("kick.wav", &r), ("t.txt", "t")]);
    let first = write_account(&a);
    assert_eq!(first.skipped, 1);
    assert!(!log_of(&a, "Band").join(DONE).exists());
    std::fs::create_dir_all(a.join(".sync-blobs/bb")).unwrap();
    std::fs::write(
        a.join(format!(".sync-blobs/bb/{}", "b".repeat(64))),
        [9u8; 4],
    )
    .unwrap();
    write_account(&a);
    assert_eq!(
        std::fs::read(a.join("workspace/Band/kick.wav")).unwrap(),
        [9; 4]
    );
    assert!(log_of(&a, "Band").join(DONE).exists());
}

#[cfg(unix)]
#[test]
fn a_link_in_the_folder_is_not_followed() {
    let a = scratch("link");
    room(&a, "Band", &[("out/x.txt", "x")]);
    let outside = scratch("link-outside");
    std::fs::create_dir_all(a.join("workspace/Band")).unwrap();
    std::os::unix::fs::symlink(&outside, a.join("workspace/Band/out")).unwrap();
    write_account(&a);
    assert!(!outside.join("x.txt").exists());
}
