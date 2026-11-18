// clips_pattern.rs — a strudel lane compiled for the engine: the plugin heading each half, the file's text and how lanes share it.
// A reader would get wrong that the file is read through the Source and capped: an unreadable pattern is "" and a warning, never a failure.

use crate::support;

use song::clips::{
    Half, MAX_FILE_LANES, MAX_LANES, MAX_PATTERN, PATTERN_PLUGIN_TYPE, Plays, pattern_plugins, pattern_shares,
    read_pattern, sample_banks,
};
use song::{DiskSource, Roots, Severity, SourceError};
use support::{Tmp, mem};

const SOURCE: &str = "s(\"bd sn\").label(\"kit\")\n";

fn share(lanes: &[(&str, &str)], lane: &str) -> song::clips::PatternShare {
    pattern_shares("/p", lanes, song::PathRule::HOST).0[lane].clone()
}

fn banks() -> Vec<String> {
    vec!["/p/samples".to_owned()]
}

#[test]
fn a_bare_wired_lane_is_headed_by_one_plugin_playing_both() {
    // enginePattern.test.ts "is headed by ONE plugin playing both when it is bare-wired"
    let halves = [Half { id: "kit", port: None }];
    let got = pattern_plugins(
        "kit",
        &halves,
        SOURCE,
        &share(&[("kit", "algo/beat.strudel")], "kit"),
        &banks(),
    );
    assert_eq!(got.len(), 1);
    let pl = &got["kit"];
    assert_eq!(pl.id, "kit:pattern");
    assert_eq!(pl.plugin_type, PATTERN_PLUGIN_TYPE);
    assert_eq!((pl.pattern_source.as_str(), pl.pattern_label.as_str()), (SOURCE, "kit"));
    assert_eq!(
        (pl.pattern_plays, pl.pattern_only, pl.pattern_catch_all),
        (Plays::Both, true, true)
    );
    assert_eq!(pl.pattern_claimed, ["kit"]);
    assert_eq!(pl.pattern_banks[0], "/p/samples");
    assert_eq!(
        (
            pl.name.as_str(),
            pl.path.as_str(),
            pl.preset_save.as_str(),
            pl.role.as_str(),
            pl.enabled
        ),
        ("Pattern", "", "", "", true)
    );
}

#[test]
fn two_halves_play_the_notes_on_midi_and_the_samples_on_audio() {
    // enginePattern.test.ts "plays its notes on #midi and its samples on #audio when the two are wired apart"
    let halves = [
        Half {
            id: "kit#audio",
            port: Some("audio"),
        },
        Half {
            id: "kit#midi",
            port: Some("midi"),
        },
    ];
    let got = pattern_plugins("kit", &halves, SOURCE, &share(&[("kit", "a.strudel")], "kit"), &banks());
    let plays: Vec<(&str, Plays, bool)> = got
        .iter()
        .map(|(k, p)| (k.as_str(), p.pattern_plays, p.pattern_only))
        .collect();
    assert_eq!(
        plays,
        [("kit#midi", Plays::Notes, false), ("kit#audio", Plays::Samples, false)]
    );
    // two plugins, two ids: the engine keys its plugins by id
    assert_eq!(got["kit#midi"].id, "kit#midi:pattern");
    assert_ne!(got["kit#midi"].id, got["kit#audio"].id);
}

#[test]
fn one_wired_half_plays_only_that_and_says_nothing_else_will() {
    // enginePattern.test.ts "with one half wired, plays only that and knows nothing else will"
    let halves = [Half {
        id: "kit#audio",
        port: Some("audio"),
    }];
    let got = pattern_plugins("kit", &halves, SOURCE, &share(&[("kit", "a.strudel")], "kit"), &banks());
    assert_eq!(got.len(), 1);
    assert_eq!(
        (got["kit#audio"].pattern_plays, got["kit#audio"].pattern_only),
        (Plays::Samples, true)
    );
}

#[test]
fn a_portless_half_stands_in_for_the_port_that_is_not_wired() {
    // `midi` else the portless half for notes, `audio` else the portless half for samples
    let halves = [
        Half {
            id: "kit#midi",
            port: Some("midi"),
        },
        Half { id: "kit", port: None },
    ];
    let got = pattern_plugins("kit", &halves, SOURCE, &share(&[("kit", "a.strudel")], "kit"), &banks());
    assert_eq!(got["kit#midi"].pattern_plays, Plays::Notes);
    assert_eq!(got["kit"].pattern_plays, Plays::Samples);
    assert!(pattern_plugins("kit", &[], SOURCE, &share(&[("kit", "a")], "kit"), &banks()).is_empty());
}

#[test]
fn the_first_lane_by_name_takes_what_no_lane_claims() {
    // enginePattern.test.ts "shares its file: the first lane by name takes what no lane claims"; decision 165
    let lanes = [
        ("pad", "algo/beat.strudel"),
        ("kit", "algo/beat.strudel"),
        ("solo", "algo/x/../other.strudel"),
    ];
    let (shares, _) = pattern_shares("/p", &lanes, song::PathRule::HOST);
    assert!(shares["kit"].catch_all);
    assert!(!shares["pad"].catch_all);
    assert_eq!(shares["pad"].claimed, ["kit", "pad"]);
    assert_eq!(
        (shares["solo"].claimed.as_slice(), shares["solo"].catch_all),
        (["solo".to_owned()].as_slice(), true)
    );
    // two spellings of one file are one file
    let (same, _) = pattern_shares(
        "/p",
        &[("a", "algo/a.strudel"), ("b", "algo/x/../a.strudel")],
        song::PathRule::HOST,
    );
    assert_eq!(same["a"].claimed, ["a", "b"]);
}

#[test]
fn lanes_are_ordered_by_code_point() {
    let (shares, _) = pattern_shares(
        "/p",
        &[
            ("b", "a.strudel"),
            ("B", "a.strudel"),
            ("\u{e9}", "a.strudel"),
            ("z", "a.strudel"),
        ],
        song::PathRule::HOST,
    );
    assert_eq!(shares["B"].claimed, ["B", "b", "z", "\u{e9}"]);
}

#[test]
fn the_banks_are_the_projects_own_then_the_hosts() {
    // decision 167
    let roots = Roots {
        project: "/p".into(),
        sample_banks: vec!["/dirt".into(), "/strudel".into()],
        ..Roots::default()
    };
    assert_eq!(sample_banks(&roots), ["/p/samples", "/dirt", "/strudel"]);
}

#[test]
fn a_pattern_is_read_whole_through_the_source() {
    let src = mem(&[("algo/beat.strudel", SOURCE)]);
    assert_eq!(
        read_pattern(&src, "/p", "algo/beat.strudel", song::PathRule::HOST),
        Ok((SOURCE.to_owned(), None))
    );
}

#[test]
fn a_pattern_that_is_missing_or_a_folder_is_nothing_and_says_nothing() {
    // decision 164: 2e names it
    let src = mem(&[("algo/x.strudel/inner.txt", "x")]);
    assert_eq!(
        read_pattern(&src, "/p", "algo/absent.strudel", song::PathRule::HOST),
        Ok((String::new(), None))
    );
    assert_eq!(
        read_pattern(&src, "/p", "algo/x.strudel", song::PathRule::HOST),
        Ok((String::new(), None))
    );
}

#[test]
fn a_pattern_the_source_refuses_is_nothing_and_a_warning_named_by_its_path() {
    // decision 164: outside the mounts, with a lexical climb and a NUL
    let src = mem(&[("algo/beat.strudel", SOURCE)]);
    for bad in ["algo/../../x.strudel", "/etc/hosts", "algo/a\0b.strudel"] {
        let (text, warning) = read_pattern(&src, "/p", bad, song::PathRule::HOST).expect("answers");
        let w = warning.unwrap_or_else(|| panic!("{bad:?} gave no warning"));
        assert_eq!(
            (text.as_str(), w.file.as_str(), w.severity),
            ("", bad, Severity::Warning)
        );
    }
}

#[test]
fn a_pattern_past_the_cap_is_not_sent_and_is_never_read() {
    // decision 164: stat first, so an oversize file costs no read
    let big = "x".repeat(usize::try_from(MAX_PATTERN).expect("size") + 1);
    let src = mem(&[("algo/big.strudel", big.as_str())]);
    let spy = support::Spy {
        inner: &src,
        reads: std::sync::Mutex::new(Vec::new()),
    };
    let (text, warning) = read_pattern(&spy, "/p", "algo/big.strudel", song::PathRule::HOST).expect("answers");
    assert_eq!(
        (text.as_str(), warning.map(|w| w.severity)),
        ("", Some(Severity::Warning))
    );
    assert!(spy.reads.lock().expect("lock").is_empty());
}

#[test]
fn a_pattern_that_is_not_utf8_is_nothing_and_a_warning() {
    // decision 164, on disk: a bundle cannot hold bytes that are not text
    let t = Tmp::new("pattern-utf8");
    std::fs::create_dir_all(t.0.join("ws/proj/algo")).expect("folder");
    std::fs::write(t.0.join("ws/proj/algo/bad.strudel"), [0xff, 0xfe, 0x00, 0x41]).expect("file");
    let project = t.at("ws/proj");
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
    let (text, warning) = read_pattern(&disk, &project, "algo/bad.strudel", song::PathRule::HOST).expect("answers");
    assert_eq!(
        (text.as_str(), warning.map(|w| w.severity)),
        ("", Some(Severity::Warning))
    );
}

#[test]
fn a_bundle_that_cannot_answer_is_missing() {
    // decision 164: `Missing` is an Err, so the compile answers needs_files
    let bare = song::SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: std::collections::BTreeMap::new(),
        lists: std::collections::BTreeMap::new(),
        ..Default::default()
    };
    let src = song::MemSource::from_bundle(bare).expect("a source");
    assert_eq!(
        read_pattern(&src, "/p", "algo/beat.strudel", song::PathRule::HOST),
        Err(SourceError::Missing)
    );
}

#[test]
fn lanes_past_a_files_limit_or_the_projects_play_nothing_with_one_error_per_file() {
    // decision 178: 3,000 lanes on one file were 9,000,000 claimed names
    let names: Vec<String> = (0..3000).map(|i| format!("l{i:04}")).collect();
    let lanes: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "algo/a.strudel")).collect();
    let (shares, findings) = pattern_shares("/p", &lanes, song::PathRule::HOST);
    assert_eq!(shares.len(), MAX_FILE_LANES);
    assert!(shares.values().all(|s| s.claimed.len() == MAX_FILE_LANES));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "algo/a.strudel");
    // 200 files of one lane stop at the project's limit, and each file left out is named
    let files: Vec<String> = (0..200).map(|i| format!("algo/f{i}.strudel")).collect();
    let many: Vec<(&str, &str)> = names
        .iter()
        .zip(&files)
        .map(|(n, f)| (n.as_str(), f.as_str()))
        .collect();
    let (shares, findings) = pattern_shares("/p", &many, song::PathRule::HOST);
    assert_eq!((shares.len(), findings.len()), (MAX_LANES, 72));
}
