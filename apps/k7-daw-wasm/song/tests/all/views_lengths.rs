// views_lengths.rs — how a clip's audio is measured: ByHeader through a Source, ByTable from a table, the cache between.
// A reader would get wrong that a bundle cannot measure: it holds a binary by its length, so a replay uses ByTable (611).

use crate::support;
use crate::views_support;

use song::views::lengths::{ByHeader, ByTable, Lengths, MAX_HEADERS, Measure};
use song::{
    BundleFile, Cache, DiskSource, Entry, Kind, MemSource, Severity, Source, SourceBundle, SourceError, Span, Stamp,
    Stat,
};
use std::cell::Cell;
use std::collections::BTreeMap;
use support::Tmp;
use views_support::{fast, wav};

/// Counts the header reads a Source is asked for.
struct Reads<S> {
    inner: S,
    reads: Cell<usize>,
}

impl<S: Source> Source for Reads<S> {
    fn stat(&self, p: &str) -> Result<Stat, SourceError> {
        self.inner.stat(p)
    }
    fn list(&self, d: &str) -> Result<Vec<Entry>, SourceError> {
        self.inner.list(d)
    }
    fn read(&self, p: &str, s: Span) -> Result<Vec<u8>, SourceError> {
        self.reads.set(self.reads.get() + 1);
        self.inner.read(p, s)
    }
}

fn disk(t: &Tmp) -> Reads<DiskSource> {
    Reads {
        inner: DiskSource::new(&[support::spell(&t.0)]).expect("mounts"),
        reads: Cell::new(0),
    }
}

fn put(t: &Tmp, name: &str, bytes: &[u8]) {
    let path = t.0.join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::write(path, bytes).expect("write");
}

#[test]
fn measures_a_wav_and_answers_none_for_what_is_not_one() {
    // decision 611 and the throw-or-finding table: absent, a folder, a 0-byte file, not RIFF are None.
    let t = Tmp::new("len-kinds");
    put(&t, "a.wav", &wav(2.0));
    put(&t, "empty.wav", b"");
    put(&t, "note.txt", b"hello, this is not a riff file");
    t.mkdir("dir");
    t.age();
    let src = disk(&t);
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    let secs = |m: &mut ByHeader, n: &str| m.seconds(&t.at(n)).expect("answers");
    assert!((secs(&mut m, "a.wav").expect("a length") - 2.0).abs() < 1e-9);
    for n in ["empty.wav", "note.txt", "dir", "absent.wav"] {
        assert_eq!(secs(&mut m, n), None, "{n}");
    }
    let (lengths, findings) = m.finish();
    assert_eq!(lengths.keys().collect::<Vec<_>>(), [&t.at("a.wav")]);
    assert!(findings.is_empty());
}

#[test]
fn a_path_the_source_refuses_costs_that_path_only() {
    // throw-or-finding: a file the Source refuses answers None.
    let t = Tmp::new("len-out");
    let src = disk(&t);
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    assert_eq!(m.seconds("/definitely/outside/x.wav").expect("answers"), None);
}

#[test]
fn one_path_asked_a_hundred_thousand_times_is_one_read() {
    // hostile table (ByHeader): memoised per path for the call.
    let t = Tmp::new("len-memo");
    put(&t, "a.wav", &wav(1.0));
    t.age();
    let src = disk(&t);
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    let p = t.at("a.wav");
    fast("100,000 asks", || {
        for _ in 0..100_000 {
            assert!(m.seconds(&p).expect("answers").is_some());
        }
    });
    drop(m);
    assert_eq!(src.reads.get(), 1);
}

#[test]
fn a_settled_stamp_is_held_across_calls_and_no_read_is_made_twice() {
    // decision 613: a length is memoised in the Cache by path and SETTLED stamp.
    let t = Tmp::new("len-cache");
    for i in 0..30 {
        put(&t, &format!("w{i}.wav"), &wav(0.25));
    }
    t.age();
    let src = disk(&t);
    let mut cache = Cache::new();
    let paths: Vec<String> = (0..30).map(|i| t.at(&format!("w{i}.wav"))).collect();
    for pass in 0..2 {
        let mut m = ByHeader::new(&src, &mut cache);
        m.measure_all(paths.iter().map(String::as_str)).expect("measures");
        let (lengths, findings) = m.finish();
        assert_eq!((lengths.len(), findings.len()), (30, 0));
        assert_eq!(src.reads.get(), 30, "after pass {pass}");
    }
}

#[test]
fn an_unsettled_stamp_is_measured_every_time() {
    // decision 613: a write of the same size in the same tick would reproduce an unsettled stamp.
    let t = Tmp::new("len-fresh");
    put(&t, "a.wav", &wav(0.25));
    let src = disk(&t);
    let mut cache = Cache::new();
    for _ in 0..2 {
        let mut m = ByHeader::new(&src, &mut cache);
        m.measure_all([t.at("a.wav").as_str()].into_iter()).expect("measures");
    }
    assert_eq!(src.reads.get(), 2);
    let stamp = Stamp {
        mtime_ns: 1,
        len: 1,
        settled: false,
    };
    cache.hold_length("/x", stamp, Some(1.0));
    assert_eq!(cache.length_of("/x", stamp), None);
}

#[test]
fn the_cache_map_is_dropped_whole_past_its_limit() {
    // decision 613: past 100,000 entries the map is dropped, never grown.
    let mut cache = Cache::new();
    let stamp = Stamp {
        mtime_ns: 1,
        len: 1,
        settled: true,
    };
    for i in 0..100_001 {
        cache.hold_length(&format!("/p{i}"), stamp, Some(1.0));
    }
    assert_eq!(cache.length_of("/p0", stamp), None);
    assert_eq!(cache.length_of("/p100000", stamp), Some(Some(1.0)));
}

/// A Source of many files that are not WAVs, counting reads.
struct Many(Cell<usize>);

impl Source for Many {
    fn stat(&self, _: &str) -> Result<Stat, SourceError> {
        Ok(Stat {
            kind: Kind::File,
            len: 10,
            stamp: Stamp {
                mtime_ns: 5,
                len: 10,
                settled: true,
            },
        })
    }
    fn list(&self, _: &str) -> Result<Vec<Entry>, SourceError> {
        Err(SourceError::NotFound)
    }
    fn read(&self, _: &str, _: Span) -> Result<Vec<u8>, SourceError> {
        self.0.set(self.0.get() + 1);
        Ok(vec![0; 10])
    }
}

#[test]
fn past_the_header_limit_a_path_answers_none_and_one_warning_is_named() {
    // decision 614: 25,000 distinct files over a counting Source: 20,000 reads, one WARNING named `clips`.
    let src = Many(Cell::new(0));
    let mut cache = Cache::new();
    let names: Vec<String> = (0..25_000).map(|i| format!("/a/{i}.wav")).collect();
    let mut m = ByHeader::new(&src, &mut cache);
    fast("25,000 files", || {
        m.measure_all(names.iter().map(String::as_str)).expect("measures")
    });
    let (lengths, findings) = m.finish();
    assert!(lengths.is_empty());
    assert_eq!(src.0.get(), MAX_HEADERS);
    assert_eq!(
        (findings.len(), findings[0].file.as_str(), findings[0].severity),
        (1, "clips", Severity::Warning)
    );
    // The cache answered what was read: the next call reads only the 5,000 left.
    let mut m = ByHeader::new(&src, &mut cache);
    m.measure_all(names.iter().map(String::as_str)).expect("measures");
    assert_eq!(src.0.get(), 25_000);
    assert!(m.finish().1.is_empty());
}

/// A Source that holds nothing and counts the stats it was asked.
struct Gone(Cell<usize>);

impl Source for Gone {
    fn stat(&self, _: &str) -> Result<Stat, SourceError> {
        self.0.set(self.0.get() + 1);
        Err(SourceError::NotFound)
    }
    fn list(&self, _: &str) -> Result<Vec<Entry>, SourceError> {
        Err(SourceError::NotFound)
    }
    fn read(&self, _: &str, _: Span) -> Result<Vec<u8>, SourceError> {
        Err(SourceError::NotFound)
    }
}

#[test]
fn the_walk_over_distinct_paths_is_weighed_too() {
    // decision 614: a path is a stat and a copy; past the budget a new path costs nothing and one WARNING names `clips`.
    let src = Gone(Cell::new(0));
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    let names: Vec<String> = (0..1_200_000).map(|i| format!("/a/{i}")).collect();
    fast("1,200,000 paths", || {
        m.measure_all(names.iter().map(String::as_str)).expect("measures")
    });
    let (lengths, findings) = m.finish();
    assert!(lengths.is_empty());
    let stats = src.0.get();
    assert!((900_000..=1_000_000).contains(&stats), "{stats}");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        (findings[0].file.as_str(), findings[0].severity),
        ("clips", Severity::Warning)
    );
}

#[test]
fn a_bundle_cannot_measure_and_says_missing() {
    // decision 611: ByHeader over a MemSource holding a binary by its length answers Missing, which is why ByTable exists.
    let mut files = BTreeMap::new();
    files.insert(
        "/p/a.wav".to_owned(),
        BundleFile {
            text: None,
            len: Some(100),
            ..Default::default()
        },
    );
    let mut lists = BTreeMap::new();
    lists.insert(
        "/p".to_owned(),
        vec![Entry {
            name: "a.wav".into(),
            kind: Kind::File,
        }],
    );
    let src = MemSource::from_bundle(SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files,
        lists,
        ..Default::default()
    })
    .expect("bundle");
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    assert_eq!(m.seconds("/p/a.wav"), Err(SourceError::Missing));
}

#[test]
fn a_table_answers_what_it_holds_and_none_for_the_rest() {
    // decision 611: ByTable answers Ok(None) for a path not in it and never Missing.
    let table: Lengths = [("/a.wav".to_owned(), 1.5)].into_iter().collect();
    let mut m = ByTable(&table);
    assert_eq!(m.seconds("/a.wav"), Ok(Some(1.5)));
    assert_eq!(m.seconds("/b.wav"), Ok(None));
}

#[test]
fn a_bundle_answers_the_seconds_it_measured_when_it_was_made() {
    // 3c: a walk's bundle holds a binary by its length and its seconds beside it; ByHeader reads the seconds, never a header.
    let mut files = BTreeMap::new();
    files.insert(
        "/p/a.wav".to_owned(),
        BundleFile {
            len: Some(100),
            binary: Some(true),
            ..Default::default()
        },
    );
    let mut lists = BTreeMap::new();
    lists.insert(
        "/p".to_owned(),
        vec![Entry {
            name: "a.wav".into(),
            kind: Kind::File,
        }],
    );
    let src = MemSource::from_bundle(SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files,
        lists,
        lengths: [("/p/a.wav".to_owned(), 12.345_678_9)].into_iter().collect(),
        ..Default::default()
    })
    .expect("bundle");
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    assert_eq!(m.seconds("/p/a.wav"), Ok(Some(12.345_678_9)));
}
