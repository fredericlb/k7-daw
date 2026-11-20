// families.rs — a family folder read into values by id and findings by file, and the cache's file layer under it.
// A reader would get wrong that a broken file is named and left out while its neighbours still read.

use crate::support;

use song::{Cache, Clip, DiskSource, MemSource, Sequence, Source, SourceBundle, Timeline, read_family};
use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, Instant, SystemTime};
use support::{Tmp, mem};

#[test]
fn reads_in_listing_order_and_names_the_file_that_does_not_decode() {
    let src = mem(&[
        ("clips/snare.k7x", "source(\"snare.wav\")\n"),
        ("clips/broken.k7x", "source(\"a.wav\")\nfrom(2)\nto(1)\n"),
        ("clips/kick.k7x", "source(\"kick.wav\")\n"),
        ("clips/notes.txt", "not a document\n"),
        ("clips/.k7x", "source(\"hidden.wav\")\n"),
        ("clips/UP.K7X", "source(\"up.wav\")\n"),
    ]);
    let fam = read_family::<Clip>(&src, &mut Cache::new(), "/p").expect("reads");
    let ids: Vec<&str> = fam.values.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["UP", "kick", "snare"]);
    assert_eq!(fam.findings.len(), 1);
    assert_eq!(fam.findings[0].file, "clips/broken.k7x");
    assert!(fam.get("kick").is_some() && fam.get("broken").is_none());
}

#[test]
fn a_missing_family_folder_is_empty() {
    let src = mem(&[("clips/a.k7x", "source(\"a.wav\")\n")]);
    let fam = read_family::<Timeline>(&src, &mut Cache::new(), "/p").expect("reads");
    assert!(fam.values.is_empty() && fam.findings.is_empty());
}

#[test]
fn a_folder_the_bundle_holds_no_listing_for_is_missing_not_empty() {
    let bare = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: BTreeMap::new(),
        lists: BTreeMap::new(),
        ..Default::default()
    };
    let src = MemSource::from_bundle(bare).expect("a source");
    assert!(read_family::<Sequence>(&src, &mut Cache::new(), "/p").is_err());
}

#[test]
fn a_sequence_family_expands_its_short_form() {
    let src = mem(&[
        ("sequences/m.k7x", "notes(\"c4 d4\")\n"),
        ("sequences/l.k7x", "notes([t(0).d(1).k(60)])\n"),
    ]);
    let fam = read_family::<Sequence>(&src, &mut Cache::new(), "/p").expect("reads");
    assert_eq!(fam.get("m").map(|s| s.notes.0.len()), Some(2));
    assert_eq!(fam.get("l").map(|s| s.notes.0.len()), Some(1));
}

#[test]
fn a_memory_source_always_settles_so_the_cache_serves_the_same_value() {
    // yamlCache.test.ts: serves a settled file from the cache — the same object, not merely an equal one.
    let src = mem(&[("clips/a.k7x", "source(\"a.wav\")\nfrom(0.25)\n")]);
    let mut cache = Cache::new();
    let a = read_family::<Clip>(&src, &mut cache, "/p").expect("reads");
    let b = read_family::<Clip>(&src, &mut cache, "/p").expect("reads");
    assert!(std::sync::Arc::ptr_eq(&a.values[0].1, &b.values[0].1));
    assert_eq!(cache.len(), 1);
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn a_draft_is_decoded_afresh() {
    // yamlCache.test.ts: re-reads when the stamp moved.
    let mut src = mem(&[("clips/a.k7x", "source(\"a.wav\")\nfrom(0.25)\n")]);
    let mut cache = Cache::new();
    let a = read_family::<Clip>(&src, &mut cache, "/p").expect("reads");
    src.set("/p/clips/a.k7x", Some("source(\"a.wav\")\nfrom(0.5)\n".into()))
        .expect("set");
    let b = read_family::<Clip>(&src, &mut cache, "/p").expect("reads");
    assert!(!std::sync::Arc::ptr_eq(&a.values[0].1, &b.values[0].1));
    let Clip::Audio(clip) = &*b.values[0].1 else {
        panic!("an audio clip")
    };
    assert_eq!(clip.from.as_ref().and_then(song::files::WindowEdge::seconds), Some(0.5));
}

fn backdate(t: &Tmp, rel: &str) {
    let old = SystemTime::now() - Duration::from_secs(60);
    fs::OpenOptions::new()
        .write(true)
        .open(t.0.join(rel))
        .expect("open")
        .set_modified(old)
        .expect("backdate");
}

#[test]
fn a_fresh_disk_file_is_never_cached_and_a_settled_one_is() {
    // yamlCache.test.ts: does not cache a file it read while the file was still fresh.
    let t = Tmp::new("fam");
    t.write("song/clips/a.k7x", "source(\"a.wav\")\n");
    let src = DiskSource::new(&[t.at("song")]).expect("source");
    let mut cache = Cache::new();
    let a = read_family::<Clip>(&src, &mut cache, &t.at("song")).expect("reads");
    let b = read_family::<Clip>(&src, &mut cache, &t.at("song")).expect("reads");
    assert!(!std::sync::Arc::ptr_eq(&a.values[0].1, &b.values[0].1));
    assert!(cache.is_empty());
    backdate(&t, "song/clips/a.k7x");
    let c = read_family::<Clip>(&src, &mut cache, &t.at("song")).expect("reads");
    let d = read_family::<Clip>(&src, &mut cache, &t.at("song")).expect("reads");
    assert!(std::sync::Arc::ptr_eq(&c.values[0].1, &d.values[0].1));
}

#[test]
fn re_reads_when_only_the_size_moved_under_an_unchanged_mtime() {
    // yamlCache.test.ts: re-reads when only the SIZE moved under an unchanged mtime.
    let t = Tmp::new("fam-size");
    t.write("song/clips/a.k7x", "source(\"a.wav\")\n");
    backdate(&t, "song/clips/a.k7x");
    let src = DiskSource::new(&[t.at("song")]).expect("source");
    let mut cache = Cache::new();
    let before = read_family::<Clip>(&src, &mut cache, &t.at("song")).expect("reads");
    let mtime = src.stat(&t.at("song/clips/a.k7x")).expect("stat").stamp.mtime_ns;
    t.write("song/clips/a.k7x", "source(\"a.wav\")\nfrom(0.5)\n");
    backdate(&t, "song/clips/a.k7x");
    let at = std::time::UNIX_EPOCH + Duration::from_nanos(mtime);
    fs::OpenOptions::new()
        .write(true)
        .open(t.0.join("song/clips/a.k7x"))
        .expect("open")
        .set_modified(at)
        .expect("same mtime");
    let after = read_family::<Clip>(&src, &mut cache, &t.at("song")).expect("reads");
    assert!(!std::sync::Arc::ptr_eq(&before.values[0].1, &after.values[0].1));
}

#[test]
fn a_file_the_listing_names_but_a_link_leads_out_is_a_refusal_not_a_finding() {
    let t = Tmp::new("fam-out");
    t.write("outside/a.k7x", "source(\"a.wav\")\n");
    t.mkdir("song");
    let linked = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(t.0.join("outside"), t.0.join("song/clips")).is_ok()
        }
        #[cfg(not(unix))]
        {
            false
        }
    };
    if !linked {
        return;
    }
    let src = DiskSource::new(&[t.at("song")]).expect("source");
    assert!(read_family::<Clip>(&src, &mut Cache::new(), &t.at("song")).is_err());
}

#[test]
fn one_cache_over_two_bundles_never_serves_the_other_ones_text() {
    // Same path, same length, other text: the stamp is the text's hash, so the cache cannot mix them up.
    let (a, b) = (
        mem(&[("clips/a.k7x", "source(\"a1.wav\")\n")]),
        mem(&[("clips/a.k7x", "source(\"a2.wav\")\n")]),
    );
    let mut cache = Cache::new();
    let first = read_family::<Clip>(&a, &mut cache, "/p").expect("reads");
    let second = read_family::<Clip>(&b, &mut cache, "/p").expect("reads");
    let source = |f: &song::FamilyRead<Clip>| match &*f.values[0].1 {
        Clip::Audio(c) => c.source.0.clone(),
        _ => String::new(),
    };
    assert_eq!(
        (source(&first), source(&second)),
        ("a1.wav".to_owned(), "a2.wav".to_owned())
    );
}

#[test]
fn a_file_that_is_not_utf8_is_a_finding_for_that_file_and_the_family_still_reads() {
    let t = Tmp::new("fam-utf8");
    t.write("song/clips/ok.k7x", "source(\"a.wav\")\n");
    fs::write(t.0.join("song/clips/bin.k7x"), [0xff, 0xfe]).expect("write");
    let src = DiskSource::new(&[t.at("song")]).expect("source");
    let fam = read_family::<Clip>(&src, &mut Cache::new(), &t.at("song")).expect("reads");
    assert_eq!(fam.values.len(), 1);
    assert_eq!(fam.findings[0].file, "clips/bin.k7x");
}

#[test]
fn a_repeated_base_name_reads_only_the_last_file() {
    let src = mem(&[
        ("clips/a.k7x", "source(\"first.wav\")\n"),
        ("clips/a.K7X", "nonsense(1)\n"),
    ]);
    let fam = read_family::<Clip>(&src, &mut Cache::new(), "/p").expect("reads");
    // Bytewise `a.K7X` comes first and `a.k7x` last, which wins; the first is never read, so it cannot be a finding.
    assert_eq!(fam.values.len(), 1);
    assert!(fam.findings.is_empty());
}

// A listing of 40,000 files is indexed by base name, not searched once per file; order and the last-file rule hold.
#[test]
fn a_listing_of_forty_thousand_files_reads_in_linear_time() {
    let names: Vec<String> = (0..40_000).map(|n| format!("clips/c{n}.k7x")).collect();
    let twins: Vec<String> = (0..40_000).step_by(1000).map(|n| format!("clips/c{n}.K7X")).collect();
    let mut files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "source(\"a.wav\")\n")).collect();
    files.extend(twins.iter().map(|n| (n.as_str(), "source(\"twin.wav\")\n")));
    let src = mem(&files);
    let started = Instant::now();
    let fam = read_family::<Clip>(&src, &mut Cache::new(), "/p").expect("reads");
    assert!(
        started.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 36 } else { 9 }),
        "took {:?}",
        started.elapsed()
    );
    assert_eq!(fam.values.len(), 40_000);
    let ids: Vec<&str> = fam.values.iter().take(3).map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["c0", "c1", "c10"]);
    // A repeated base keeps the first position and the last file: the listing is sorted, so `K7X` comes first.
    assert!(fam.names.iter().any(|n| n == "c1000.k7x" || n == "c1000.K7X"));
    assert_eq!(fam.names.iter().filter(|n| n.starts_with("c1000.")).count(), 1);
}
