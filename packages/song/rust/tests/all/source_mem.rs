// source_mem.rs — MemSource and its bundle: what is Missing, what is NotFound, drafts and their generations.
// A reader would get wrong that a binary is held by length alone: its stat answers, its bytes are Missing.

use crate::support;

use song::{BundleFile, Entry, Kind, MemSource, Source, SourceBundle, SourceError, Span};
use std::collections::BTreeMap;
use support::mem;

#[test]
fn a_bundle_is_json() {
    let json = r#"{"format":1,"mounts":["/p"],
        "files":{"/p/clips/a.k7x":{"text":"x(1)\n"},"/p/a.wav":{"len":9}},
        "lists":{"/p":[{"name":"a.wav","kind":"file"},{"name":"clips","kind":"dir"}],"/p/clips":[{"name":"a.k7x","kind":"file"}]}}"#;
    let bundle: SourceBundle = serde_json::from_str(json).expect("a bundle");
    let src = MemSource::from_bundle(bundle.clone()).expect("a source");
    assert_eq!(src.stat("/p/a.wav").expect("stat").len, 9);
    assert_eq!(
        serde_json::from_str::<SourceBundle>(&serde_json::to_string(&bundle).expect("json")).expect("back"),
        bundle
    );
    assert!(
        serde_json::from_str::<SourceBundle>(r#"{"format":1,"mounts":[],"files":{},"lists":{},"extra":1}"#).is_err()
    );
}

#[test]
fn refuses_a_bundle_that_is_not_format_1_or_whose_file_holds_both() {
    let base = SourceBundle {
        format: 2,
        mounts: vec!["/p".into()],
        files: BTreeMap::new(),
        lists: BTreeMap::new(),
        ..Default::default()
    };
    assert!(matches!(MemSource::from_bundle(base.clone()), Err(SourceError::Io(_))));
    let mut both = SourceBundle { format: 1, ..base };
    both.files.insert(
        "/p/a".into(),
        BundleFile {
            text: Some("x".into()),
            len: Some(1),
            ..Default::default()
        },
    );
    assert!(matches!(MemSource::from_bundle(both), Err(SourceError::Io(_))));
}

#[test]
fn reads_text_and_ranges() {
    let src = mem(&[("clips/a.k7x", "0123456789")]);
    assert_eq!(src.read_text("/p/clips/a.k7x").expect("text"), "0123456789");
    assert_eq!(
        src.read("/p/clips/a.k7x", Span::Range { offset: 3, len: 4 }),
        Ok(b"3456".to_vec())
    );
    assert_eq!(
        src.read("/p/clips/./../clips/a.k7x", Span::Range { offset: 8, len: 50 }),
        Ok(b"89".to_vec())
    );
}

#[test]
fn a_path_nothing_holds_is_missing_but_a_listed_folder_that_omits_it_says_not_found() {
    let src = mem(&[("clips/a.k7x", "x(1)\n")]);
    assert_eq!(src.stat("/p/clips/b.k7x"), Err(SourceError::NotFound));
    assert_eq!(src.stat("/p/timelines"), Err(SourceError::NotFound));
    // The nearest listing held above a path decides: /p omits `sequences`, so nothing below it is there.
    assert_eq!(src.stat("/p/sequences/a.k7x"), Err(SourceError::NotFound));
    assert_eq!(src.list("/p/sequences"), Err(SourceError::NotFound));
    assert_eq!(src.stat("/p/clips").expect("a folder").kind, Kind::Dir);
}

#[test]
fn a_binary_is_a_length_and_nothing_else() {
    let mut bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: BTreeMap::new(),
        lists: BTreeMap::new(),
        ..Default::default()
    };
    bundle.files.insert(
        "/p/a.wav".into(),
        BundleFile {
            text: None,
            len: Some(44),
            ..Default::default()
        },
    );
    bundle.lists.insert(
        "/p".into(),
        vec![Entry {
            name: "a.wav".into(),
            kind: Kind::File,
        }],
    );
    let src = MemSource::from_bundle(bundle).expect("a source");
    assert_eq!(src.stat("/p/a.wav").expect("stat").len, 44);
    assert_eq!(
        src.read("/p/a.wav", Span::Range { offset: 0, len: 4 }),
        Err(SourceError::Missing)
    );
}

#[test]
fn refuses_what_lies_outside_every_mount() {
    let src = mem(&[("a.k7x", "x(1)\n")]);
    assert_eq!(src.stat("/q/a.k7x"), Err(SourceError::Outside));
    assert_eq!(src.read("/p/../q/a.k7x", Span::All), Err(SourceError::Outside));
    assert!(matches!(src.stat("p/a.k7x"), Err(SourceError::Io(_))));
}

#[test]
fn a_draft_is_laid_over_and_bumps_its_generation() {
    let mut src = mem(&[("clips/a.k7x", "x(1)\n")]);
    let before = src.stat("/p/clips/a.k7x").expect("stat").stamp;
    src.set("/p/clips/a.k7x", Some("x(22)\n".into())).expect("set");
    let after = src.stat("/p/clips/a.k7x").expect("stat");
    assert_ne!(before, after.stamp);
    assert!(after.stamp.settled);
    assert_eq!(after.len, 6);
    assert_eq!(src.read_text("/p/clips/a.k7x").expect("text"), "x(22)\n");
    src.set("/p/clips/new.k7x", Some("y(1)\n".into())).expect("set");
    let names: Vec<String> = src
        .list("/p/clips")
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["a.k7x", "new.k7x"]);
    src.set("/p/clips/a.k7x", None).expect("set");
    assert_eq!(src.stat("/p/clips/a.k7x"), Err(SourceError::NotFound));
    let names: Vec<String> = src
        .list("/p/clips")
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["new.k7x"]);
}

#[test]
fn lists_bytewise() {
    let src = mem(&[
        ("clips/b.k7x", "x(1)\n"),
        ("clips/B.k7x", "x(1)\n"),
        ("clips/a-b.k7x", "x(1)\n"),
        ("clips/a.k7x", "x(1)\n"),
    ]);
    let names: Vec<String> = src
        .list("/p/clips")
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["B.k7x", "a-b.k7x", "a.k7x", "b.k7x"]);
}

#[test]
fn a_bundle_with_no_listing_above_a_path_says_missing() {
    let bare = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: BTreeMap::new(),
        lists: BTreeMap::new(),
        ..Default::default()
    };
    let src = MemSource::from_bundle(bare).expect("a source");
    assert_eq!(src.stat("/p/a/b/c.k7x"), Err(SourceError::Missing));
    assert_eq!(src.list("/p/a"), Err(SourceError::Missing));
}

#[test]
fn a_draft_in_a_folder_known_to_be_absent_is_listed_not_missing() {
    let mut src = mem(&[("clips/a.k7x", "x(1)\n")]);
    assert_eq!(src.list("/p/sequences"), Err(SourceError::NotFound));
    src.set("/p/sequences/new.k7x", Some("notes(\"c4\")\n".into()))
        .expect("set");
    let names: Vec<String> = src
        .list("/p/sequences")
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["new.k7x"]);
    assert_eq!(src.stat("/p/sequences").expect("a folder").kind, Kind::Dir);
    // A folder only drafts lie in is wholly known, so another path in it is absent (2b decision 34).
    assert_eq!(src.stat("/p/sequences/other.k7x"), Err(SourceError::NotFound));
}

#[test]
fn two_bundles_with_the_same_path_and_length_and_other_text_have_other_stamps() {
    let (a, b) = (mem(&[("clips/a.k7x", "x(11)\n")]), mem(&[("clips/a.k7x", "x(22)\n")]));
    let (sa, sb) = (
        a.stat("/p/clips/a.k7x").expect("stat"),
        b.stat("/p/clips/a.k7x").expect("stat"),
    );
    assert_eq!(sa.len, sb.len);
    assert_ne!(sa.stamp, sb.stamp);
}

#[test]
fn a_draft_moves_its_folders_stamp_too() {
    let mut src = mem(&[("clips/a.k7x", "x(1)\n")]);
    let before = src.stat("/p/clips").expect("stat").stamp;
    src.set("/p/clips/a.k7x", Some("x(2)\n".into())).expect("set");
    assert_ne!(src.stat("/p/clips").expect("stat").stamp, before);
}

// Decision 28: a draft moves the generation of every folder up to its mount, and a folder no listing names gets listed.
#[test]
fn a_draft_two_levels_down_moves_every_folder_above_it_and_lists_the_new_folder() {
    let mut src = mem(&[("lib/audio/x/module.k7x", "plugin({})\n")]);
    let stamp = |s: &MemSource, d: &str| s.stat(d).expect("stat").stamp;
    let (lib_before, audio_before, root_before) =
        (stamp(&src, "/p/lib"), stamp(&src, "/p/lib/audio"), stamp(&src, "/p"));
    src.set("/p/lib/audio/NEW/module.k7x", Some("plugin({})\n".into()))
        .expect("draft");
    assert_ne!(stamp(&src, "/p/lib"), lib_before);
    assert_ne!(stamp(&src, "/p/lib/audio"), audio_before);
    assert_ne!(stamp(&src, "/p"), root_before);
    let names: Vec<String> = src
        .list("/p/lib/audio")
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["NEW", "x"]);
    assert_eq!(src.stat("/p/lib/audio/NEW").expect("stat").kind, Kind::Dir);
    // A folder only drafts lie in is wholly known: its other paths are absent, not missing.
    assert_eq!(src.stat("/p/lib/audio/NEW/ui.k7x"), Err(SourceError::NotFound));
    // A draft that opens two new folders lists the first, and the first lists the second.
    src.set("/p/lib/audio/Cat/leaf/module.k7x", Some("plugin({})\n".into()))
        .expect("draft");
    let cat: Vec<Entry> = src.list("/p/lib/audio/Cat").expect("list");
    assert_eq!(
        cat,
        [Entry {
            name: "leaf".into(),
            kind: Kind::Dir
        }]
    );
    // Taking the draft away again leaves the folder where the bundle had it.
    src.set("/p/lib/audio/NEW/module.k7x", None).expect("draft");
    assert!(src.stat("/p/lib/audio/NEW/module.k7x").is_err());
}

// Decision 79: a folder with a listing of its own is there, even when the listing above spells its name with another case.
#[test]
fn a_held_listing_makes_a_folder_exist_whatever_the_parent_spells() {
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/m".to_owned()],
        files: std::collections::BTreeMap::new(),
        lists: [
            (
                "/m/lib".to_owned(),
                vec![Entry {
                    name: "Samples".into(),
                    kind: Kind::Dir,
                }],
            ),
            ("/m/lib/samples".to_owned(), Vec::new()),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let src = MemSource::from_bundle(bundle).expect("a bundle");
    assert_eq!(src.stat("/m/lib/samples").expect("a folder").kind, Kind::Dir);
    assert_eq!(src.stat("/m/lib/nothing"), Err(SourceError::NotFound));
}

// A list or read of an absent path asks one lookup, not a scan of everything held (40,000 modules).
#[test]
fn asking_for_what_is_absent_does_not_scan_what_is_held() {
    let n = 40_000;
    let mut files = BTreeMap::new();
    let mut lists = BTreeMap::new();
    let mut top = Vec::new();
    for i in 0..n {
        files.insert(
            format!("/m/lib/mod{i}/module.k7x"),
            BundleFile {
                text: Some("x(1)\n".into()),
                len: None,
                ..Default::default()
            },
        );
        lists.insert(
            format!("/m/lib/mod{i}"),
            vec![Entry {
                name: "module.k7x".into(),
                kind: Kind::File,
            }],
        );
        top.push(Entry {
            name: format!("mod{i}"),
            kind: Kind::Dir,
        });
    }
    lists.insert("/m/lib".to_owned(), top);
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/m".to_owned()],
        files,
        lists,
        ..Default::default()
    };
    let src = MemSource::from_bundle(bundle).expect("a bundle");
    let started = std::time::Instant::now();
    for i in 0..n {
        let engines = format!("/m/lib/mod{i}/engines");
        assert_eq!(src.list(&engines), Err(SourceError::NotFound));
        assert_eq!(
            src.read(&format!("{engines}/e.k7x"), Span::All),
            Err(SourceError::NotFound)
        );
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
}
