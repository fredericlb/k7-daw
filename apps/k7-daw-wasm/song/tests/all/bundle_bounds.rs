// bundle_bounds.rs — a bundle past a bound of decision 797 is refused whole, and a real project is far inside every bound (753, 797).
// A reader would get wrong that the bounds are on KEYS and COUNTS too: a hostile bundle is small in text and huge in what it builds.

use song::{
    BundleFile, Entry, Kind, MAX_BUNDLE, MAX_MOUNTS, MAX_PATH_DEPTH, MAX_PATH_LEN, MemSource, Source, SourceBundle,
    SourceError,
};
use std::time::{Duration, Instant};

fn bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

fn bundle() -> SourceBundle {
    SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        ..SourceBundle::default()
    }
}

fn file(len: u64) -> BundleFile {
    BundleFile {
        len: Some(len),
        ..BundleFile::default()
    }
}

fn names(n: usize) -> String {
    format!("/p{}", "/a".repeat(n))
}

#[test]
fn a_key_of_twenty_thousand_names_is_refused_before_anything_is_built_from_it() {
    let mut b = bundle();
    b.files.insert(names(20_000), file(1));
    let start = Instant::now();
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("bundle key")));
    assert!(start.elapsed() < bound());
}

#[test]
fn a_key_past_the_depth_or_the_length_is_refused_in_every_map() {
    let long = format!("/p/{}", "x".repeat(MAX_PATH_LEN));
    let deep = names(MAX_PATH_DEPTH + 1);
    for key in [long, deep] {
        let mut maps: Vec<SourceBundle> = (0..9).map(|_| bundle()).collect();
        maps[0].files.insert(key.clone(), file(1));
        maps[1].lists.insert(key.clone(), Vec::new());
        maps[2].refused.insert(key.clone());
        maps[3].absent.insert(key.clone());
        maps[4].failed_stats.insert(key.clone(), "x".into());
        maps[5].failed_lists.insert(key.clone(), "x".into());
        maps[6].digests.insert(key.clone(), "x".into());
        maps[7].lengths.insert(key.clone(), 1.0);
        maps[8].jsdsp_stale.insert(key.clone());
        for (i, b) in maps.into_iter().enumerate() {
            assert!(
                MemSource::from_bundle(b).is_err(),
                "map {i}, key of {} bytes",
                key.len()
            );
        }
    }
}

#[test]
fn a_deep_real_path_is_inside_the_bound() {
    // Six hundred names of eleven bytes: far past any project k7 reads, inside both bounds.
    let mut b = bundle();
    let key = format!("/p{}", "/folder-name".repeat(600));
    assert!(key.len() <= MAX_PATH_LEN);
    b.files.insert(key.clone(), file(3));
    let src = MemSource::from_bundle(b).expect("a source");
    assert_eq!(src.stat(&key).expect("held").len, 3);
}

#[test]
fn many_distinct_deep_keys_are_refused_by_what_their_folders_would_weigh() {
    let mut b = bundle();
    // Each key is 2 KB and its folders sum to about 1 MB: 200 of them would build 200 MB of folders.
    for i in 0..200 {
        b.files.insert(format!("/p/d{i}{}", "/a".repeat(1_000)), file(1));
    }
    let start = Instant::now();
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("folders above")));
    assert!(start.elapsed() < bound());
}

#[test]
fn names_alone_past_the_bundle_limit_are_refused() {
    let mut b = bundle();
    // Files with no text: 40,000 keys of 3.6 KB are 144 MB of names.
    let pad = "n".repeat(3_600);
    for i in 0..40_000 {
        b.files.insert(format!("/p/{i:06}{pad}"), file(1));
    }
    assert!(b.weight() > MAX_BUNDLE);
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("weighs")));
}

#[test]
fn one_listing_whose_entries_outweigh_the_bundle_limit_is_refused() {
    let mut b = bundle();
    let entry = |i: usize| Entry {
        name: format!("{i:06}{}", "e".repeat(1_000)),
        kind: Kind::File,
    };
    b.lists.insert("/p".into(), (0..140_000).map(entry).collect());
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("weighs")));
}

#[test]
fn mounts_past_the_cap_are_refused_and_a_full_set_stays_fast() {
    let mut b = bundle();
    b.mounts = (0..=MAX_MOUNTS).map(|i| format!("/m{i}")).collect();
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("mounts")));
    let mut b = bundle();
    b.mounts = (0..200_000).map(|i| format!("/m{i}")).collect();
    assert!(MemSource::from_bundle(b).is_err());
    let mut b = bundle();
    b.mounts = (0..MAX_MOUNTS).map(|i| format!("/m{i}")).collect();
    b.mounts.push("/p".into());
    b.mounts.truncate(MAX_MOUNTS);
    let src = MemSource::from_bundle(b).expect("a source");
    let start = Instant::now();
    for i in 0..2_000 {
        let _ = src.stat(&format!("/nowhere/{i}"));
        let _ = src.stat(&format!("/m{}/x", i + 1_000));
    }
    assert!(start.elapsed() < bound(), "{:?}", start.elapsed());
}
