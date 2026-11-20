//! capture_blobs.rs — a capture with a blob cache: binaries sent once settled, a ref written for another replica, the
//! files waiting for bytes, and the index that spares a re-hash (`liveCapture.test.ts`'s binaries). What a reader gets
//! wrong: a ref the cache lacks is PENDING, never removed, even across a restart.

use k7_capture::cache::sha256_bytes;
use k7_js::number::text_hash;
use k7_replica::blob::blob_ref;
use serde_json::json;

use crate::rig::{MemBaseline, Rig, age, bytes, files, ops, put};

fn reference(data: &[u8]) -> String {
    let id = sha256_bytes(data);
    blob_ref(&id.hash, data.len() as u64).unwrap()
}

/// A written binary, settled and swept into a batch.
fn sent(r: &mut Rig, rel: &str, data: &[u8]) -> serde_json::Value {
    put(&r.root, rel, data);
    assert!(r.touched_now(&[rel]).is_none(), "young");
    r.clock.advance(100.0);
    r.cap.on_young();
    r.quiet().expect("the settled binary")
}

#[test]
fn a_written_audio_file_is_sent_once_settled_kept_and_named_on_removal() {
    let mut r = Rig::started("wav", &[], true);
    let data = bytes(4096, 7);
    let b = sent(&mut r, "take.wav", &data);
    let rf = reference(&data);
    assert_eq!(
        ops(&b),
        vec![json!({ "kind": "text", "file": "take.wav", "text": rf, "was": null })]
    );
    let path = r
        .cap
        .blob_file(&sha256_bytes(&data).hash)
        .unwrap()
        .expect("kept");
    assert_eq!(std::fs::read(path).unwrap(), data);
    std::fs::remove_file(format!("{}/take.wav", r.root)).unwrap();
    let b = r.touched(&["take.wav"]).unwrap();
    assert_eq!(
        ops(&b),
        vec![json!({ "kind": "remove", "file": "take.wav", "was": text_hash(&rf) })]
    );
}

#[test]
fn a_ref_whose_bytes_it_holds_is_written_and_echoes_nothing() {
    let mut r = Rig::started("place", &[], true);
    let data = bytes(2048, 3);
    sent(&mut r, "one.wav", &data);
    r.cap
        .write_known("two.wav", Some(&reference(&data)))
        .unwrap();
    assert_eq!(std::fs::read(format!("{}/two.wav", r.root)).unwrap(), data);
    assert_eq!(r.cap.read("two.wav").unwrap(), Some(reference(&data)));
    assert!(r.cap.take_missing().is_empty());
    assert!(r.touched(&["two.wav"]).is_none());
    assert!(r.sweep().is_none());
    assert_eq!(r.cap.take_batches().len(), 1);
}

#[test]
fn a_ref_it_lacks_the_bytes_of_is_pending_asked_for_and_written_once_fetched() {
    let mut r = Rig::started("pending", &[], true);
    let data = bytes(1024, 5);
    let id = sha256_bytes(&data);
    r.cap
        .write_known("far.wav", Some(&reference(&data)))
        .unwrap();
    assert_eq!(r.cap.take_missing(), vec![id.clone()]);
    assert_eq!(r.cap.read("far.wav").unwrap(), Some(reference(&data)));
    assert!(r.cap.pending_on(&id.hash));
    assert!(r.sweep().is_none());
    std::fs::write(format!("{}/blobs/{}", r.state, id.hash), &data).unwrap();
    let got = r.cap.fetched(&id.hash);
    assert!(got.failed.is_none());
    assert_eq!(got.placed, ["far.wav"]);
    assert_eq!(std::fs::read(format!("{}/far.wav", r.root)).unwrap(), data);
    assert!(!r.cap.pending_on(&id.hash));
    assert!(r.touched(&["far.wav"]).is_none());
    assert!(r.cap.take_batches().is_empty());
}

#[test]
fn it_still_waits_for_the_bytes_after_a_restart() {
    let mut r = Rig::started("restart", &[], true);
    let rf = reference(&bytes(512, 9));
    r.cap.write_known("far.wav", Some(&rf)).unwrap();
    r.reopen();
    let baseline = MemBaseline::of(&[("far.wav", &rf)]);
    r.cap.start(baseline.arc()).unwrap();
    assert!(r.cap.take_batches().is_empty());
    assert_eq!(r.cap.take_missing().len(), 1);
    assert_eq!(r.cap.read("far.wav").unwrap(), Some(rf));
}

#[test]
fn an_unchanged_binary_is_known_at_the_next_start_by_the_hash_it_kept() {
    let mut r = Rig::started("index", &[], true);
    r.cap.stop();
    let data = bytes(4096, 11);
    put(&r.root, "old.wav", &data);
    age(&r.root, "old.wav", 60);
    r.reopen();
    r.cap.start(None).unwrap();
    r.reopen();
    let index = format!("{}/blob-index.json", r.state);
    let mut saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
    assert_eq!(saved["files"]["old.wav"]["hash"], sha256_bytes(&data).hash);
    // A hash only the index could have given proves the file was not hashed again.
    let fake = "f".repeat(64);
    saved["files"]["old.wav"]["hash"] = json!(fake);
    std::fs::write(&index, saved.to_string()).unwrap();
    r.fresh();
    r.cap.start(None).unwrap();
    assert_eq!(
        r.cap.read("old.wav").unwrap(),
        Some(blob_ref(&fake, data.len() as u64).unwrap())
    );
    // A file whose stamp moved is hashed afresh.
    age(&r.root, "old.wav", 30);
    r.reopen();
    r.cap.start(None).unwrap();
    assert_eq!(r.cap.read("old.wav").unwrap(), Some(reference(&data)));
}

#[test]
fn a_large_text_that_is_not_k7x_travels_as_its_blob_ref() {
    let mut r = Rig::started("amp", &[], true);
    let model = format!("{{\"weights\":[{}]}}", vec!["0.125"; 40_000].join(","));
    assert!(model.len() > 128 * 1024);
    let b = sent(&mut r, "amp.json", model.as_bytes());
    assert_eq!(files(&Some(b.clone())), ["amp.json"]);
    assert_eq!(ops(&b)[0]["text"], reference(model.as_bytes()));
}

#[test]
fn a_dropped_scan_keeps_a_blob_a_waiting_file_needs() {
    let mut r = Rig::started("abort-keeps", &[], true);
    let data = bytes(1024, 9);
    let id = sha256_bytes(&data);
    r.cap
        .write_known("far.wav", Some(&reference(&data)))
        .unwrap();
    // Another file holds the same bytes: the scan copies them into the cache, which is what the download would have done.
    put(&r.root, "near.wav", &data);
    age(&r.root, "near.wav", 60);
    r.cap.touch("near.wav");
    let step = r.cap.step_flush(false).unwrap();
    r.cap.abort(step);
    assert!(
        std::path::Path::new(&format!("{}/blobs/{}", r.state, id.hash)).exists(),
        "the abort evicted the bytes a file still waits for"
    );
    let got = r.cap.fetched(&id.hash);
    assert_eq!(got.placed, ["far.wav"]);
}

#[test]
fn a_fetch_that_fails_midway_keeps_what_it_placed_and_the_bytes_for_the_next_try() {
    let mut r = Rig::started("fetch-partial", &[], true);
    let data = bytes(1024, 7);
    let id = sha256_bytes(&data);
    for f in ["a.wav", "b.wav"] {
        r.cap.write_known(f, Some(&reference(&data))).unwrap();
    }
    // The second file's place fails: a folder stands where its file goes.
    std::fs::create_dir_all(format!("{}/b.wav", r.root)).unwrap();
    std::fs::write(format!("{}/blobs/{}", r.state, id.hash), &data).unwrap();
    let got = r.cap.fetched(&id.hash);
    assert_eq!(got.placed, ["a.wav"]);
    assert!(got.failed.is_some());
    assert!(r.cap.pending_on(&id.hash), "b.wav still waits");
    assert!(std::path::Path::new(&format!("{}/blobs/{}", r.state, id.hash)).exists());
    std::fs::remove_dir_all(format!("{}/b.wav", r.root)).unwrap();
    let again = r.cap.fetched(&id.hash);
    assert_eq!(again.placed, ["b.wav"]);
    assert!(again.failed.is_none() && !r.cap.pending_on(&id.hash));
}

#[test]
fn a_young_binary_counts_as_uncaptured_until_it_settles_so_a_write_phase_never_hashes_it() {
    let mut r = Rig::started("wav-young", &[], true);
    put(&r.root, "take.wav", bytes(4096, 3));
    assert!(r.touched_now(&["take.wav"]).is_none(), "young");
    assert_eq!(r.cap.uncaptured(&["take.wav".to_string()]), ["take.wav"]);
    r.clock.advance(100.0);
    r.cap.on_young();
    r.quiet().expect("the settled binary");
    assert!(r.cap.uncaptured(&["take.wav".to_string()]).is_empty());
}
