//! capture_start.rs — what a capture reads when it starts, and a file or folder another process holds while a scan reads
//! it: `liveCaptureStart.test.ts` and `liveCaptureLocked.test.ts`. What a reader gets wrong: reads are counted by the
//! capture itself, so a test proves a file was NOT read, which the ledger cannot show.

use k7_js::number::text_hash;
use serde_json::Value;

use crate::rig::{MemBaseline, Rig, age, files, ops, put};
use crate::support::{lock, set_mode, unlock};

const MIX: &str = "plugin(\"mixer\")\nparams({ gain: 0.5 })\n";
const NOTES: &str = "- mix the bass\n";
const FILE: &str = "song.k7/nodes/mix.k7x";

/// A workspace of `files`, every mtime well settled.
fn workspace(name: &str, list: &[(&str, &str)]) -> Rig {
    let r = Rig::new(name, list, true);
    for (rel, _) in list {
        age(&r.root, rel, 60);
    }
    r
}

fn reads(r: &mut Rig, base: Option<&MemBaseline>) -> Vec<String> {
    r.reopen();
    r.cap.track_reads();
    r.cap.start(base.and_then(MemBaseline::arc)).unwrap();
    let mut got = r.cap.take_reads();
    got.sort();
    got
}

#[test]
fn an_unchanged_text_is_known_by_the_hash_it_kept_and_only_k7x_files_are_read() {
    let mut r = workspace("known", &[(FILE, MIX), ("NOTES.md", NOTES)]);
    r.cap.start(None).unwrap();
    r.cap.stop();
    let saved: Value =
        serde_json::from_slice(&std::fs::read(format!("{}/blob-index.json", r.state)).unwrap())
            .unwrap();
    assert_eq!(saved["texts"]["NOTES.md"]["hash"], text_hash(NOTES));
    // Never under `files`, where a k7 that knows only blobs would read it as one.
    assert!(saved["files"]["NOTES.md"].is_null());
    assert_eq!(saved["texts"][FILE]["hash"], text_hash(MIX));
    assert_eq!(reads(&mut r, None), [FILE]);
}

#[test]
fn a_text_is_read_afresh_once_its_stamp_moved() {
    let mut r = workspace("moved", &[("NOTES.md", NOTES)]);
    r.cap.start(None).unwrap();
    r.cap.stop();
    age(&r.root, "NOTES.md", 30);
    assert_eq!(reads(&mut r, None), ["NOTES.md"]);
}

#[test]
fn against_a_baseline_the_disk_matches_each_file_is_read_once_and_nothing_is_sent() {
    let base = MemBaseline::of(&[(FILE, MIX), ("NOTES.md", NOTES)]);
    let mut r = workspace("match", &[(FILE, MIX), ("NOTES.md", NOTES)]);
    assert_eq!(reads(&mut r, Some(&base)), ["NOTES.md", FILE]);
    // The first sweep trusts the walk's stamps: nothing is read again.
    assert!(r.sweep().is_none());
    assert!(r.cap.take_reads().is_empty());
    assert!(r.cap.take_batches().is_empty());
}

#[test]
fn the_index_spares_every_read_against_a_baseline_and_a_later_edit_is_still_a_path_op() {
    let base = MemBaseline::of(&[(FILE, MIX), ("NOTES.md", NOTES)]);
    let mut r = workspace("path", &[(FILE, MIX), ("NOTES.md", NOTES)]);
    r.reopen();
    r.cap.start(base.arc()).unwrap();
    assert!(reads(&mut r, Some(&base)).is_empty());
    put(&r.root, FILE, MIX.replace("0.5", "0.7"));
    let b = r.sweep();
    // A k7x op, not a whole text: the text before came back from the baseline.
    let kinds: Vec<_> = b
        .iter()
        .flat_map(ops)
        .map(|o| o["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["k7x"]);
}

#[test]
fn the_whole_text_is_sent_when_the_baseline_no_longer_holds_what_the_ledger_knew() {
    let base = MemBaseline::of(&[(FILE, MIX)]);
    let mut r = workspace("whole", &[(FILE, MIX)]);
    r.reopen();
    r.cap.start(base.arc()).unwrap();
    assert!(reads(&mut r, Some(&base)).is_empty());
    base.set(FILE, "plugin(\"other\")\n");
    put(&r.root, FILE, MIX.replace("0.5", "0.7"));
    let b = r.sweep().unwrap();
    let op = &ops(&b)[0];
    assert_eq!(
        (op["kind"].as_str(), op["was"].as_str()),
        (Some("text"), Some(text_hash(MIX).as_str()))
    );
}

#[test]
fn prepare_builds_the_ledger_and_start_then_reads_nothing_again() {
    let list: Vec<(String, String)> = (0..60)
        .map(|i| (format!("song.k7/nodes/n{i}.k7x"), format!("n({i})\n")))
        .collect();
    let refs: Vec<(&str, &str)> = list.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut r = workspace("prepare", &refs);
    r.cap.track_reads();
    r.cap.prepare(None).unwrap();
    assert_eq!(r.cap.take_reads().len(), 60);
    r.cap.start(None).unwrap();
    assert!(r.cap.take_reads().is_empty());
    // Only the file edited is news: every other one is in the ledger as it is on disk.
    put(&r.root, "song.k7/nodes/n3.k7x", "n(33)\n");
    assert_eq!(files(&r.sweep()), ["song.k7/nodes/n3.k7x"]);
}

#[test]
fn a_locked_file_is_skipped_by_the_scan_said_once_and_sent_once_it_is_free() {
    let mut r = Rig::started("locked", &[("a.txt", "one"), ("b.txt", "uno")], false);
    put(&r.root, "a.txt", "one, then two");
    put(&r.root, "b.txt", "uno, dos");
    let a = format!("{}/a.txt", r.root);
    if !lock(&a) {
        return;
    }
    assert_eq!(files(&r.sweep()), ["b.txt"]);
    assert!(r.sweep().is_none());
    let said: Vec<String> = r
        .cap
        .take_notes()
        .into_iter()
        .filter(|n| n.contains("a.txt"))
        .collect();
    assert_eq!(said.len(), 1);
    assert!(said[0].contains("read again at the next scan") && said[0].contains("EACCES"));
    unlock(&a);
    let later = r.sweep().unwrap();
    assert_eq!(ops(&later)[0]["file"], "a.txt");
    assert_eq!(ops(&later)[0]["text"], "one, then two");
}

#[test]
fn a_locked_file_does_not_stop_a_capture_starting_and_is_learned_at_the_next_scan() {
    let mut r = Rig::new("locked-start", &[("a.txt", "one"), ("b.txt", "uno")], false);
    let a = format!("{}/a.txt", r.root);
    if !lock(&a) {
        return;
    }
    r.cap.start(None).unwrap();
    unlock(&a);
    assert_eq!(r.cap.text_files(), ["b.txt"]);
    assert_eq!(files(&r.sweep()), ["a.txt"]);
    let mut all = r.cap.text_files();
    all.sort();
    assert_eq!(all, ["a.txt", "b.txt"]);
}

#[test]
fn a_folder_that_cannot_be_listed_is_not_read_as_emptied() {
    let mut r = Rig::started("locked-dir", &[("song/x.txt", "x"), ("y.txt", "y")], false);
    let dir = format!("{}/song", r.root);
    if !set_mode(&dir, 0o000) || std::fs::read_dir(&dir).is_ok() {
        set_mode(&dir, 0o755);
        return;
    }
    assert!(r.sweep().is_none());
    assert_eq!(r.cap.take_notes().len(), 1);
    set_mode(&dir, 0o755);
    assert!(r.sweep().is_none());
    let mut all = r.cap.text_files();
    all.sort();
    assert_eq!(all, ["song/x.txt", "y.txt"]);
}

#[test]
fn what_the_room_holds_through_a_link_is_forgotten_and_never_removed() {
    use crate::support::link;
    let base = MemBaseline::of(&[
        ("real/x.md", NOTES),
        ("link-dir/x.md", NOTES),
        ("link-file", NOTES),
    ]);
    let mut r = workspace("links", &[("real/x.md", NOTES)]);
    let ok = link(
        &format!("{}/real", r.root),
        &format!("{}/link-dir", r.root),
        true,
    ) && link(
        &format!("{}/real/x.md", r.root),
        &format!("{}/link-file", r.root),
        false,
    );
    if !ok {
        return;
    }
    r.reopen();
    r.cap.start(base.arc()).unwrap();
    assert!(r.sweep().is_none());
    assert!(
        r.cap.take_batches().is_empty(),
        "a removal was sent for a linked file"
    );
}
