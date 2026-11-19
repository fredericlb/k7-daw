// bundle_hostile.rs — a bundle or a recording built to make the Source slow or the bundle huge, each answered in time (743, 752, 753, 791, 792, 797).
// A reader would get wrong that the bound is wall clock: fifteen seconds, sixty on musl, to catch a runaway walk and not to time it.

#[cfg(unix)]
use crate::support;

use song::record::Recorder;
use song::{
    BundleFile, Entry, Kind, MAX_BUNDLE, MAX_ENTRIES, MemSource, Source, SourceBundle, SourceError, Span, Stat,
};
use std::time::{Duration, Instant};

fn bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

fn bundle(mounts: &[&str]) -> SourceBundle {
    SourceBundle {
        format: 1,
        mounts: mounts.iter().map(|m| (*m).to_owned()).collect(),
        ..SourceBundle::default()
    }
}

fn deep(base: &str, names: usize) -> String {
    format!("{base}{}", "/a".repeat(names))
}

// Decision 792: the lookup of a path beyond the deepest held one starts at that depth.
#[test]
fn a_hundred_thousand_refused_paths_and_a_path_a_million_names_deep_below_none() {
    let mut b = bundle(&["/p"]);
    for i in 0..100_000 {
        b.refused.insert(format!("/p/r{i}/link"));
    }
    let src = MemSource::from_bundle(b).expect("a source");
    let start = Instant::now();
    let path = deep("/p/clear", 1_000_000);
    // Not refused, and held nowhere: an answer from the shallow end, not a walk of a million folders.
    assert_eq!(src.stat(&path), Err(SourceError::Missing));
    assert_eq!(src.stat("/p/r99999/link/below/it"), Err(SourceError::Outside));
    assert_eq!(src.stat("/p/r99999/other"), Err(SourceError::Missing));
    assert!(start.elapsed() < bound(), "{:?}", start.elapsed());
}

#[test]
fn a_refused_entry_of_depth_two_and_a_stat_at_depth_ten_thousand() {
    let mut b = bundle(&["/p"]);
    b.refused.insert("/p/link".into());
    let src = MemSource::from_bundle(b).expect("a source");
    let start = Instant::now();
    assert_eq!(src.stat(&deep("/p/link", 10_000)), Err(SourceError::Outside));
    assert_eq!(src.read(&deep("/p/link", 10_000), Span::All), Err(SourceError::Outside));
    assert_eq!(src.list("/p/link"), Err(SourceError::Outside));
    // A sibling of the same depth is not caught by it.
    assert_eq!(src.stat(&deep("/p/linked", 10_000)), Err(SourceError::Missing));
    assert!(start.elapsed() < bound());
}

#[test]
fn two_hundred_thousand_absent_stats_over_a_hundred_thousand_held_paths_are_linear() {
    let mut b = bundle(&["/p"]);
    for i in 0..100_000 {
        b.files.insert(
            format!("/p/d{}/f{i}.k7x", i % 100),
            BundleFile {
                text: Some("x".into()),
                ..BundleFile::default()
            },
        );
    }
    let src = MemSource::from_bundle(b).expect("a source");
    let start = Instant::now();
    for i in 0..200_000 {
        // No listing is held, so each absent path is looked for below, by the index and not by a scan.
        assert_eq!(src.stat(&format!("/p/d{}/g{i}/h", i % 100)), Err(SourceError::Missing));
    }
    assert_eq!(src.stat("/p/d1").expect("a folder").kind, Kind::Dir);
    assert!(start.elapsed() < bound(), "{:?}", start.elapsed());
}

#[test]
fn a_binary_of_the_largest_length_is_one_byte_and_a_length() {
    let mut b = bundle(&["/p"]);
    b.files.insert(
        "/p/huge.bin".into(),
        BundleFile {
            len: Some(u64::MAX),
            binary: Some(true),
            ..BundleFile::default()
        },
    );
    let src = MemSource::from_bundle(b).expect("a source");
    assert_eq!(src.stat("/p/huge.bin").expect("stat").len, u64::MAX);
    assert_eq!(src.read("/p/huge.bin", Span::All), Ok(vec![0xFF]));
    assert!(src.read_text("/p/huge.bin").is_err());
    assert_eq!(
        src.read("/p/huge.bin", Span::Range { offset: 0, len: 4 }),
        Err(SourceError::Missing)
    );
}

#[test]
fn a_file_that_names_text_and_a_length_or_an_io_is_refused_when_the_source_is_made() {
    for f in [
        BundleFile {
            text: Some("x".into()),
            len: Some(1),
            ..BundleFile::default()
        },
        BundleFile {
            text: Some("x".into()),
            io: Some("denied".into()),
            ..BundleFile::default()
        },
    ] {
        let mut b = bundle(&["/p"]);
        b.files.insert("/p/a".into(), f);
        assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(_))));
    }
}

#[test]
fn a_failed_stat_or_list_answers_io_and_a_key_that_does_not_normalize_refuses_the_bundle() {
    let mut b = bundle(&["/p"]);
    b.failed_stats
        .insert("/p/a.wav/sub.wav".into(), "not a directory".into());
    b.failed_lists.insert("/p/closed".into(), "denied".into());
    b.files.insert(
        "/p/c.k7x".into(),
        BundleFile {
            len: Some(3),
            io: Some("denied".into()),
            ..BundleFile::default()
        },
    );
    let src = MemSource::from_bundle(b).expect("a source");
    assert_eq!(
        src.stat("/p/a.wav/sub.wav"),
        Err(SourceError::Io("not a directory".into()))
    );
    assert_eq!(
        src.read("/p/a.wav/sub.wav", Span::All),
        Err(SourceError::Io("not a directory".into()))
    );
    assert_eq!(src.list("/p/closed"), Err(SourceError::Io("denied".into())));
    assert_eq!(src.stat("/p/closed").expect("a folder").kind, Kind::Dir);
    assert_eq!(src.stat("/p/c.k7x").expect("stat").len, 3);
    assert_eq!(src.read("/p/c.k7x", Span::All), Err(SourceError::Io("denied".into())));
    let mut b = bundle(&["/p"]);
    b.failed_stats.insert("p/not/absolute".into(), "x".into());
    assert!(MemSource::from_bundle(b).is_err());
}

#[test]
fn a_bundle_past_the_entry_limit_is_not_read() {
    let mut b = bundle(&["/p"]);
    for i in 0..=MAX_ENTRIES {
        b.refused.insert(format!("/p/{i}"));
    }
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(_))));
}

/// A Source that answers whatever it is asked with one fixed answer.
struct Fixed {
    text: usize,
    outside: bool,
}

impl Source for Fixed {
    fn stat(&self, _path: &str) -> Result<Stat, SourceError> {
        Err(SourceError::Outside)
    }

    fn list(&self, _dir: &str) -> Result<Vec<Entry>, SourceError> {
        Err(SourceError::Outside)
    }

    fn read(&self, _path: &str, _span: Span) -> Result<Vec<u8>, SourceError> {
        if self.outside {
            Err(SourceError::Outside)
        } else {
            Ok(vec![b'a'; self.text])
        }
    }
}

// Decisions 791 and 796: the Recorder says it overflowed and the bundle it made says so too.
#[test]
fn a_recording_past_the_text_limit_stops_adding_and_says_so() {
    let inner = Fixed {
        text: 1 << 20,
        outside: false,
    };
    let rec = Recorder::new(&inner);
    let start = Instant::now();
    for i in 0..(MAX_BUNDLE >> 20) + 8 {
        assert!(rec.read(&format!("/p/{i}.k7x"), Span::All).is_ok());
    }
    assert!(rec.overflow().is_some_and(|s| s.contains("bytes")));
    let b = rec.into_bundle(&["/p".to_owned()]);
    let held: usize = b.files.values().filter_map(|f| f.text.as_ref()).map(String::len).sum();
    assert!(held <= MAX_BUNDLE && held > MAX_BUNDLE - (2 << 20), "{held}");
    // The bundle says it was cut, so no replay can take it for a whole one.
    assert!(b.overflow.as_deref().is_some_and(|s| s.contains("bytes")));
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("cut short")));
    assert!(start.elapsed() < bound());
}

#[test]
fn a_recording_past_the_entry_limit_stops_adding_and_says_so() {
    let rec = Recorder::new(&Fixed { text: 0, outside: true });
    let start = Instant::now();
    for i in 0..=MAX_ENTRIES {
        assert_eq!(rec.stat(&format!("/p/{i}")), Err(SourceError::Outside));
    }
    assert!(rec.overflow().is_some_and(|s| s.contains("refused")));
    let b = rec.into_bundle(&["/p".to_owned()]);
    assert_eq!(b.refused.len(), MAX_ENTRIES);
    // What it kept says it is cut, and is not replayed.
    assert!(b.overflow.as_deref().is_some_and(|s| s.contains("refused")));
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("cut short")));
    assert!(start.elapsed() < bound());
}

#[cfg(unix)]
#[test]
fn a_tree_where_every_folder_is_a_link_out_records_a_thousand_refusals() {
    let t = support::Tmp::new("hostile-links");
    t.mkdir("p");
    t.mkdir("outside");
    let (p, outside) = (t.0.join("p"), t.0.join("outside"));
    for i in 0..1_000 {
        if std::os::unix::fs::symlink(&outside, p.join(format!("l{i}"))).is_err() {
            support::skip("every_folder_is_a_link_out", "a link needs a privilege here");
            return;
        }
    }
    let project = t.at("p");
    let disk = song::DiskSource::new(std::slice::from_ref(&project)).expect("a source");
    let rec = Recorder::new(&disk);
    let start = Instant::now();
    for i in 0..1_000 {
        assert_eq!(rec.list(&format!("{project}/l{i}")), Err(SourceError::Outside));
    }
    assert_eq!(rec.overflow(), None);
    let b = rec.into_bundle(std::slice::from_ref(&project));
    assert_eq!(b.refused.len(), 1_000);
    let mem = MemSource::from_bundle(b).expect("a source");
    assert_eq!(mem.stat(&format!("{project}/l7/x/y")), Err(SourceError::Outside));
    assert!(start.elapsed() < bound());
}

#[test]
fn the_heads_of_a_path_are_slices_bounded_by_names_and_bytes() {
    use song::source::path::{any_head, depth, head};
    assert_eq!(
        (depth("/"), depth("/a"), depth("/a/b"), depth("C:/"), depth("C:/a/b")),
        (0, 1, 2, 0, 2)
    );
    assert_eq!(
        (head("/a/b/c", 0), head("/a/b/c", 2), head("/a/b/c", 9)),
        ("/", "/a/b", "/a/b/c")
    );
    assert_eq!((head("C:/a/b", 0), head("C:/a/b", 1)), ("C:/", "C:/a"));
    let seen = |path: &str, names, len| {
        let mut all = Vec::new();
        any_head(path, names, len, |h| {
            all.push(h.to_owned());
            false
        });
        all
    };
    assert_eq!(seen("/a/b/c", 2, 99), ["/", "/a", "/a/b"]);
    assert_eq!(seen("/a/b/c", 3, 99), ["/", "/a", "/a/b", "/a/b/c"]);
    assert_eq!(seen("/a/b/c", 9, 3), ["/", "/a"]);
    assert_eq!(seen("/", 9, 9), ["/"]);
}
