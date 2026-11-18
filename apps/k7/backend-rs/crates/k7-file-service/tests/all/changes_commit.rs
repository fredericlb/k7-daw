//! A change set landed (3-D7): whole or nothing, every kind, the binaries the journal keeps, the job outputs swept.

use crate::support;

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

use k7_file_service::atomic::Real;
use k7_file_service::changes::commit::commit;
use k7_file_service::changes::jobs;
use k7_file_service::text_hash::text_hash;
use serde_json::json;
use support::changes::{Flaky, land, planned, seed, text, tree};
use support::entries::Fx;

// 3-D7: a landing that fails at any rename leaves the disk as it was: no file changed, none made, no folder made, no temp left.
#[test]
fn a_failure_at_any_step_writes_nothing() {
    let ops = || {
        json!([
            text("song.k7/a.txt", "new a", json!(null)),
            text("song.k7/deep/er/b.txt", "new b", json!(null)),
            { "kind": "remove", "file": "song.k7/gone.txt", "was": null },
            { "kind": "copy", "from": "song.k7/c.txt", "file": "song.k7/c2.txt" },
            { "kind": "move", "from": "song.k7/m.txt", "file": "song.k7/moved/m.txt" },
        ])
    };
    let reference = {
        let fx = Fx::new("c-atomic-ref");
        seed(&fx);
        let p = planned(&fx, true, ops()).unwrap();
        let landed = commit(&Real, &p, &mut |_| {}).unwrap();
        assert!(landed.structural);
        tree(&fx.root)
    };
    // The count of renames a whole landing makes, so the loop below covers every one and one past.
    let renames = {
        let fx = Fx::new("c-atomic-count");
        seed(&fx);
        let p = planned(&fx, true, ops()).unwrap();
        let fs = Flaky::at(usize::MAX);
        commit(&fs, &p, &mut |_| {}).unwrap();
        fs.renames.load(Ordering::SeqCst)
    };
    assert!(renames >= 6, "{renames}");
    for n in 0..renames {
        let fx = Fx::new(&format!("c-atomic-{n}"));
        seed(&fx);
        let before = tree(&fx.root);
        let p = planned(&fx, true, ops()).unwrap();
        let err = commit(&Flaky::at(n), &p, &mut |_| {}).expect_err("the Nth rename was refused");
        assert!(err.what.contains("injected"), "{err:?}");
        assert_eq!(tree(&fx.root), before, "after a failure at rename {n}");
    }
    assert_ne!(reference, BTreeMap::new());
    let done = {
        let fx = Fx::new("c-atomic-done");
        seed(&fx);
        let p = planned(&fx, true, ops()).unwrap();
        let fs = Flaky::at(renames);
        commit(&fs, &p, &mut |_| {}).unwrap();
        tree(&fx.root)
    };
    assert_eq!(done, reference);
    assert_eq!(done.get("song.k7/a.txt"), Some(&"new a".to_string()));
    assert_eq!(
        done.get("song.k7/deep/er/b.txt"),
        Some(&"new b".to_string())
    );
    assert!(!done.contains_key("song.k7/gone.txt") && !done.contains_key("song.k7/m.txt"));
    assert_eq!(done.get("song.k7/c2.txt"), Some(&"c".to_string()));
    assert_eq!(done.get("song.k7/moved/m.txt"), Some(&"m".to_string()));
    assert!(!done.keys().any(|k| k.contains(".tmp")), "{done:?}");
}

// A failure while staging (before any rename) also leaves nothing: the parents made are taken away.
#[test]
fn a_failure_while_staging_leaves_no_folder_behind() {
    let fx = Fx::new("c-stage");
    // A file where the second text's folder must be: the first text's folders were already made.
    fx.s.file("studio/workspace/songs/song.k7/block", "file");
    let p = planned(
        &fx,
        true,
        json!([
            text("song.k7/x/y/a.txt", "a", json!(null)),
            text("song.k7/block/b.txt", "b", json!(null)),
        ]),
    )
    .unwrap();
    let before = tree(&fx.root);
    let err = commit(&Real, &p, &mut |_| {}).expect_err("a file is in the way");
    assert!(!err.what.is_empty());
    assert_eq!(tree(&fx.root).len(), before.len());
    assert!(!std::path::Path::new(&fx.at("song.k7/x")).exists());
}

// What lands: a text over a text, a removal, a copy, a move over a file, an adopted output whose staged file is consumed.
#[test]
fn every_kind_lands_and_an_adopted_output_is_consumed() {
    let fx = Fx::new("c-kinds");
    seed(&fx);
    fx.s.file("studio/.job-out/j1/a.wav", "RIFF");
    fx.s.file("studio/workspace/songs/song.k7/old.wav", "OLD");
    let landed = land(&fx, json!([
        text("song.k7/a.txt", "new a", json!(text_hash("old a"))),
        { "kind": "remove", "file": "song.k7/gone.txt", "was": text_hash("g") },
        { "kind": "move", "from": "song.k7/m.txt", "file": "song.k7/clips/m.txt" },
        { "kind": "adopt", "job": "j1", "out": "a.wav", "file": "song.k7/old.wav", "replace": true },
    ])).unwrap();
    assert_eq!(landed.touched.len(), 5);
    let t = tree(&fx.root);
    assert_eq!(t["song.k7/a.txt"], "new a");
    assert_eq!(t["song.k7/old.wav"], "RIFF");
    assert_eq!(t["song.k7/clips/m.txt"], "m");
    assert!(!t.contains_key("song.k7/gone.txt") && !t.contains_key("song.k7/m.txt"));
    assert!(!std::path::Path::new(&fx.s.path("studio/.job-out/j1/a.wav")).exists());
}

// An unchanged text and a removal of an absent file land nothing: no rename, the file's mtime untouched.
#[test]
fn a_set_that_changes_nothing_touches_nothing() {
    let fx = Fx::new("c-noop");
    let a = fx.s.file("studio/workspace/songs/song.k7/a.txt", "same");
    let before = std::fs::metadata(&a).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let landed = land(
        &fx,
        json!([
            text("song.k7/a.txt", "same", json!(null)),
            { "kind": "remove", "file": "song.k7/never.txt", "was": null },
        ]),
    )
    .unwrap();
    assert_eq!((landed.touched.len(), landed.structural), (0, false));
    assert_eq!(std::fs::metadata(&a).unwrap().modified().unwrap(), before);
}

// The journal keeps a pre-image of a binary the set replaces or removes, and asks for nothing else.
#[test]
fn a_binary_about_to_go_is_snapshotted_and_a_text_is_not() {
    let fx = Fx::new("c-snap");
    fx.s.file("studio/workspace/songs/song.k7/a.wav", "A");
    fx.s.file("studio/workspace/songs/song.k7/b.wav", "B");
    fx.s.file("studio/workspace/songs/song.k7/c.wav", "C");
    fx.s.file("studio/workspace/songs/song.k7/t.txt", "T");
    fx.s.file("studio/workspace/songs/song.k7/src.wav", "S");
    let p = planned(
        &fx,
        true,
        json!([
            { "kind": "remove", "file": "song.k7/a.wav", "was": null },
            { "kind": "copy", "from": "song.k7/src.wav", "file": "song.k7/b.wav", "replace": true },
            { "kind": "move", "from": "song.k7/c.wav", "file": "song.k7/d.wav" },
            text("song.k7/t.txt", "T2", json!(text_hash("T"))),
            { "kind": "copy", "from": "song.k7/src.wav", "file": "song.k7/new.wav" },
        ]),
    )
    .unwrap();
    let mut asked = Vec::new();
    commit(&Real, &p, &mut |f| {
        asked.push(f.rsplit('/').next().unwrap().to_string())
    })
    .unwrap();
    asked.sort();
    assert_eq!(asked, ["a.wav", "b.wav", "c.wav"]);
}

// A folder opened for its dates: Windows needs the backup-semantics flag, and write access to set them.
fn open_dir(p: &str) -> std::fs::File {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::File::options()
            .write(true)
            .custom_flags(0x0200_0000)
            .open(p)
            .unwrap()
    }
    #[cfg(not(windows))]
    std::fs::File::open(p).unwrap()
}

// 3-B8: a job's staged output is swept after a day, by its newest file; a young one stays.
#[test]
fn a_job_folder_is_swept_a_day_after_its_newest_file() {
    let s = support::Scratch::new("c-sweep");
    let studio = s.path("studio");
    let old = s.file("studio/.job-out/old/a.wav", "x");
    s.file("studio/.job-out/young/a.wav", "x");
    let busy_old = s.file("studio/.job-out/busy/a.wav", "x");
    let busy_new = s.file("studio/.job-out/busy/b.wav", "x");
    let now = SystemTime::now();
    let aged = |p: &str, by: Duration| {
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(now - by)
            .unwrap();
    };
    aged(&old, Duration::from_secs(25 * 3600));
    aged(&busy_old, Duration::from_secs(30 * 3600));
    aged(&busy_new, Duration::from_secs(60));
    // The folders' own dates are the files' here: a folder's mtime moves with its entries.
    for d in ["old", "busy"] {
        let p = s.path(&format!("studio/.job-out/{d}"));
        let f = open_dir(&p);
        f.set_modified(now - Duration::from_secs(30 * 3600))
            .unwrap();
    }
    assert_eq!(jobs::sweep(&studio, now, jobs::KEEP), 1);
    assert!(!std::path::Path::new(&s.path("studio/.job-out/old")).exists());
    assert!(std::path::Path::new(&s.path("studio/.job-out/young")).exists());
    assert!(std::path::Path::new(&s.path("studio/.job-out/busy")).exists());
}
