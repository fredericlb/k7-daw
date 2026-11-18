// bundle_shape.rs — the bundle's JSON: every field round-trips, an unknown key is refused, an old bundle reads (decision 742).
// A reader would get wrong that the new fields are camelCase while `Roots` keeps its snake_case keys.

use serde_json::json;
use song::{BundleFile, BundleRequest, Entry, Kind, MemSource, PathRule, Roots, SourceBundle, SourceError};

fn full() -> SourceBundle {
    let mut b = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        ..SourceBundle::default()
    };
    b.files.insert(
        "/p/a.k7x".into(),
        BundleFile {
            text: Some("x(1)\n".into()),
            ..BundleFile::default()
        },
    );
    b.files.insert(
        "/p/b.wav".into(),
        BundleFile {
            len: Some(9),
            binary: Some(true),
            ..BundleFile::default()
        },
    );
    b.files.insert(
        "/p/c.k7x".into(),
        BundleFile {
            len: Some(3),
            io: Some("denied".into()),
            ..BundleFile::default()
        },
    );
    b.lists.insert(
        "/p".into(),
        vec![Entry {
            name: "a.k7x".into(),
            kind: Kind::File,
        }],
    );
    b.refused.insert("/p/link".into());
    b.failed_stats
        .insert("/p/a.wav/sub.wav".into(), "not a directory".into());
    b.failed_lists.insert("/p/closed".into(), "denied".into());
    b.absent.insert("/p/closed/gone.k7x".into());
    b.digests.insert("/p/b.wav".into(), "ab".repeat(32));
    b.jsdsp_stale.insert("/p/m/jsdsp.ts".into());
    b.lengths.insert("/p/b.wav".into(), 1.5);
    b.request = Some(BundleRequest {
        roots: Roots {
            project: "/p".into(),
            also_readable: vec!["/q".into()],
            paths: PathRule::Windows,
            ..Roots::default()
        },
        rig: json!({"channels": []}),
        vsts: "scan".into(),
        thawed: vec!["a".into()],
    });
    b
}

#[test]
fn a_bundle_with_every_field_round_trips_in_camel_case() {
    let b = full();
    let text = serde_json::to_string(&b).expect("json");
    let v: serde_json::Value = serde_json::from_str(&text).expect("value");
    for key in [
        "refused",
        "failedStats",
        "failedLists",
        "absent",
        "digests",
        "jsdspStale",
        "lengths",
        "request",
    ] {
        assert!(v.get(key).is_some(), "{key} is a camelCase key");
    }
    assert_eq!(v["request"]["roots"]["also_readable"], json!(["/q"]));
    assert_eq!(v["request"]["roots"]["paths"], json!("windows"));
    assert_eq!(serde_json::from_str::<SourceBundle>(&text).expect("back"), b);
    let src = MemSource::from_bundle(b).expect("a source");
    // Decision 794: the digests, the stale jsdsp sources and the lengths are read back, normalized.
    assert_eq!(src.digests().get("/p/b.wav"), Some(&"ab".repeat(32)));
    assert!(src.jsdsp_stale().contains("/p/m/jsdsp.ts"));
    assert_eq!(src.lengths().get("/p/b.wav"), Some(&1.5));
    assert_eq!(src.request().map(|r| r.vsts.as_str()), Some("scan"));
}

// Decision 796: the bundle carries the sentence of a Recorder that outgrew a limit, and a replay refuses it.
#[test]
fn a_bundle_that_says_it_was_cut_short_round_trips_and_is_refused() {
    let mut b = full();
    b.overflow = Some("the bundle would hold more than 1 bytes".into());
    let v = serde_json::to_value(&b).expect("json");
    assert_eq!(v["overflow"], json!("the bundle would hold more than 1 bytes"));
    assert_eq!(serde_json::from_value::<SourceBundle>(v).expect("back"), b);
    assert!(matches!(MemSource::from_bundle(b), Err(SourceError::Io(m)) if m.contains("cut short")));
}

#[test]
fn a_bundle_with_none_of_the_new_fields_reads_and_writes_none() {
    let json = r#"{"format":1,"mounts":["/p"],"files":{"/p/a":{"text":"x"},"/p/b":{"len":2}},"lists":{}}"#;
    let b: SourceBundle = serde_json::from_str(json).expect("an old bundle");
    assert!(b.refused.is_empty() && b.request.is_none() && b.lengths.is_empty());
    let back = serde_json::to_value(&b).expect("json");
    for key in [
        "refused",
        "failedStats",
        "failedLists",
        "absent",
        "overflow",
        "digests",
        "jsdspStale",
        "lengths",
        "request",
    ] {
        assert!(back.get(key).is_none(), "{key} is skipped when empty");
    }
    assert!(MemSource::from_bundle(b).is_ok());
}

#[test]
fn an_unknown_key_is_refused_at_every_level() {
    for json in [
        r#"{"format":1,"mounts":[],"files":{},"lists":{},"extra":1}"#,
        r#"{"format":1,"mounts":[],"files":{"/p/a":{"len":1,"extra":1}},"lists":{}}"#,
        r#"{"format":1,"mounts":[],"files":{},"lists":{},"failed_stats":{}}"#,
        r#"{"format":1,"mounts":[],"files":{},"lists":{},"request":{"roots":{"project":"/p"},"rig":{},"vsts":"none","thawed":[],"x":1}}"#,
    ] {
        assert!(serde_json::from_str::<SourceBundle>(json).is_err(), "{json}");
    }
}

#[test]
fn the_path_rule_is_a_lowercase_word_that_defaults_to_the_host() {
    assert_eq!(serde_json::to_string(&PathRule::Posix).expect("json"), "\"posix\"");
    assert_eq!(
        serde_json::from_str::<PathRule>("\"windows\"").expect("rule"),
        PathRule::Windows
    );
    assert!(serde_json::from_str::<PathRule>("\"mac\"").is_err());
    assert_eq!(PathRule::default(), PathRule::HOST);
    assert_eq!(PathRule::HOST.is_windows(), cfg!(windows));
    let r: Roots = serde_json::from_str(r#"{"project":"/p"}"#).expect("roots");
    assert_eq!(r.paths, PathRule::HOST);
    assert!(r.also_readable.is_empty());
}

#[test]
fn a_file_that_names_two_things_is_refused_when_the_source_is_made() {
    let one = |f: BundleFile| {
        let mut b = SourceBundle {
            format: 1,
            mounts: vec!["/p".into()],
            ..SourceBundle::default()
        };
        b.files.insert("/p/a".into(), f);
        MemSource::from_bundle(b).map(|_| ())
    };
    let ok = BundleFile {
        len: Some(4),
        ..BundleFile::default()
    };
    assert_eq!(one(ok.clone()), Ok(()));
    let text = |t: &str| BundleFile {
        text: Some(t.into()),
        ..BundleFile::default()
    };
    for bad in [
        BundleFile {
            len: Some(1),
            ..text("x")
        },
        BundleFile {
            io: Some("e".into()),
            ..text("x")
        },
        BundleFile {
            binary: Some(true),
            ..text("x")
        },
        BundleFile {
            binary: Some(true),
            io: Some("e".into()),
            ..ok.clone()
        },
        BundleFile {
            binary: Some(true),
            len: Some(0),
            ..BundleFile::default()
        },
        BundleFile {
            binary: Some(true),
            ..BundleFile::default()
        },
        BundleFile {
            io: Some("e".into()),
            ..BundleFile::default()
        },
        BundleFile::default(),
    ] {
        assert!(matches!(one(bad.clone()), Err(SourceError::Io(_))), "{bad:?}");
    }
}

#[test]
fn a_key_that_does_not_normalize_or_a_vsts_that_is_no_mode_refuses_the_bundle() {
    let base = || SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        ..SourceBundle::default()
    };
    let mut b = base();
    b.failed_stats.insert("relative/path".into(), "x".into());
    assert!(MemSource::from_bundle(b).is_err());
    let mut b = base();
    b.failed_lists.insert("/p/\0".into(), "x".into());
    assert!(MemSource::from_bundle(b).is_err());
    let mut b = base();
    b.refused.insert("nope".into());
    assert!(MemSource::from_bundle(b).is_err());
    let mut b = base();
    b.lengths.insert("nope".into(), 1.0);
    assert!(MemSource::from_bundle(b).is_err());
    let mut b = base();
    b.request = Some(BundleRequest {
        roots: Roots::default(),
        rig: json!({}),
        vsts: "all".into(),
        thawed: vec![],
    });
    assert!(MemSource::from_bundle(b).is_err());
}
