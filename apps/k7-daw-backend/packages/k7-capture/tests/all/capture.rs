//! capture.rs — what a change becomes, over real folders and a clock moved by hand: `liveCapture.test.ts` and
//! `liveCaptureFeed.test.ts` as tables. What a reader gets wrong: the watcher is replaced by `touch`, the timers by `quiet`.

use k7_capture::capture::{LiveCapture, Options};
use k7_js::number::text_hash;
use k7_replica::blob::blob_ref;
use serde_json::{Value, json};

use crate::rig::{Rig, bytes, files, get, ops, put, settler};
use crate::support::link;

const MIX: &str = "plugin(\"mixer\")\n// the strip\nparams({ gain: 0.5, pan: 0 })\n";
const FILE: &str = "song.k7/nodes/mix.k7x";

fn mix(from: &str, to: &str) -> String {
    MIX.replace(from, to)
}

fn started() -> Rig {
    Rig::started("cap", &[(FILE, MIX)], false)
}

#[test]
fn nothing_is_emitted_for_what_was_there_when_it_started() {
    let mut r = started();
    assert!(r.cap.take_batches().is_empty());
    assert!(r.cap.on_sweep(true).is_none());
}

#[test]
fn a_value_edit_is_one_k7x_path_op() {
    let mut r = started();
    put(&r.root, FILE, mix("0.5", "0.8"));
    let b = r.touched(&[FILE]).unwrap();
    assert_eq!(
        ops(&b),
        vec![
            json!({ "kind": "k7x", "file": FILE, "ops": [{ "op": "set", "path": ["params", "gain"], "k7x": "0.8", "was": "0.5" }] })
        ]
    );
    assert_eq!(
        (
            b["label"].as_str(),
            b["local"].as_u64(),
            b["replica"].as_str()
        ),
        (Some("edit"), Some(1), Some("cap"))
    );
}

/// Each row: what the disk does, the paths the watcher reports, and the ops that come out.
#[test]
fn a_change_becomes_text_and_removal_ops_by_the_table() {
    type Act = fn(&str);
    let rows: Vec<(&str, Act, Vec<&str>, Vec<Value>)> = vec![
        (
            "a new file is text with no was",
            |root| put(root, "song.k7/TODOS.md", "- mix\n"),
            vec!["song.k7/TODOS.md"],
            vec![
                json!({ "kind": "text", "file": "song.k7/TODOS.md", "text": "- mix\n", "was": null }),
            ],
        ),
        (
            "a deleted file is a removal naming the hash it had",
            |root| std::fs::remove_file(format!("{root}/{FILE}")).unwrap(),
            vec![FILE],
            vec![json!({ "kind": "remove", "file": FILE, "was": text_hash(MIX) })],
        ),
        (
            "a folder renamed whole is removals and creations",
            |root| std::fs::rename(format!("{root}/song.k7"), format!("{root}/tune.k7")).unwrap(),
            vec!["song.k7", "tune.k7"],
            vec![
                json!({ "kind": "remove", "file": FILE, "was": text_hash(MIX) }),
                json!({ "kind": "text", "file": "tune.k7/nodes/mix.k7x", "text": MIX, "was": null }),
            ],
        ),
        (
            "what the workspace hides is never captured",
            |root| {
                put(root, ".git/HEAD", "ref: x\n");
                put(root, "song.k7/clips/take.wav.cache", "x");
                put(root, "_recycle_bin/old.k7x", "a(1)\n");
            },
            vec![
                ".git/HEAD",
                "song.k7/clips/take.wav.cache",
                "_recycle_bin/old.k7x",
            ],
            vec![],
        ),
        (
            "a text rewritten with the same content sends nothing",
            |root| put(root, FILE, MIX),
            vec![FILE],
            vec![],
        ),
    ];
    for (name, act, report, want) in rows {
        let mut r = started();
        act(&r.root);
        let got = r.touched(&report).map(|b| ops(&b)).unwrap_or_default();
        let mut got_sorted = got.clone();
        got_sorted.sort_by_key(|o| o["file"].as_str().unwrap().to_string());
        let mut want_sorted = want.clone();
        want_sorted.sort_by_key(|o| o["file"].as_str().unwrap().to_string());
        assert_eq!(got_sorted, want_sorted, "{name}");
        // A full sweep afterwards finds nothing the watcher missed.
        assert!(r.sweep().is_none(), "{name}: sweep");
    }
}

#[test]
fn the_temp_an_atomic_write_goes_through_is_never_captured() {
    let mut r = started();
    let target = format!("{}/{FILE}", r.root);
    // A temp a killed process left behind, then a real atomic save beside it.
    std::fs::write(
        format!("{}/song.k7/nodes/.mix.k7x.4242.deadbeef.tmp", r.root),
        "half a fi",
    )
    .unwrap();
    k7_sync::fsx::write_atomic(std::path::Path::new(&target), &mix("0.5", "0.8")).unwrap();
    let b = r.touched(&["song.k7/nodes/.mix.k7x.4242.deadbeef.tmp", FILE]);
    assert_eq!(files(&b), [FILE]);
    assert!(r.sweep().is_none());
}

#[test]
fn a_file_written_for_another_replica_leaves_no_temp_beside_it() {
    let mut r = started();
    r.cap
        .write_known("song.k7/notes.md", Some("- hi\n"))
        .unwrap();
    assert_eq!(get(&r.root, "song.k7/notes.md"), "- hi\n");
    let mut names: Vec<String> = std::fs::read_dir(format!("{}/song.k7", r.root))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["nodes", "notes.md"]);
    // The watcher hears it and finds nothing to send.
    assert!(r.touched(&["song.k7/notes.md"]).is_none());
    r.cap.write_known("song.k7/notes.md", None).unwrap();
    assert!(!std::path::Path::new(&format!("{}/song.k7/notes.md", r.root)).exists());
    assert!(r.touched(&["song.k7/notes.md"]).is_none());
}

#[test]
fn a_binary_travels_as_its_blob_ref_whatever_its_extension() {
    let mut r = started();
    let data = [0x43, 0x63, 0x6e, 0x4b, 0, 0, 1, 2];
    put(&r.root, "song.k7/preset.fxp", data);
    let b = r.touched(&["song.k7/preset.fxp"]).unwrap();
    let id = k7_capture::cache::sha256_bytes(&data);
    let reference = blob_ref(&id.hash, 8).unwrap();
    assert_eq!(
        ops(&b),
        vec![
            json!({ "kind": "text", "file": "song.k7/preset.fxp", "text": reference, "was": null })
        ]
    );
    assert_eq!(r.cap.read("song.k7/preset.fxp").unwrap(), Some(reference));
}

#[test]
fn a_closed_window_names_the_batch_and_two_join_their_labels() {
    // The window's count is the worker's (worker.rs); the capture only takes the labels of the windows closed.
    let mut r = started();
    put(&r.root, FILE, mix("0.5", "0.6"));
    put(&r.root, "song.k7/nodes/new.k7x", "plugin(\"eq\")\n");
    r.cap.touch(FILE);
    r.cap.touch("song.k7/nodes/new.k7x");
    r.cap.release("track mix");
    assert!(r.cap.quiet_armed());
    let b = r.quiet();
    assert_eq!(b.as_ref().unwrap()["label"], "track mix");
    assert_eq!(files(&b), [FILE, "song.k7/nodes/new.k7x"]);
    put(&r.root, FILE, mix("0.5", "0.7"));
    r.cap.touch(FILE);
    r.cap.release("a");
    r.cap.release("b");
    assert_eq!(r.quiet().unwrap()["label"], "a + b");
}

#[test]
fn a_full_sweep_finds_what_a_watcher_dropped() {
    let mut r = started();
    r.cap.stop();
    put(&r.root, FILE, mix("pan: 0", "pan: 1"));
    let b = r.sweep().unwrap();
    assert_eq!(
        (b["ops"][0]["kind"].as_str(), b["ops"][0]["file"].as_str()),
        (Some("k7x"), Some(FILE))
    );
}

#[test]
fn a_same_size_rewrite_that_kept_its_mtime_is_found_once_that_mtime_settled() {
    let mut r = Rig::new("tick", &[(FILE, MIX)], false);
    // Read at start while still unsettled, and settled once the clock has moved.
    let abs = format!("{}/{FILE}", r.root);
    let at = std::time::SystemTime::now();
    r.cap.start(None).unwrap();
    std::fs::write(&abs, mix("gain: 0.5", "gain: 0.9")).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&abs)
        .unwrap()
        .set_modified(at)
        .unwrap();
    r.clock.advance(3000.0);
    let b = r.sweep().unwrap();
    assert_eq!(
        (b["ops"][0]["kind"].as_str(), b["ops"][0]["file"].as_str()),
        (Some("k7x"), Some(FILE))
    );
}

#[test]
fn the_sweeps_spacing_follows_the_workspace_being_active_or_idle() {
    let mut r = started();
    // Active: every call sweeps.
    put(&r.root, "a.md", "a");
    assert_eq!(files(&r.cap.on_sweep(false)), ["a.md"]);
    // Idle and swept just now: skipped until the spacing passes, 60 s with no watcher.
    r.clock.advance(3.0 * 60_000.0);
    r.cap.on_sweep(true);
    put(&r.root, "b.md", "b");
    r.clock.advance(30_000.0);
    assert!(r.cap.on_sweep(false).is_none());
    r.clock.advance(31_000.0);
    assert_eq!(files(&r.cap.on_sweep(false)), ["b.md"]);
    // With the watcher up the net is five minutes.
    r.cap.set_watching(true);
    put(&r.root, "c.md", "c");
    r.clock.advance(120_000.0);
    assert!(r.cap.on_sweep(false).is_none());
    r.clock.advance(181_000.0);
    assert_eq!(files(&r.cap.on_sweep(false)), ["c.md"]);
}

#[test]
fn folder_and_file_links_are_never_captured() {
    let mut r = started();
    put(&r.root, "real/in.md", "in");
    let (a, b) = (format!("{}/real/in.md", r.root), format!("{}/real", r.root));
    let linked = link(&a, &format!("{}/file-link.md", r.root), false)
        && link(&b, &format!("{}/dir-link", r.root), true)
        && link(&r.root, &format!("{}/real/loop", r.root), true);
    if !linked {
        return;
    }
    let got = r.sweep();
    assert_eq!(files(&got), ["real/in.md"]);
}

#[test]
fn the_feed_reports_each_path_with_a_generation_that_moves() {
    let mut r = started();
    assert_eq!(r.cap.generation(), 0);
    put(&r.root, "song.k7/beat.strudel", "s(\"bd\")\n");
    r.cap.touch("song.k7/beat.strudel");
    assert!(r.cap.generation() > 0);
    assert_eq!(r.cap.take_paths(), ["song.k7/beat.strudel"]);
    // A write made for another replica is a change for this process too, watcher or none.
    let g = r.cap.generation();
    r.cap
        .write_known("song.k7/beat.strudel", Some("s(\"hh\")\n"))
        .unwrap();
    assert_eq!(r.cap.take_paths(), ["song.k7/beat.strudel"]);
    assert!(r.cap.generation() > g);
    // A hidden path is no news.
    let g = r.cap.generation();
    r.cap.touch(".git/x");
    assert_eq!(r.cap.generation(), g);
    assert_eq!(r.cap.root(), r.root);
}

#[test]
fn a_sweep_finding_files_reports_them_to_the_feed() {
    let mut r = started();
    put(&r.root, "x.md", "x");
    r.sweep();
    assert_eq!(r.cap.take_paths(), ["x.md"]);
}

#[test]
fn a_young_binary_waits_and_is_sent_once_settled() {
    let mut r = Rig::started("young", &[], true);
    put(&r.root, "take.wav", bytes(4096, 1));
    assert!(r.touched_now(&["take.wav"]).is_none());
    let due = r.cap.young_due().expect("a timer for the young file");
    assert!(due > 0.0);
    r.clock.advance(100.0);
    r.cap.on_young();
    assert!(r.cap.young_due().is_none());
    let b = r.quiet().unwrap();
    assert_eq!(files(&Some(b)), ["take.wav"]);
}

#[test]
fn options_name_the_defaults_of_the_typescript() {
    let o = Options::new("/w", "r", settler());
    assert_eq!((o.binary_settle_ms, o.active_ms), (1000.0, 120_000.0));
    let _ = LiveCapture::new(o);
}
