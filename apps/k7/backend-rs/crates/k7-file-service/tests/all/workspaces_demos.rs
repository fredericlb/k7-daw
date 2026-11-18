//! The demos in Examples (1640 to 1643): where the bundle is, the stamps of a tree, the plan of a mirror and the
//! mirror itself; the stamps file as the TypeScript writes and reads it, and a folder that fails costs that demo alone.

use crate::support;

use k7_file_service::workspaces::demos::*;
use serde_json::Value;
use support::ws::touch;
use support::{Scratch, can_link, link};

const EMPTY_SHA1: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";

struct Fx {
    s: Scratch,
    src: String,
    ex: String,
}

fn fx(name: &str) -> Fx {
    let s = Scratch::new(name);
    let src = s.dir("bundle");
    s.file("bundle/a-demo.k7/project.k7x", "project({ name: \"a\" })\n");
    s.file("bundle/a-demo.k7/nodes/n.k7x", "extends(\"x\")\n");
    s.file("bundle/b-demo.k7/project.k7x", "project({ name: \"b\" })\n");
    s.file("bundle/.hidden/x", "x");
    s.file("bundle/readme.txt", "not a demo");
    let ex = s.path("studio/workspace/Examples");
    Fx { s, src, ex }
}

fn stamps(f: &Fx) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/.studio-demos.json", f.ex)).unwrap())
        .unwrap()
}

#[test]
fn the_bundle_is_the_accounts_folder_or_the_first_demos_walking_up() {
    let s = Scratch::new("ws-demos-dir");
    let env = s.dir("elsewhere");
    s.dir("root/demos");
    let root = s.path("root");
    assert_eq!(demos_dir(Some(&env), &root).as_deref(), Some(env.as_str()));
    // An env that is not there falls through to the walk; so does an empty one.
    for bad in [Some(s.path("missing")), Some(String::new()), None] {
        assert_eq!(demos_dir(bad.as_deref(), &root), Some(s.path("root/demos")));
    }
    s.dir("up/demos");
    s.dir("up/a/b/c/d");
    // The walk starts at the code's root and climbs.
    assert_eq!(
        demos_dir(None, &s.path("up/a/b/c/d")),
        Some(s.path("up/demos"))
    );
    let deep = s.path("nest/a/b/c/d/e/f/g/h/i/j");
    std::fs::create_dir_all(&deep).unwrap();
    assert_eq!(demos_dir(None, &deep), None);
}

#[test]
fn a_tree_stamp_is_a_sha1_of_what_the_mirror_copies() {
    let f = fx("ws-demos-stamp");
    let a = tree_stamp(&format!("{}/a-demo.k7", f.src));
    assert_eq!(a.len(), 40);
    assert_eq!(tree_stamp(&f.s.path("nowhere")), "");
    assert_eq!(tree_stamp(&f.s.dir("empty")), EMPTY_SHA1);
    // The filter's names are not in it, and a file's size and mtime are.
    f.s.file("bundle/a-demo.k7/node_modules/x.js", "x");
    f.s.file("bundle/a-demo.k7/.git/config", "x");
    f.s.file("bundle/a-demo.k7/.studio-live", "x");
    assert_eq!(tree_stamp(&format!("{}/a-demo.k7", f.src)), a);
    f.s.file("bundle/a-demo.k7/.kept", "x");
    assert_ne!(tree_stamp(&format!("{}/a-demo.k7", f.src)), a);
    let p = f.s.path("bundle/a-demo.k7/project.k7x");
    let before = tree_stamp(&format!("{}/b-demo.k7", f.src));
    touch(&f.s.path("bundle/b-demo.k7/project.k7x"), 1_700_000_000, 1);
    assert_ne!(tree_stamp(&format!("{}/b-demo.k7", f.src)), before);
    let _ = p;
}

#[test]
fn a_link_to_a_folder_counts_as_a_file_and_is_not_entered() {
    let f = fx("ws-demos-link");
    if !can_link(&f.s) {
        return;
    }
    let dir = format!("{}/a-demo.k7", f.src);
    let before = tree_stamp(&dir);
    link(&f.s.dir("outside"), &format!("{dir}/linked")).unwrap();
    f.s.file("outside/deep/x", "x");
    let with = tree_stamp(&dir);
    assert_ne!(with, before);
    f.s.file("outside/deep/y", "y");
    assert_eq!(tree_stamp(&dir), with);
}

#[test]
fn a_first_mirror_copies_every_demo_and_writes_the_stamps() {
    let f = fx("ws-demos-first");
    let plan = plan_mirror(&f.src, &f.ex);
    assert_eq!(
        (plan.copy.as_slice(), plan.kept.as_slice()),
        (
            &["a-demo.k7".to_string(), "b-demo.k7".to_string()][..],
            &[][..]
        )
    );
    assert!(std::fs::metadata(&f.ex).is_err(), "a plan creates nothing");
    assert_eq!(mirror(&f.src, &f.ex), ["a-demo.k7", "b-demo.k7"]);
    assert_eq!(
        std::fs::read_to_string(format!("{}/a-demo.k7/nodes/n.k7x", f.ex)).unwrap(),
        "extends(\"x\")\n"
    );
    let s = stamps(&f);
    let names: Vec<&String> = s.as_object().unwrap().keys().collect();
    assert_eq!(names, ["a-demo.k7", "b-demo.k7"]);
    assert_eq!(
        s["a-demo.k7"]["src"],
        tree_stamp(&format!("{}/a-demo.k7", f.src))
    );
    assert_eq!(
        s["a-demo.k7"]["dest"],
        tree_stamp(&format!("{}/a-demo.k7", f.ex))
    );
    assert!(
        !std::fs::read_to_string(format!("{}/.studio-demos.json", f.ex))
            .unwrap()
            .contains(' ')
    );
}

#[test]
fn demos_as_the_last_mirror_left_them_are_not_copied_again() {
    let f = fx("ws-demos-current");
    mirror(&f.src, &f.ex);
    let plan = plan_mirror(&f.src, &f.ex);
    assert_eq!((plan.copy.len(), plan.kept.len()), (0, 2));
    assert_eq!(mirror(&f.src, &f.ex), Vec::<String>::new());
    // The bytes of the stamps file are what a start left, so the other implementation reads them whole.
    let before = std::fs::read(format!("{}/.studio-demos.json", f.ex)).unwrap();
    mirror(&f.src, &f.ex);
    assert_eq!(
        std::fs::read(format!("{}/.studio-demos.json", f.ex)).unwrap(),
        before
    );
}

#[test]
fn an_edited_demo_is_replaced_and_a_users_own_project_is_not_touched() {
    let f = fx("ws-demos-edit");
    mirror(&f.src, &f.ex);
    f.s.file("studio/workspace/Examples/mine.k7/project.k7x", "mine");
    f.s.file("studio/workspace/Examples/a-demo.k7/extra.k7x", "stale");
    f.s.file("studio/workspace/Examples/a-demo.k7/project.k7x", "edited");
    assert_eq!(plan_mirror(&f.src, &f.ex).copy, ["a-demo.k7"]);
    assert_eq!(mirror(&f.src, &f.ex), ["a-demo.k7"]);
    assert_eq!(
        std::fs::read_to_string(format!("{}/a-demo.k7/project.k7x", f.ex)).unwrap(),
        "project({ name: \"a\" })\n"
    );
    assert!(
        std::fs::metadata(format!("{}/a-demo.k7/extra.k7x", f.ex)).is_err(),
        "replaced, not merged"
    );
    assert_eq!(
        std::fs::read_to_string(format!("{}/mine.k7/project.k7x", f.ex)).unwrap(),
        "mine"
    );
}

#[test]
fn a_changed_bundle_and_a_deleted_demo_are_both_copied_again() {
    let f = fx("ws-demos-bundle");
    mirror(&f.src, &f.ex);
    f.s.file("bundle/b-demo.k7/new.k7x", "new");
    std::fs::remove_dir_all(format!("{}/a-demo.k7", f.ex)).unwrap();
    assert_eq!(plan_mirror(&f.src, &f.ex).copy, ["a-demo.k7", "b-demo.k7"]);
    assert_eq!(mirror(&f.src, &f.ex), ["a-demo.k7", "b-demo.k7"]);
    assert!(std::fs::metadata(format!("{}/b-demo.k7/new.k7x", f.ex)).is_ok());
}

#[test]
fn the_filter_keeps_caches_and_vcs_noise_out_of_the_copy() {
    let f = fx("ws-demos-filter");
    f.s.file("bundle/a-demo.k7/node_modules/x.js", "x");
    f.s.file("bundle/a-demo.k7/.git/config", "x");
    f.s.file("bundle/a-demo.k7/.studio-live/state", "x");
    f.s.file("bundle/a-demo.k7/.kept", "x");
    mirror(&f.src, &f.ex);
    for gone in ["node_modules", ".git", ".studio-live"] {
        assert!(
            std::fs::metadata(format!("{}/a-demo.k7/{gone}", f.ex)).is_err(),
            "{gone}"
        );
    }
    assert!(std::fs::metadata(format!("{}/a-demo.k7/.kept", f.ex)).is_ok());
}

#[test]
fn no_bundle_or_an_examples_that_cannot_be_made_is_no_mirror() {
    let f = fx("ws-demos-none");
    assert!(mirror(&f.s.path("nowhere"), &f.ex).is_empty());
    assert!(mirror(&f.s.dir("emptybundle"), &f.ex).is_empty());
    assert!(std::fs::metadata(&f.ex).is_err());
    let blocked = f.s.file("blocked", "a file where Examples should be");
    assert!(mirror(&f.src, &blocked).is_empty());
    assert_eq!(plan_mirror(&f.s.path("nowhere"), &f.ex).copy.len(), 0);
}

#[test]
fn a_stamps_file_that_is_not_an_object_reads_as_none() {
    for (i, text) in [
        "",
        "not json",
        "[1,2]",
        "3",
        "null",
        "\"s\"",
        "{\"a-demo.k7\":\"x\"}",
        "{\"a-demo.k7\":{\"src\":1,\"dest\":2}}",
    ]
    .iter()
    .enumerate()
    {
        let f = fx(&format!("ws-demos-badstamps{i}"));
        f.s.file("studio/workspace/Examples/.studio-demos.json", text);
        assert_eq!(plan_mirror(&f.src, &f.ex).copy.len(), 2, "{text:?}");
        assert_eq!(mirror(&f.src, &f.ex).len(), 2, "{text:?}");
        assert_eq!(stamps(&f).as_object().unwrap().len(), 2);
    }
}

#[test]
fn a_demo_that_is_a_link_loop_fails_alone() {
    let f = fx("ws-demos-loop");
    if !can_link(&f.s) {
        return;
    }
    link(
        &format!("{}/a-demo.k7", f.src),
        &format!("{}/a-demo.k7/up", f.src),
    )
    .unwrap();
    assert_eq!(mirror(&f.src, &f.ex), ["b-demo.k7"]);
    assert!(stamps(&f).get("a-demo.k7").is_none());
    assert!(stamps(&f).get("b-demo.k7").is_some());
}

#[test]
fn a_demo_replaced_by_a_file_is_copied_as_a_file() {
    let f = fx("ws-demos-file");
    std::fs::remove_dir_all(format!("{}/b-demo.k7", f.src)).unwrap();
    f.s.file("bundle/b-demo.k7", "now a file");
    // The listing sees a file, so it is no demo; the one that was a folder at the plan and a file at the copy is `copy`'s.
    assert_eq!(mirror(&f.src, &f.ex), ["a-demo.k7"]);
}

#[cfg(unix)]
#[test]
fn a_folder_whose_backslash_name_folds_onto_its_parent_fails_the_stamp_without_a_hang() {
    let f = fx("ws-demos-backslash");
    // `x\..` is the demo folder itself to the shim: a walk that follows the spelling never ends.
    let demo = format!("{}/a-demo.k7", f.src);
    std::fs::create_dir(format!("{demo}/x\\..")).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(tree_stamp(&demo)).unwrap());
    let bound = std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });
    assert_eq!(rx.recv_timeout(bound).unwrap(), "");
}
