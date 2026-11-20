//! capture_steps.rs — what C3 asks of the capture: a scan that changes nothing until committed, no link followed as a
//! scope or a parent folder, a write that lands whole, a start that keeps `pending.json` as it was.
//! What a reader gets wrong: each test fails without its fix; a host that refuses links or modes returns early inside.

use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use k7_capture::cache::sha256_bytes;
use k7_capture::capture::{Baseline, DIRTY_SWEEP, LiveCapture, Options};
use k7_replica::blob::blob_ref;

use crate::rig::{Rig, age, bytes, files, get, put, settler};
use crate::support::{Scratch, link, set_mode};

fn reference(data: &[u8]) -> String {
    blob_ref(&sha256_bytes(data).hash, data.len() as u64).unwrap()
}

/// A folder outside the workspace holding `o.md`, and links into it from the workspace.
fn outside(r: &Rig) -> Option<String> {
    let out = r.dir.at("outside");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(format!("{out}/o.md"), "outside").unwrap();
    let ok = link(&out, &format!("{}/dir-link", r.root), true)
        && link(
            &format!("{out}/o.md"),
            &format!("{}/file-link.md", r.root),
            false,
        );
    ok.then_some(out)
}

#[test]
fn a_link_that_is_the_scope_is_never_walked() {
    let mut r = Rig::started("scope-link", &[("keep.md", "k")], false);
    let Some(out) = outside(&r) else { return };
    assert!(
        r.touched(&["dir-link", "file-link.md", "dir-link/o.md"])
            .is_none()
    );
    let got = r
        .cap
        .capture_files(&["dir-link".into(), "file-link.md".into()])
        .unwrap();
    assert!(got.is_none());
    assert_eq!(r.cap.text_files(), ["keep.md"]);
    assert!(r.sweep().is_none());
    assert_eq!(get(&out, "o.md"), "outside");
}

#[test]
fn a_scope_that_climbs_or_sits_under_a_file_reads_nothing_and_says_nothing() {
    let mut r = Rig::started("scope-climb", &[("a.md", "a")], false);
    put(&r.dir.at(""), "outside.md", "x");
    let scopes = ["../outside.md".to_string(), "a.md/inner".to_string()];
    assert!(r.cap.capture_files(&scopes).unwrap().is_none());
    // Below a file is gone, not a failure; a climbing scope is said once.
    let notes = r.cap.take_notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("../outside.md"));
}

#[test]
fn a_write_through_a_linked_folder_is_refused_and_lands_nowhere() {
    let mut r = Rig::started("write-link", &[], true);
    let Some(out) = outside(&r) else { return };
    std::fs::write(format!("{out}/y.md"), "y").unwrap();
    let e = r.cap.write_known("dir-link/x.md", Some("t")).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    assert!(!std::path::Path::new(&format!("{out}/x.md")).exists());
    assert!(r.cap.write_known("dir-link/y.md", None).is_err());
    assert_eq!(get(&out, "y.md"), "y");
    let data = bytes(64, 3);
    let id = sha256_bytes(&data);
    std::fs::create_dir_all(format!("{}/blobs", r.state)).unwrap();
    std::fs::write(format!("{}/blobs/{}", r.state, id.hash), &data).unwrap();
    assert!(
        r.cap
            .write_known("dir-link/z.wav", Some(&reference(&data)))
            .is_err()
    );
    assert!(!std::path::Path::new(&format!("{out}/z.wav")).exists());
    assert!(r.cap.text_files().is_empty());
}

#[test]
fn a_placed_file_lands_whole_or_not_at_all() {
    let mut r = Rig::started("place", &[("far.wav", "old")], true);
    let data = bytes(256, 5);
    std::fs::create_dir_all(format!("{}/blobs", r.state)).unwrap();
    let blob = format!("{}/blobs/{}", r.state, sha256_bytes(&data).hash);
    std::fs::write(&blob, &data).unwrap();
    if !set_mode(&blob, 0o000) {
        return;
    }
    let failed = r.cap.write_known("far.wav", Some(&reference(&data)));
    set_mode(&blob, 0o644);
    assert!(failed.is_err());
    assert_eq!(
        get(&r.root, "far.wav"),
        "old",
        "the old file stays until the new one is whole"
    );
    r.cap
        .write_known("far.wav", Some(&reference(&data)))
        .unwrap();
    assert_eq!(std::fs::read(format!("{}/far.wav", r.root)).unwrap(), data);
    let hidden = std::fs::read_dir(&r.root)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        })
        .count();
    assert_eq!(hidden, 0);
}

#[test]
fn a_start_keeps_pending_json_as_it_was_whatever_the_disk_answers() {
    let mut r = Rig::started("pend-start", &[], true);
    for i in 0..3 {
        let f = format!("f{i}.wav");
        r.cap
            .write_known(&f, Some(&reference(&bytes(64, i))))
            .unwrap();
    }
    r.reopen();
    let saved = format!("{}/blobs/pending.json", r.state);
    let before = std::fs::read(&saved).unwrap();
    // A folder that takes no new file: any save during the start would fail it.
    if !set_mode(&format!("{}/blobs", r.state), 0o555) {
        return;
    }
    let started = r.cap.start(None);
    set_mode(&format!("{}/blobs", r.state), 0o755);
    started.unwrap();
    assert_eq!(r.cap.take_missing().len(), 3);
    assert_eq!(std::fs::read(&saved).unwrap(), before);
}

#[test]
fn a_scan_changes_nothing_until_committed_and_an_abort_gives_it_back() {
    let mut r = Rig::started("step", &[], true);
    put(&r.root, "a.md", "a");
    put(&r.root, "take.wav", bytes(256, 1));
    age(&r.root, "take.wav", 10);
    r.cap.touch("a.md");
    r.cap.touch("take.wav");
    let kept = |r: &Rig| {
        std::fs::read_dir(format!("{}/blobs", r.state))
            .map(|d| {
                d.filter(|e| e.as_ref().unwrap().file_name().len() == 64)
                    .count()
            })
            .unwrap_or(0)
    };
    let step = r.cap.step_flush(false).unwrap();
    assert!(!step.is_empty());
    assert_eq!(kept(&r), 1, "the binary's snapshot waits for the verdict");
    r.cap.abort(step);
    assert_eq!(kept(&r), 0, "an aborted scan keeps no snapshot");
    assert!(r.cap.text_files().is_empty());
    // The dirty paths came back: the same flush finds both files.
    let step = r.cap.step_flush(false).unwrap();
    let b = r.cap.commit(step).unwrap();
    assert_eq!(files(&b), ["a.md", "take.wav"]);
    assert_eq!(kept(&r), 1);
    assert_eq!(r.cap.text_files().len(), 2);
}

#[test]
fn past_a_threshold_of_dirty_paths_one_sweep_runs_and_finds_what_nobody_reported() {
    let mut r = Rig::started("threshold", &[("quiet.md", "q")], false);
    for i in 0..=DIRTY_SWEEP {
        r.cap.touch(&format!("none/{i}.md"));
    }
    put(&r.root, "quiet.md", "changed");
    assert_eq!(files(&r.quiet()), ["quiet.md"]);
}

struct Flaky;

impl Baseline for Flaky {
    fn files(&self) -> io::Result<Vec<String>> {
        Ok(vec!["a.md".into(), "b.md".into()])
    }

    fn get(&self, file: &str) -> io::Result<Option<String>> {
        match file {
            "a.md" => Ok(Some("a".into())),
            _ => Err(io::Error::new(ErrorKind::PermissionDenied, "locked")),
        }
    }
}

#[test]
fn a_prepare_that_fails_leaves_no_half_built_ledger() {
    let mut r = Rig::new("half", &[("a.md", "a"), ("b.md", "b")], false);
    assert!(r.cap.prepare(Some(Arc::new(Flaky))).is_err());
    assert!(r.cap.text_files().is_empty());
    assert!(r.cap.take_batches().is_empty());
}

static ID_FAILS: AtomicBool = AtomicBool::new(true);

fn id_fails_once() -> io::Result<String> {
    if ID_FAILS.swap(false, Ordering::SeqCst) {
        return Err(io::Error::other("no randomness"));
    }
    k7_capture::disk::uuid_v4()
}

#[test]
fn a_batch_whose_id_cannot_be_made_leaves_its_edits_for_the_next_scan() {
    let dir = Scratch::new("id-fails");
    let root = dir.at("ws");
    put(&root, "a.md", "a");
    let mut o = Options::new(&root, "id-fails", settler());
    o.new_id = id_fails_once;
    let mut cap = LiveCapture::new(o);
    cap.start(None).unwrap();
    put(&root, "a.md", "edited");
    cap.touch("a.md");
    assert!(cap.flush_now(false).is_err());
    // The ledger still holds the old text, so the sweep a failed step asks for finds the edit.
    cap.step_failed(&io::Error::other("no randomness"));
    let batch = cap.flush_now(true).unwrap();
    assert_eq!(files(&batch), ["a.md"]);
}

#[test]
fn a_stat_check_finds_what_a_scan_has_not_seen_and_reads_nothing_it_can_tell_by_a_stamp() {
    let mut r = Rig::new("uncaptured", &[("a.md", "a"), ("b.md", "b")], false);
    for f in ["a.md", "b.md"] {
        age(&r.root, f, 60);
    }
    r.cap.start(None).unwrap();
    let all: Vec<String> = ["a.md", "b.md", "new.md", ".hidden/x.md"]
        .map(String::from)
        .into();
    put(&r.root, ".hidden/x.md", "skipped by name");
    assert_eq!(r.cap.uncaptured(&all), Vec::<String>::new());
    put(&r.root, "a.md", "a longer text");
    put(&r.root, "new.md", "new");
    std::fs::remove_file(format!("{}/b.md", r.root)).unwrap();
    r.cap.track_reads();
    assert_eq!(r.cap.uncaptured(&all), ["a.md", "b.md", "new.md"]);
    assert!(r.cap.take_reads().is_empty(), "the check read a file");
    r.cap.flush_now(true).unwrap();
    // Just written, so unsettled: no stamp is kept, and a rewrite of the same size is found by its content.
    assert!(r.cap.uncaptured(&all).is_empty());
    put(&r.root, "new.md", "nEw");
    assert_eq!(r.cap.uncaptured(&all), ["new.md"]);
}

#[test]
fn a_peek_is_a_read_the_probe_counts() {
    let mut r = Rig::new("peek-counted", &[("a.md", "one")], false);
    r.cap.start(None).unwrap();
    r.cap.flush_now(true).unwrap();
    r.cap.track_reads();
    assert_eq!(r.cap.peek("a.md").unwrap().as_deref(), Some("one"));
    assert_eq!(r.cap.take_reads(), ["a.md"]);
}
