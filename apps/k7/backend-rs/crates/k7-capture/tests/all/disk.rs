//! disk.rs — what a workspace's capture keeps on disk: the replica's id, the head, the batch log, the blob index, the
//! cache and the confirmed mirror, each as the TypeScript writes it, and each unreadable file an error, never absent (2102).
//! What a reader gets wrong: "unreadable" is staged with a folder where a file belongs, which fails to read on every host.

use std::io::ErrorKind;

use indexmap::IndexMap;
use k7_capture::cache::{BlobCache, sha256_bytes};
use k7_capture::confirmed::ConfirmedDir;
use k7_capture::disk::{LOG_CEILING, LiveLog, load_head, log_dir_of, replica_id, save_head};
use k7_capture::index::BlobIndex;
use k7_capture::ledger::Known;
use k7_replica::Confirmed;
use serde_json::json;

use crate::support::{Scratch, lock, unlock};

#[test]
fn the_replica_id_is_made_once_and_never_made_over_an_unreadable_one() {
    let dir = Scratch::new("id");
    let home = dir.at("home");
    let id = replica_id(&home).unwrap();
    assert_eq!(id.len(), 36);
    assert_eq!(replica_id(&home).unwrap(), id);
    // The TypeScript reads it trimmed, so a file ending in a newline is the same id.
    std::fs::write(format!("{home}/.sync/replica-id"), "abc\n").unwrap();
    assert_eq!(replica_id(&home).unwrap(), "abc");
    // A folder in its place cannot be read: no id is made over it (2102).
    let locked = dir.at("locked");
    std::fs::create_dir_all(format!("{locked}/.sync/replica-id")).unwrap();
    assert!(replica_id(&locked).is_err());
    assert!(
        std::fs::metadata(format!("{locked}/.sync/replica-id"))
            .unwrap()
            .is_dir()
    );
}

#[test]
fn a_locked_id_or_head_is_never_written_over() {
    let dir = Scratch::new("locked");
    let home = dir.at("home");
    let id = replica_id(&home).unwrap();
    let file = format!("{home}/.sync/replica-id");
    let log = dir.at("log");
    save_head(&log, 9).unwrap();
    let head = format!("{log}/head");
    // Where the host cannot lock a file, the case has nothing to stage.
    if !lock(&file) || !lock(&head) {
        return;
    }
    assert!(replica_id(&home).is_err());
    assert!(load_head(&log).is_err());
    unlock(&file);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), id);
}

#[test]
fn a_head_or_id_that_is_there_and_makes_no_sense_is_an_error_naming_it() {
    let dir = Scratch::new("nonsense");
    let (home, log) = (dir.at("home"), dir.at("log"));
    std::fs::create_dir_all(format!("{home}/.sync")).unwrap();
    std::fs::create_dir_all(&log).unwrap();
    for text in ["abc", "7abc", "1,2"] {
        std::fs::write(format!("{log}/head"), text).unwrap();
        let e = load_head(&log).unwrap_err();
        assert!(e.to_string().starts_with("head"), "{text}: {e}");
    }
    // A negative head reads as 0 and a fractional one as its whole part: nothing extra is replayed.
    for (text, want) in [("-3", 0), ("2.9", 2), ("", 0)] {
        std::fs::write(format!("{log}/head"), text).unwrap();
        assert_eq!(load_head(&log).unwrap(), want, "{text:?}");
    }
    for text in ["", " \n"] {
        std::fs::write(format!("{home}/.sync/replica-id"), text).unwrap();
        let e = replica_id(&home).unwrap_err();
        assert!(e.to_string().starts_with("replica-id"), "{e}");
        assert_eq!(
            std::fs::read_to_string(format!("{home}/.sync/replica-id")).unwrap(),
            text
        );
    }
}

#[test]
fn the_head_is_written_whole_and_an_unreadable_one_is_an_error() {
    let dir = Scratch::new("head");
    let log = log_dir_of(&dir.at("home"), &dir.at("ws/songs"));
    assert_eq!(load_head(&log).unwrap(), 0);
    save_head(&log, 42).unwrap();
    assert_eq!(
        std::fs::read_to_string(format!("{log}/head")).unwrap(),
        "42"
    );
    assert_eq!(load_head(&log).unwrap(), 42);
    std::fs::remove_file(format!("{log}/head")).unwrap();
    std::fs::create_dir(format!("{log}/head")).unwrap();
    assert!(load_head(&log).is_err());
}

#[test]
fn the_log_appends_rotates_once_and_tails() {
    let dir = Scratch::new("log");
    let log = LiveLog::new(&dir.at("log"));
    assert!(log.tail(5.0).unwrap().is_empty());
    for i in 0..4 {
        log.append(&json!({ "id": format!("b{i}"), "local": i + 1 }))
            .unwrap();
    }
    let file = std::fs::read_to_string(dir.at("log/batches.jsonl")).unwrap();
    assert_eq!(file.lines().count(), 4);
    assert!(file.starts_with("{\"id\":\"b0\",\"local\":1}\n"));
    assert_eq!(
        log.tail(2.0)
            .unwrap()
            .iter()
            .map(|b| b["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        ["b2", "b3"]
    );
    // Past the ceiling the file moves aside and a new one starts.
    std::fs::OpenOptions::new()
        .write(true)
        .open(dir.at("log/batches.jsonl"))
        .unwrap()
        .set_len(LOG_CEILING + 1)
        .unwrap();
    log.append(&json!({ "id": "after" })).unwrap();
    assert_eq!(
        std::fs::metadata(dir.at("log/batches.1.jsonl"))
            .unwrap()
            .len(),
        LOG_CEILING + 1
    );
    assert_eq!(
        std::fs::read_to_string(dir.at("log/batches.jsonl")).unwrap(),
        "{\"id\":\"after\"}\n"
    );
}

#[test]
fn a_log_that_cannot_be_read_or_parsed_is_an_error() {
    let dir = Scratch::new("badlog");
    std::fs::create_dir_all(dir.at("a/batches.jsonl")).unwrap();
    assert!(LiveLog::new(&dir.at("a")).tail(5.0).is_err());
    std::fs::create_dir_all(dir.at("b")).unwrap();
    std::fs::write(dir.at("b/batches.jsonl"), "{\"id\":1}\nnot json\n").unwrap();
    assert_eq!(
        LiveLog::new(&dir.at("b")).tail(5.0).unwrap_err().kind(),
        ErrorKind::InvalidData
    );
}

fn known(stamp: &str, hash: &str, r#ref: Option<&str>) -> Known {
    Known {
        stamp: stamp.into(),
        hash: hash.into(),
        text: None,
        r#ref: r#ref.map(String::from),
        pending: false,
    }
}

#[test]
fn the_index_keeps_settled_stamps_only_and_writes_nothing_twice() {
    let dir = Scratch::new("index");
    let file = dir.at("log/blob-index.json");
    let blob_ref = format!("\u{0}k7fs-blob {} 12\n", "a".repeat(64));
    let mut ledger: IndexMap<String, Known> = IndexMap::new();
    ledger.insert("song.k7x".into(), known("10:1000", "h1", None));
    ledger.insert("take.wav".into(), known("12:1000", "h2", Some(&blob_ref)));
    ledger.insert("fresh.txt".into(), known("5:9500", "h3", None));
    ledger.insert(
        "wait.wav".into(),
        Known {
            pending: true,
            ..known("9:1000", "h4", Some(&blob_ref))
        },
    );
    ledger.insert("odd".into(), known("9:1000", "h5", Some("not a ref")));
    ledger.insert("nostamp".into(), known("", "h6", None));
    let mut index = BlobIndex::open(&file);
    index.save(&ledger, 10_000.0).unwrap();
    let want = format!(
        "{{\"files\":{{\"take.wav\":{{\"stamp\":\"12:1000\",\"hash\":\"{}\",\"size\":12}}}},\"texts\":{{\"song.k7x\":{{\"stamp\":\"10:1000\",\"hash\":\"h1\"}}}}}}",
        "a".repeat(64)
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), want);
    // Nothing moved: the file is not written again.
    std::fs::remove_file(&file).unwrap();
    index.save(&ledger, 10_000.0).unwrap();
    assert!(!std::path::Path::new(&file).exists());
    assert_eq!(index.get("take.wav", "12:1000").unwrap().size, 12.0);
    assert!(index.get("take.wav", "12:1001").is_none());
    assert_eq!(index.text_hash("song.k7x", "10:1000"), Some("h1"));
    assert!(index.text_hash("fresh.txt", "5:9500").is_none());
}

#[test]
fn an_index_that_does_not_parse_is_empty() {
    let dir = Scratch::new("torn");
    let file = dir.at("blob-index.json");
    std::fs::write(&file, "{\"files\":{\"a\":").unwrap();
    let index = BlobIndex::open(&file);
    assert!(index.get("a", "1:1").is_none());
    std::fs::write(
        &file,
        "{\"files\":{\"a\":{\"stamp\":\"1:1\",\"hash\":\"x\",\"size\":3}},\"texts\":{}}",
    )
    .unwrap();
    assert_eq!(BlobIndex::open(&file).get("a", "1:1").unwrap().hash, "x");
}

#[test]
fn a_snapshot_hashes_its_copy_and_a_failed_one_leaves_no_temp() {
    let dir = Scratch::new("cache");
    let cache = BlobCache::new(&dir.at("blobs"));
    std::fs::write(dir.at("take.wav"), b"RIFFdata").unwrap();
    let first = cache.snapshot(&dir.at("take.wav")).unwrap();
    assert!(first.fresh);
    assert_eq!(first.id, sha256_bytes(b"RIFFdata"));
    assert_eq!(
        std::fs::read(cache.path(&first.id.hash)).unwrap(),
        b"RIFFdata"
    );
    assert!(!cache.snapshot(&dir.at("take.wav")).unwrap().fresh);
    assert!(cache.snapshot(&dir.at("gone.wav")).is_err());
    let names: Vec<String> = std::fs::read_dir(dir.at("blobs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, std::slice::from_ref(&first.id.hash));
    cache.evict(&first.id.hash);
    assert!(!cache.has(&first.id.hash));
}

#[test]
fn the_files_waiting_for_bytes_are_kept_in_order_and_an_unreadable_list_is_an_error() {
    let dir = Scratch::new("pending");
    let cache = BlobCache::new(&dir.at("blobs"));
    assert!(cache.pending().unwrap().is_empty());
    let mut files = IndexMap::new();
    files.insert("b.wav".to_string(), "ref-b".to_string());
    files.insert("a.wav".to_string(), "ref-a".to_string());
    cache.save_pending(&files).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.at("blobs/pending.json")).unwrap(),
        "{\"b.wav\":\"ref-b\",\"a.wav\":\"ref-a\"}"
    );
    assert_eq!(
        cache.pending().unwrap().keys().collect::<Vec<_>>(),
        ["b.wav", "a.wav"]
    );
    std::fs::write(dir.at("blobs/pending.json"), "{").unwrap();
    let e = cache.pending().unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidData);
    assert!(e.to_string().starts_with("pending.json"), "{e}");
    std::fs::remove_file(dir.at("blobs/pending.json")).unwrap();
    std::fs::create_dir(dir.at("blobs/pending.json")).unwrap();
    assert!(cache.pending().is_err());
}

#[test]
fn the_confirmed_mirror_holds_text_by_path_and_refuses_a_path_that_climbs() {
    let dir = Scratch::new("confirmed");
    let mut c = ConfirmedDir::new(&dir.at("confirmed"));
    assert!(c.files().unwrap().is_empty());
    assert_eq!(c.get("a/b.txt").unwrap(), None);
    c.set("a/b.txt", Some("one")).unwrap();
    c.set("z.k7x", Some("two")).unwrap();
    c.set("a/c.txt", Some("three")).unwrap();
    assert_eq!(c.get("a/b.txt").unwrap().as_deref(), Some("one"));
    assert_eq!(c.files().unwrap(), ["z.k7x", "a/b.txt", "a/c.txt"]);
    // A torn write's temp is no file of the room.
    std::fs::write(dir.at("confirmed/a/.b.txt.123.deadbeef.tmp"), "x").unwrap();
    assert_eq!(c.files().unwrap().len(), 3);
    c.set("a/b.txt", None).unwrap();
    c.set("a/b.txt", None).unwrap();
    assert_eq!(c.get("a/b.txt").unwrap(), None);
    for bad in ["../x", "/x", "C:/x", "a\\b", ""] {
        assert!(
            c.get(bad).is_err() && c.set(bad, Some("x")).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn only_not_found_reads_as_absent_in_the_confirmed_mirror() {
    let dir = Scratch::new("confirmed-below");
    let mut c = ConfirmedDir::new(&dir.at("confirmed"));
    c.set("a", Some("a file")).unwrap();
    // Below a file is NotFound on Windows and NotADirectory elsewhere: absent on every host, so `a` and `a/b` can both apply.
    assert_eq!(c.get("a/b").unwrap(), None);
    c.set("a/b", None).unwrap();
    // A folder where the file belongs cannot be read as text.
    std::fs::create_dir_all(dir.at("confirmed/d.txt")).unwrap();
    assert!(c.get("d.txt").is_err());
}

#[test]
fn an_indexed_confirmed_dir_writes_only_a_text_that_differs_and_never_trusts_a_failed_write() {
    let dir = Scratch::new("confirmed-index");
    let root = dir.at("confirmed");
    let mut c = ConfirmedDir::new(&root);
    c.set("a.md", Some("one")).unwrap();
    c.set("b/c.md", Some("two")).unwrap();
    let mut c = ConfirmedDir::indexed(&root);
    // The same text, and the removal of a file never held, touch no disk.
    c.set("a.md", Some("one")).unwrap();
    c.set("gone.md", None).unwrap();
    assert_eq!(c.writes(), 0);
    c.set("a.md", Some("changed")).unwrap();
    c.set("b/c.md", None).unwrap();
    assert_eq!(c.writes(), 2);
    assert_eq!(c.get("a.md").unwrap().as_deref(), Some("changed"));
    assert_eq!(c.get("b/c.md").unwrap(), None);
    // A write that fails leaves nothing in the index: the same text is written once the way is clear.
    std::fs::create_dir_all(format!("{root}/d.md")).unwrap();
    assert!(c.set("d.md", Some("three")).is_err());
    std::fs::remove_dir(format!("{root}/d.md")).unwrap();
    c.set("d.md", Some("three")).unwrap();
    assert_eq!(c.get("d.md").unwrap().as_deref(), Some("three"));
}

#[cfg(unix)]
#[test]
fn a_confirmed_folder_that_cannot_be_listed_gets_no_index_so_a_removal_still_lands() {
    use crate::support::set_mode;
    let dir = Scratch::new("confirmed-unlisted");
    let root = dir.at("confirmed");
    let mut c = ConfirmedDir::new(&root);
    c.set("a.md", Some("one")).unwrap();
    c.set("sub/b.md", Some("two")).unwrap();
    assert!(set_mode(&format!("{root}/sub"), 0o000));
    let mut c = ConfirmedDir::indexed(&root);
    assert!(set_mode(&format!("{root}/sub"), 0o755));
    // An error listing is not an empty folder: with an index of nothing, this removal would be skipped.
    c.set("a.md", None).unwrap();
    assert_eq!(c.get("a.md").unwrap(), None);
    assert_eq!(c.files().unwrap(), ["sub/b.md"]);
}

#[test]
fn a_batch_holds_its_sets_in_memory_answers_from_them_and_writes_them_out_on_commit() {
    let dir = Scratch::new("confirmed-batch");
    let root = dir.at("confirmed");
    let mut c = ConfirmedDir::new(&root);
    c.set("a.md", Some("one")).unwrap();
    c.set("z/old.md", Some("gone soon")).unwrap();
    let mut c = ConfirmedDir::indexed(&root);
    c.begin_batch();
    c.set("a.md", Some("two")).unwrap();
    c.set("b/new.md", Some("new")).unwrap();
    c.set("c.md", Some("top")).unwrap();
    c.set("z/old.md", None).unwrap();
    // The disk is untouched, the batch answers, and the listing keeps the walk's order: files, then folders.
    assert_eq!(c.writes(), 0);
    assert_eq!(
        std::fs::read_to_string(format!("{root}/a.md")).unwrap(),
        "one"
    );
    assert!(!std::path::Path::new(&format!("{root}/b/new.md")).exists());
    assert_eq!(c.get("a.md").unwrap().as_deref(), Some("two"));
    assert_eq!(c.get("z/old.md").unwrap(), None);
    assert_eq!(c.files().unwrap(), ["a.md", "c.md", "b/new.md"]);
    assert!(c.commit().is_empty());
    assert_eq!(c.writes(), 4);
    assert_eq!(
        std::fs::read_to_string(format!("{root}/a.md")).unwrap(),
        "two"
    );
    assert_eq!(c.files().unwrap(), ["a.md", "c.md", "b/new.md"]);
    // A file that cannot be written is named, and written again by the next set of the same text.
    std::fs::create_dir_all(format!("{root}/d.md")).unwrap();
    c.begin_batch();
    c.set("d.md", Some("three")).unwrap();
    assert_eq!(c.commit(), ["d.md"]);
    std::fs::remove_dir(format!("{root}/d.md")).unwrap();
    c.set("d.md", Some("three")).unwrap();
    assert_eq!(c.get("d.md").unwrap().as_deref(), Some("three"));
}
