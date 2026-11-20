//! capture_hostile.rs — the capture against a disk that fails: a write that cannot land, a `pending.json` that cannot be
//! saved, links, a deep nest, a large tree, a baseline that cannot be read. Each answers, counted, without a hang.
//! What a reader gets wrong: the bound is a count of files and ops; the wall time is only a backstop.

use std::io::{self, ErrorKind};
use std::time::Instant;

use k7_capture::cache::sha256_bytes;
use k7_capture::capture::{Baseline, files_under};
use k7_replica::blob::blob_ref;
use std::sync::Arc;

use crate::rig::{Rig, bytes, files, ops, put};
use crate::support::link;

fn reference(data: &[u8]) -> String {
    blob_ref(&sha256_bytes(data).hash, data.len() as u64).unwrap()
}

#[test]
fn a_write_that_fails_leaves_the_ledger_and_the_folder_as_they_were() {
    let mut r = Rig::started("w-fail", &[("keep.md", "k")], false);
    // A folder where the file belongs: the rename onto it fails on every host.
    std::fs::create_dir_all(format!("{}/a.md/inner", r.root)).unwrap();
    let before = r.cap.text_files();
    assert!(r.cap.write_known("a.md", Some("text")).is_err());
    assert_eq!(r.cap.text_files(), before);
    assert!(r.cap.take_paths().is_empty(), "a failed write is no news");
    let temps = std::fs::read_dir(&r.root)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        })
        .count();
    assert_eq!(temps, 0);
    // A path that leaves the workspace is refused before anything is written.
    let e = r.cap.write_known("../x.md", Some("t")).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
}

#[test]
fn a_ref_that_cannot_be_placed_leaves_the_ledger_as_it_was() {
    let mut r = Rig::started("p-fail", &[], true);
    let data = bytes(256, 2);
    r.cap
        .write_known("one.wav", Some(&reference(&data)))
        .unwrap();
    assert_eq!(r.cap.take_missing().len(), 1);
    let id = sha256_bytes(&data);
    std::fs::write(format!("{}/blobs/{}", r.state, id.hash), &data).unwrap();
    std::fs::create_dir_all(format!("{}/two.wav/inner", r.root)).unwrap();
    assert!(
        r.cap
            .write_known("two.wav", Some(&reference(&data)))
            .is_err()
    );
    assert!(!r.cap.text_files().contains(&"two.wav".to_string()));
}

#[test]
fn a_pending_json_that_cannot_be_saved_leaves_no_file_pending_in_memory() {
    let mut r = Rig::started("pend-fail", &[], true);
    let data = bytes(300, 4);
    let id = sha256_bytes(&data);
    // A folder where `pending.json` belongs.
    std::fs::create_dir_all(format!("{}/blobs/pending.json", r.state)).unwrap();
    assert!(
        r.cap
            .write_known("far.wav", Some(&reference(&data)))
            .is_err()
    );
    assert!(!r.cap.pending_on(&id.hash));
    assert!(r.cap.text_files().is_empty());
    assert!(
        r.cap.take_missing().is_empty(),
        "nobody is asked for bytes it could not remember waiting for"
    );
    // Once the disk answers, the same write goes through and a restart still waits for it.
    std::fs::remove_dir(format!("{}/blobs/pending.json", r.state)).unwrap();
    r.cap
        .write_known("far.wav", Some(&reference(&data)))
        .unwrap();
    assert!(r.cap.pending_on(&id.hash));
    r.reopen();
    r.cap.start(None).unwrap();
    assert_eq!(r.cap.take_missing().len(), 1);
    assert!(r.cap.pending_on(&id.hash));
}

#[test]
fn a_link_loop_and_links_to_files_are_never_entered() {
    let r = Rig::new("loops", &[("real/in.md", "in"), ("top.md", "t")], false);
    let ok = link(&r.root, &format!("{}/real/loop", r.root), true)
        && link(
            &format!("{}/real", r.root),
            &format!("{}/again", r.root),
            true,
        )
        && link(
            &format!("{}/top.md", r.root),
            &format!("{}/real/top-link.md", r.root),
            false,
        );
    if !ok {
        return;
    }
    let mut got = files_under(&r.root, "", k7_file_service::filter::skip).unwrap();
    got.sort();
    assert_eq!(got, ["real/in.md", "top.md"]);
}

#[test]
fn a_nest_a_few_hundred_deep_is_walked_on_a_small_stack() {
    let r = Rig::new("nest", &[], false);
    let mut rel = String::new();
    for _ in 0..350 {
        rel.push_str("d/");
    }
    std::fs::create_dir_all(format!("{}/{rel}", r.root)).unwrap();
    put(&r.root, &format!("{rel}leaf.md"), "x");
    let root = r.root.clone();
    // A recursive walk would need a frame per level; an explicit stack needs none of this thread's.
    let found = std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || files_under(&root, "", k7_file_service::filter::skip).unwrap())
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].ends_with("d/leaf.md"));
}

#[test]
fn a_large_tree_is_one_batch_of_one_op_per_file() {
    let n = 500;
    let mut r = Rig::started("large", &[], false);
    for i in 0..n {
        let dir = format!("{}/g{}", r.root, i % 50);
        if i < 50 {
            std::fs::create_dir_all(&dir).unwrap();
        }
        std::fs::write(format!("{dir}/f{i}.md"), "x").unwrap();
    }
    let b = r.sweep();
    assert_eq!(files(&b).len(), n);
    assert!(ops(b.as_ref().unwrap()).iter().all(|o| o["kind"] == "text"));
    assert!(r.sweep().is_none());
    assert_eq!(r.cap.text_files().len(), n);
}

struct Broken;

impl Baseline for Broken {
    fn files(&self) -> io::Result<Vec<String>> {
        Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "confirmed/ is unreadable",
        ))
    }

    fn get(&self, _: &str) -> io::Result<Option<String>> {
        Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "confirmed/ is unreadable",
        ))
    }
}

#[test]
fn a_confirmed_folder_that_cannot_be_read_stops_the_start_and_emits_nothing() {
    let mut r = Rig::new("conf", &[("a.md", "a")], false);
    assert!(r.cap.start(Some(Arc::new(Broken))).is_err());
    assert!(
        r.cap.take_batches().is_empty(),
        "no removal is sent for what could not be read"
    );
    assert!(r.cap.text_files().is_empty());
}

#[test]
fn a_ledger_entry_for_a_file_gone_is_a_removal_and_one_still_pending_is_not() {
    let mut r = Rig::started("gone", &[("a.md", "a")], true);
    let data = bytes(64, 8);
    r.cap
        .write_known("far.wav", Some(&reference(&data)))
        .unwrap();
    std::fs::remove_file(format!("{}/a.md", r.root)).unwrap();
    assert_eq!(files(&r.sweep()), ["a.md"]);
}

/// Work of `n` touched paths and of the removal of the whole tree of `n` files, counted where the loops visit entries.
fn sweeps_of(n: usize) -> (u64, u64) {
    let mut r = Rig::started("scale", &[], false);
    for i in 0..n {
        let dir = format!("{}/g{}", r.root, i % 50);
        if i < 50 {
            std::fs::create_dir_all(&dir).unwrap();
        }
        std::fs::write(format!("{dir}/f{i}.md"), "x").unwrap();
    }
    assert_eq!(files(&r.sweep()).len(), n);
    let t = Instant::now();
    let before = r.cap.work();
    for i in 0..n {
        r.cap.touch(&format!("g{}/f{i}.md", i % 50));
    }
    assert!(r.cap.flush_now(false).unwrap().is_none());
    let touched = r.cap.work() - before;
    for g in 0..50 {
        std::fs::remove_dir_all(format!("{}/g{g}", r.root)).unwrap();
    }
    let before = r.cap.work();
    let removed = r.sweep();
    let gone = r.cap.work() - before;
    assert_eq!(files(&removed).len(), n);
    assert!(r.cap.text_files().is_empty());
    println!(
        "{n} files: touched {touched} work, removal {gone} work, {:?}",
        t.elapsed()
    );
    (touched, gone)
}

#[test]
fn a_dirty_flush_and_the_removal_of_a_whole_tree_cost_in_proportion_to_the_files() {
    let (small, big) = (sweeps_of(250), sweeps_of(1_000));
    // Four times the files is four times the work; a cost inside a std call is judged by the seconds printed above.
    for (a, b, what) in [(small.0, big.0, "dirty flush"), (small.1, big.1, "removal")] {
        assert!(b <= 5 * a, "{what}: {a} work for 250, {b} for 1000");
    }
}
