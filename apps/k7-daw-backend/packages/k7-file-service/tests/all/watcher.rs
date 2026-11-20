//! What a host watches (`watcher.rs`): a root whole, or on a scarce host a library's folder without its published
//! versions, and a polled watcher that hears a live folder and never a frozen one.

use crate::support;

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use k7_file_service::watcher::{make_as, plan, under_frozen};
use notify::RecursiveMode;
use support::Scratch;

/// A library's folder as the hub keeps it: published versions with their manifest, and the draft.
fn library(name: &str) -> Scratch {
    let s = Scratch::new(name);
    for v in ["1", "2"] {
        s.file(&format!("lib/{v}/library.k7x"), "x");
        s.file(&format!("lib/{v}/modules/a/main.k7x"), "x");
    }
    s.file("lib/draft/modules/b/main.k7x", "x");
    s
}

#[test]
fn a_host_that_pays_nothing_per_file_watches_a_root_whole() {
    let s = library("watcher-whole");
    let root = s.0.join("lib");
    assert_eq!(plan(&root, false), vec![(root, RecursiveMode::Recursive)]);
}

#[test]
fn a_scarce_host_leaves_a_librarys_published_versions_out() {
    let s = library("watcher-scarce");
    let root = s.0.join("lib");
    let planned = plan(&root, true);
    assert!(planned.contains(&(root.clone(), RecursiveMode::NonRecursive)));
    assert!(planned.contains(&(root.join("draft"), RecursiveMode::Recursive)));
    assert_eq!(
        planned.len(),
        2,
        "no published version is watched: {planned:?}"
    );
}

#[test]
fn a_folder_named_draft_or_by_a_number_without_a_manifest_is_no_library() {
    let s = Scratch::new("watcher-workspace");
    s.file("ws/draft/main.k7x", "x");
    s.file("ws/1/notes.txt", "x");
    let root = s.0.join("ws");
    assert_eq!(plan(&root, true), vec![(root, RecursiveMode::Recursive)]);
}

#[test]
fn a_path_in_a_published_version_is_frozen_and_the_draft_is_not() {
    let s = library("watcher-frozen");
    assert!(under_frozen(&s.0.join("lib/1")));
    assert!(under_frozen(&s.0.join("lib/2/modules/a/main.k7x")));
    assert!(!under_frozen(&s.0.join("lib/draft/modules/b")));
    assert!(!under_frozen(&s.0.join("lib")));
}

#[test]
fn a_polled_watcher_hears_the_draft_and_never_a_version() {
    let s = library("watcher-poll");
    let root = s.0.join("lib");
    let (tx, rx) = mpsc::channel();
    let mut w = make_as(true, move |ev: notify::Result<notify::Event>| {
        if let Ok(ev) = ev {
            for p in ev.paths {
                let _ = tx.send(p);
            }
        }
    })
    .unwrap();
    for (path, mode) in plan(&root, true) {
        w.watch(&path, mode).unwrap();
    }
    s.file("lib/2/modules/a/late.k7x", "x");
    s.file("lib/draft/modules/b/late.k7x", "x");
    let mut heard: Vec<std::path::PathBuf> = Vec::new();
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end && !heard.iter().any(|p| p.ends_with("b/late.k7x")) {
        if let Ok(p) = rx.recv_timeout(Duration::from_millis(500)) {
            heard.push(p);
        }
    }
    assert!(
        heard.iter().any(|p| p.ends_with("b/late.k7x")),
        "the draft was never heard: {heard:?}"
    );
    assert!(
        heard.iter().all(|p| !is_in(p, &root.join("2"))),
        "a published version was heard: {heard:?}"
    );
}

fn is_in(p: &Path, dir: &Path) -> bool {
    p.starts_with(dir)
}
