// freeze_fingerprint.rs — the name of a render: the canonical hash of a cut, every file it names spelled portably (decisions 343 to 345, 348).
// A reader would get wrong that a token is checked by comparing two fingerprints: a string replaced by its token hashes as the token typed.

use crate::freeze_support;
use crate::support;

use freeze_support::{Counting, Nothing, Synthetic, doc, fast, hex16, naming, track};
use song::clips::Budget;
use song::doc::ProjectDoc;
use song::freeze::{BUDGET, MAX_DIGEST_BYTES, freeze_fingerprint};
use song::record::Recorder;
use song::{BundleFile, Cache, DiskSource, MemSource, Source, SourceBundle, SourceError};
use std::collections::BTreeMap;
use support::{Tmp, spell};

fn fp(source: &dyn Source, cut: &ProjectDoc, project: &str) -> String {
    let mut cache = Cache::new();
    freeze_fingerprint(
        source,
        &mut cache,
        cut,
        project,
        &mut Budget::new(BUDGET),
        song::PathRule::HOST,
    )
    .expect("a source")
    .expect("within the budget")
}

/// A disk Source over the whole temporary folder, which holds the project and everything it names.
fn world(t: &Tmp) -> DiskSource {
    DiskSource::new(&[spell(&t.0)]).expect("a source")
}

#[test]
fn a_fingerprint_is_sixteen_hex_digits_and_stable() {
    // decision 343
    let t = Tmp::new("fp-stable");
    let a = fp(&world(&t), &doc(vec![track("gtr", &["amp"])]), &t.at("proj"));
    assert_eq!(a.len(), 16);
    assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(a, fp(&world(&t), &doc(vec![track("gtr", &["amp"])]), &t.at("proj")));
}

#[test]
fn anything_the_engine_hears_changes_the_name() {
    // decision 343: tempo, sample rate, signature, a param, a track
    let t = Tmp::new("fp-sound");
    let src = world(&t);
    let base = fp(&src, &doc(vec![track("gtr", &["amp"])]), &t.at("proj"));
    let variants: Vec<ProjectDoc> = {
        let mut v = Vec::new();
        let mut d = doc(vec![track("gtr", &["amp"])]);
        d.tempo = 121.0;
        v.push(d);
        let mut d = doc(vec![track("gtr", &["amp"])]);
        d.sample_rate = 44100;
        v.push(d);
        let mut d = doc(vec![track("gtr", &["amp"])]);
        d.time_sig_numerator = 3;
        v.push(d);
        let mut d = doc(vec![track("gtr", &["amp"])]);
        d.tracks[0].plugins[0].params.insert("Gain".into(), 2.0);
        v.push(d);
        v.push(doc(vec![track("gtr", &["amp"]), track("keys", &[])]));
        v
    };
    let mut names: Vec<String> = variants.iter().map(|d| fp(&src, d, &t.at("proj"))).collect();
    names.push(base);
    names.sort();
    names.dedup();
    assert_eq!(names.len(), variants.len() + 1);
}

#[test]
fn what_says_nothing_about_the_sound_is_left_out() {
    // decision 343: presetSave, event, color, recordDir are NOT_SOUND at every level
    let t = Tmp::new("fp-notsound");
    let src = world(&t);
    let base = fp(&src, &doc(vec![track("gtr", &["amp"])]), &t.at("proj"));
    let mut d = doc(vec![track("gtr", &["amp"])]);
    d.tracks[0].color = "#ff0000".into();
    d.tracks[0].plugins[0].preset_save = "/somewhere/else.vstpreset".into();
    d.record_dir = "/clips".into();
    assert_eq!(fp(&src, &d, &t.at("proj")), base);
}

#[test]
fn a_file_in_the_project_enters_by_its_path_there_and_its_content() {
    // decision 344, autoFreeze.test.ts "names the render the same wherever the project lives"
    let (a, b) = (Tmp::new("fp-here"), Tmp::new("fp-there"));
    for t in [&a, &b] {
        t.write("proj/nodes/amp.vstpreset", "state-1");
    }
    let one = fp(&world(&a), &naming(&a.at("proj/nodes/amp.vstpreset")), &a.at("proj"));
    let two = fp(&world(&b), &naming(&b.at("proj/nodes/amp.vstpreset")), &b.at("proj"));
    assert_eq!(one, two);
    let typed = fp(
        &world(&a),
        &naming(&format!("nodes/amp.vstpreset#{}", hex16(b"state-1"))),
        &a.at("proj"),
    );
    assert_eq!(one, typed);
}

#[test]
fn a_preset_rewritten_in_place_is_another_name() {
    // autoFreeze.test.ts "plays live again once the VST's preset file is rewritten"; decision 345 (an unsettled stamp is hashed each time)
    let t = Tmp::new("fp-rewrite");
    let path = t.at("proj/nodes/amp.vstpreset");
    t.write("proj/nodes/amp.vstpreset", "state-1");
    let mut cache = Cache::new();
    let src = world(&t);
    let name = |c: &mut Cache| {
        freeze_fingerprint(
            &src,
            c,
            &naming(&path),
            &t.at("proj"),
            &mut Budget::new(BUDGET),
            song::PathRule::HOST,
        )
        .expect("a source")
        .expect("budget")
    };
    let before = name(&mut cache);
    t.write("proj/nodes/amp.vstpreset", "state-2 (longer)");
    assert_ne!(name(&mut cache), before);
}

#[test]
fn a_file_outside_the_project_enters_by_its_base_name() {
    // decision 344: `~/<base>`, so two machines holding the same library file agree
    let t = Tmp::new("fp-outside");
    t.write("lib/amp.vstpreset", "state");
    t.write("elsewhere/deep/amp.vstpreset", "state");
    let src = world(&t);
    let one = fp(&src, &naming(&t.at("lib/amp.vstpreset")), &t.at("proj"));
    let two = fp(&src, &naming(&t.at("elsewhere/deep/amp.vstpreset")), &t.at("proj"));
    assert_eq!(one, two);
    let typed = fp(
        &src,
        &naming(&format!("~/amp.vstpreset#{}", hex16(b"state"))),
        &t.at("proj"),
    );
    assert_eq!(one, typed);
}

#[test]
fn a_file_above_a_mebibyte_enters_by_its_length() {
    // decision 344: a render of a long sample is not read
    let t = Tmp::new("fp-long");
    let big = 1024 * 1024 + 1;
    t.write("proj/a.wav", &"a".repeat(big));
    let disk = world(&t);
    let src = Counting::new(&disk);
    let name = fp(&src, &naming(&t.at("proj/a.wav")), &t.at("proj"));
    assert_eq!(src.reads(), 0);
    assert_eq!(name, fp(&src, &naming(&format!("a.wav#{big}")), &t.at("proj")));
}

#[test]
fn a_folder_enters_as_a_folder_where_the_typescript_threw() {
    // decision 344, DEPARTS: `<name>#dir`
    let t = Tmp::new("fp-dir");
    t.mkdir("proj/samples");
    let src = world(&t);
    let name = fp(&src, &naming(&t.at("proj/samples")), &t.at("proj"));
    assert_eq!(name, fp(&src, &naming("samples#dir"), &t.at("proj")));
}

/// The fingerprint of a cut naming `name` under the folder `dir` of `t`, with `dir` as the project.
fn named_in(src: &dyn Source, t: &Tmp, dir: &str, name: &str) -> String {
    fp(src, &naming(&format!("{}/{name}", t.at(dir))), &t.at(dir))
}

#[test]
fn an_absent_file_is_left_as_written() {
    // decision 344: a Source refusal must not reveal a path into a name
    let t = Tmp::new("fp-absent");
    let disk = world(&t);
    let src = Counting::new(&disk);
    let one = named_in(&src, &t, "proj", "nothing.wav");
    assert_eq!((src.stats(), src.reads()), (1, 0));
    // Tokenized, the same file in two projects would be `nothing.wav` in both; as written, the paths differ.
    assert_ne!(one, named_in(&src, &t, "proj2", "nothing.wav"));
    let project = t.at("proj");
    assert_ne!(one, fp(&src, &naming("nothing.wav#missing"), &project));
    assert_ne!(one, fp(&src, &naming("nothing.wav"), &project));
}

#[test]
fn a_path_outside_every_mount_is_left_as_written_and_never_read() {
    // decision 344
    let t = Tmp::new("fp-refused");
    let other = Tmp::new("fp-refused-other");
    other.write("secret.wav", "x");
    let third = Tmp::new("fp-refused-third");
    third.write("secret.wav", "x");
    let disk = world(&t);
    let src = Counting::new(&disk);
    let project = t.at("proj");
    let one = fp(&src, &naming(&other.at("secret.wav")), &project);
    assert_eq!(src.reads(), 0);
    // A token would be `~/secret.wav#<digest>` for both files; as written they differ.
    assert_ne!(one, fp(&src, &naming(&third.at("secret.wav")), &project));
    assert_ne!(
        one,
        fp(&src, &naming(&format!("~/secret.wav#{}", hex16(b"x"))), &project)
    );
    assert_ne!(one, fp(&src, &naming("~/secret.wav#missing"), &project));
}

#[test]
fn a_hundred_thousand_occurrences_of_one_path_are_one_stat() {
    // freeze_fingerprint hostile row; decision 344
    let t = Tmp::new("fp-memo");
    t.write("proj/a.vstpreset", "x");
    let path = t.at("proj/a.vstpreset");
    let mut d = doc(vec![track("gtr", &["amp"])]);
    d.tracks[0].plugins[0].faust_files = Some((0..100_000).map(|i| (format!("f{i}"), path.clone())).collect());
    let disk = world(&t);
    let src = Counting::new(&disk);
    let mut cache = Cache::new();
    fast("a hundred thousand occurrences", || {
        freeze_fingerprint(
            &src,
            &mut cache,
            &d,
            &t.at("proj"),
            &mut Budget::new(BUDGET),
            song::PathRule::HOST,
        )
    })
    .expect("a source")
    .expect("budget");
    assert_eq!(src.stats(), 1);
    assert_eq!(src.reads(), 1);
}

#[test]
fn a_hundred_thousand_distinct_absent_paths_are_a_stat_each_within_the_budget() {
    // freeze_fingerprint hostile row; decision 344, 348: a stat costs 4, so 100,000 of them are far under 3,000,000
    let mut d = doc(vec![track("gtr", &["amp"])]);
    d.tracks[0].plugins[0].faust_files = Some(
        (0..100_000)
            .map(|i| (format!("f{i}"), format!("/p/n{i}.wav")))
            .collect(),
    );
    let src = Nothing::new();
    let mut budget = Budget::new(BUDGET);
    let mut cache = Cache::new();
    let out = fast("distinct absent paths", || {
        freeze_fingerprint(&src, &mut cache, &d, "/p", &mut budget, song::PathRule::HOST)
    })
    .expect("a source");
    assert!(out.is_some());
    assert_eq!(src.stats.load(std::sync::atomic::Ordering::SeqCst), 100_000);
    assert!(budget.spent() >= 400_000 && !budget.capped());
}

#[test]
fn a_budget_spent_by_the_stats_answers_none() {
    // decision 348
    let t = Tmp::new("fp-spent");
    let mut d = doc(vec![track("gtr", &["amp"])]);
    let base = t.at("proj");
    d.tracks[0].plugins[0].faust_files = Some(
        (0..2_000)
            .map(|i| (format!("f{i}"), format!("{base}/n{i}.wav")))
            .collect(),
    );
    let mut cache = Cache::new();
    let out = freeze_fingerprint(
        &world(&t),
        &mut cache,
        &d,
        &base,
        &mut Budget::new(1_000),
        song::PathRule::HOST,
    )
    .expect("a source");
    assert_eq!(out, None);
}

#[test]
fn a_string_of_five_mebibytes_starting_with_a_slash_is_never_a_path() {
    // freeze_fingerprint hostile row; decision 344: the text of a Faust module starting `// …` looks absolute
    let t = Tmp::new("fp-faust");
    let mut d = doc(vec![track("gtr", &["amp"])]);
    d.tracks[0].plugins[0].faust_dsp = Some(format!("/{}", "x".repeat(5 * 1024 * 1024)));
    let disk = world(&t);
    let src = Counting::new(&disk);
    let mut cache = Cache::new();
    fast("a long string", || {
        freeze_fingerprint(
            &src,
            &mut cache,
            &d,
            &t.at("proj"),
            &mut Budget::new(BUDGET),
            song::PathRule::HOST,
        )
    })
    .expect("a source")
    .expect("budget");
    assert_eq!(src.stats(), 0);
}

#[test]
fn a_string_of_4096_bytes_is_a_path_and_one_byte_more_is_not() {
    // decision 344: the limit
    let t = Tmp::new("fp-limit");
    let disk = world(&t);
    let src = Counting::new(&disk);
    let at = |n: usize| format!("{}/{}", t.at("proj"), "a".repeat(n - t.at("proj").len() - 1));
    let _ = fp(&src, &naming(&at(4096)), &t.at("proj"));
    assert_eq!(src.stats(), 1);
    let _ = fp(&src, &naming(&at(4097)), &t.at("proj"));
    assert_eq!(src.stats(), 1);
}

#[test]
fn a_backslash_and_a_nul_in_a_path_are_left_as_written() {
    // freeze_fingerprint hostile row; decision 344
    let t = Tmp::new("fp-hazard");
    let disk = world(&t);
    let src = Counting::new(&disk);
    let nul = named_in(&src, &t, "proj", "a\0b");
    assert_eq!(src.stats(), 0, "a NUL never reaches the Source");
    assert_ne!(nul, named_in(&src, &t, "proj2", "a\0b"));
    assert_ne!(nul, fp(&src, &naming("a\0b"), &t.at("proj")));
    // An absent name with a backslash: as written, so two projects give two names, where a token `a\\b.wav` would agree.
    let back = named_in(&src, &t, "proj", "a\\b.wav");
    assert_ne!(back, named_in(&src, &t, "proj2", "a\\b.wav"));
    assert_ne!(back, fp(&src, &naming("a\\b.wav#missing"), &t.at("proj")));
}

fn many(n: usize) -> ProjectDoc {
    let mut d = doc(vec![track("gtr", &["amp"])]);
    d.tracks[0].plugins[0].faust_files = Some((0..n).map(|i| (format!("f{i}"), format!("/s/{i}.bin"))).collect());
    d
}

#[test]
fn the_budget_binds_the_hashing_before_the_cap_does() {
    // freeze_fingerprint hostile row; decisions 344, 348: a MiB costs 4 + 32 + 1,024 of the budget, a sixteenth of it here
    let src = Synthetic::new(1024 * 1024);
    let mut cache = Cache::new();
    let mut budget = Budget::new(BUDGET / 16);
    let out = fast("three hundred files", || {
        freeze_fingerprint(&src, &mut cache, &many(300), "/p", &mut budget, song::PathRule::HOST)
    })
    .expect("a source");
    assert_eq!(out, None);
    // 187,500 / 1,060 is 176.9: 176 files fit, and the 177th is charged, refused and never read.
    let reads = src.reads.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(reads, 176, "{reads}");
}

#[test]
fn a_tiny_real_file_is_charged_as_a_stat_and_a_read() {
    // decision 348: a 10-byte file costs a stat and a read (4 + 32), not a rounding error
    let t = Tmp::new("fp-tiny");
    let mut d = doc(vec![track("gtr", &["amp"])]);
    let base = t.at("proj");
    for i in 0..300 {
        t.write(&format!("proj/f{i}.bin"), "0123456789");
    }
    d.tracks[0].plugins[0].faust_files = Some(
        (0..300)
            .map(|i| (format!("f{i}"), format!("{base}/f{i}.bin")))
            .collect(),
    );
    let mut budget = Budget::new(BUDGET);
    let mut cache = Cache::new();
    freeze_fingerprint(&world(&t), &mut cache, &d, &base, &mut budget, song::PathRule::HOST)
        .expect("a source")
        .expect("budget");
    assert!(budget.spent() >= 300 * 36, "{}", budget.spent());
}

#[test]
#[ignore = "measures 100,000 real files of 10 bytes: seconds of system calls, printed and not bounded"]
fn a_hundred_thousand_tiny_real_files_measured() {
    // decision 348: the capacity that READ_COST buys, printed by `cargo test -- --ignored --nocapture`
    let t = Tmp::new("fp-tiny-measure");
    let base = t.at("proj");
    for i in 0..100_000 {
        t.write(&format!("proj/f{i}.bin"), "0123456789");
    }
    let mut d = doc(vec![track("gtr", &["amp"])]);
    d.tracks[0].plugins[0].faust_files = Some(
        (0..100_000)
            .map(|i| (format!("f{i}"), format!("{base}/f{i}.bin")))
            .collect(),
    );
    let mut budget = Budget::new(BUDGET);
    let mut cache = Cache::new();
    let started = std::time::Instant::now();
    let out =
        freeze_fingerprint(&world(&t), &mut cache, &d, &base, &mut budget, song::PathRule::HOST).expect("a source");
    eprintln!(
        "100,000 x 10 bytes: {:?}, spent {}, capped {}, answer {}",
        started.elapsed(),
        budget.spent(),
        budget.capped(),
        out.is_some()
    );
}

#[test]
fn three_hundred_files_at_the_real_budget_are_hashed_up_to_the_cap() {
    // freeze_fingerprint hostile row; decision 348: 300 x 1 MiB cost 318,000, so the 256 MiB cap binds first
    let src = Synthetic::new(1024 * 1024);
    let mut cache = Cache::new();
    let mut budget = Budget::new(BUDGET);
    let out = fast("three hundred files at the real budget", || {
        freeze_fingerprint(&src, &mut cache, &many(300), "/p", &mut budget, song::PathRule::HOST)
    })
    .expect("a source");
    assert!(out.is_some() && !budget.capped());
    assert_eq!(src.reads.load(std::sync::atomic::Ordering::SeqCst), 256);
}

#[test]
fn the_cap_on_hashed_bytes_is_two_hundred_fifty_six_mebibytes() {
    // decision 348
    assert_eq!(MAX_DIGEST_BYTES, 256 * 1024 * 1024);
}

#[test]
fn a_file_that_vanishes_between_the_stat_and_the_read_enters_as_missing() {
    // freeze_fingerprint hostile row; decision 344
    let mut src = Synthetic::new(10);
    src.vanish = true;
    let name = fp(&src, &naming("/s/a.bin"), "/s");
    assert_eq!(name, fp(&src, &naming("a.bin#missing"), "/s"));
}

fn unknown_mount() -> MemSource {
    let mut lists = BTreeMap::new();
    lists.insert(
        "/p".to_owned(),
        vec![song::Entry {
            name: "deep".into(),
            kind: song::Kind::Dir,
        }],
    );
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: BTreeMap::<String, BundleFile>::new(),
        lists,
        ..Default::default()
    };
    MemSource::from_bundle(bundle).expect("a bundle")
}

#[test]
fn a_stat_the_bundle_cannot_answer_is_the_error() {
    // decision 344: Missing on the stat is Err
    let mut cache = Cache::new();
    let out = freeze_fingerprint(
        &unknown_mount(),
        &mut cache,
        &naming("/p/deep/x.wav"),
        "/p",
        &mut Budget::new(BUDGET),
        song::PathRule::HOST,
    );
    assert_eq!(out, Err(SourceError::Missing));
}

#[test]
fn a_file_the_bundle_holds_by_length_alone_enters_by_its_length() {
    // decision 344: a bundle can stat a binary and cannot read it; the token falls back to `<name>#<len>`
    let mut files = BTreeMap::new();
    files.insert(
        "/p/a.wav".to_owned(),
        BundleFile {
            text: None,
            len: Some(777),
            ..Default::default()
        },
    );
    let mut lists = BTreeMap::new();
    lists.insert(
        "/p".to_owned(),
        vec![song::Entry {
            name: "a.wav".into(),
            kind: song::Kind::File,
        }],
    );
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files,
        lists,
        ..Default::default()
    };
    let src = MemSource::from_bundle(bundle).expect("a bundle");
    let name = fp(&src, &naming("/p/a.wav"), "/p");
    assert_eq!(name, fp(&src, &naming("a.wav#777"), "/p"));
}

#[test]
fn a_digest_is_kept_while_its_stamp_is_settled_and_forgotten_by_clear() {
    // decision 345
    let t = Tmp::new("fp-cache");
    t.write("proj/a.vstpreset", "state");
    t.age();
    let disk = world(&t);
    let src = Counting::new(&disk);
    let mut cache = Cache::new();
    let d = naming(&t.at("proj/a.vstpreset"));
    let run = |cache: &mut Cache| {
        freeze_fingerprint(
            &src,
            cache,
            &d,
            &t.at("proj"),
            &mut Budget::new(BUDGET),
            song::PathRule::HOST,
        )
        .expect("a source")
        .expect("budget")
    };
    let first = run(&mut cache);
    assert_eq!(src.reads(), 1);
    assert_eq!(run(&mut cache), first);
    assert_eq!(src.reads(), 1, "a settled stamp is a hit");
    cache.clear();
    assert_eq!(run(&mut cache), first);
    assert_eq!(src.reads(), 2);
}

#[test]
fn seventy_binary_digests_do_not_fill_the_cache_and_are_not_read_again() {
    // decision 345: only a kept text weighs in `bytes`, a binary keeps its 16 hex; 70 MiB of them passed the 64 MiB limit
    let t = Tmp::new("fp-binary");
    let mut d = doc(vec![track("gtr", &["amp"])]);
    let base = t.at("proj");
    for i in 0..70 {
        std::fs::create_dir_all(t.0.join("proj")).expect("dir");
        std::fs::write(t.0.join(format!("proj/b{i}.bin")), vec![0xff_u8; 1024 * 1024]).expect("write");
    }
    d.tracks[0].plugins[0].faust_files = Some((0..70).map(|i| (format!("f{i}"), format!("{base}/b{i}.bin"))).collect());
    t.age();
    let disk = world(&t);
    let src = Counting::new(&disk);
    let mut cache = Cache::new();
    let run = |cache: &mut Cache| {
        freeze_fingerprint(&src, cache, &d, &base, &mut Budget::new(BUDGET), song::PathRule::HOST)
            .expect("a source")
            .expect("budget")
    };
    let first = run(&mut cache);
    assert_eq!(src.reads(), 70);
    assert_eq!(run(&mut cache), first);
    assert_eq!(src.reads(), 70, "a held digest is a hit, the cache was not cleared");
}

#[test]
fn an_unsettled_stamp_is_hashed_every_time() {
    // decision 345
    let t = Tmp::new("fp-unsettled");
    t.write("proj/a.vstpreset", "state");
    let disk = world(&t);
    let src = Counting::new(&disk);
    let mut cache = Cache::new();
    let d = naming(&t.at("proj/a.vstpreset"));
    for _ in 0..3 {
        freeze_fingerprint(
            &src,
            &mut cache,
            &d,
            &t.at("proj"),
            &mut Budget::new(BUDGET),
            song::PathRule::HOST,
        )
        .expect("a source")
        .expect("budget");
    }
    assert_eq!(src.reads(), 3);
}

#[test]
fn a_hit_replays_the_text_to_a_recorder_so_a_bundle_made_after_it_holds_it() {
    // decision 345 (and 27): a Recorder wrapped after a hit still gets the file
    let t = Tmp::new("fp-replay");
    t.write("proj/a.vstpreset", "state-text");
    t.age();
    let disk = world(&t);
    let mut cache = Cache::new();
    let d = naming(&t.at("proj/a.vstpreset"));
    let go = |s: &dyn Source, c: &mut Cache| {
        freeze_fingerprint(s, c, &d, &t.at("proj"), &mut Budget::new(BUDGET), song::PathRule::HOST)
            .expect("a source")
            .expect("budget")
    };
    go(&disk, &mut cache);
    let rec = Recorder::new(&disk);
    go(&rec, &mut cache);
    let bundle = rec.into_bundle(&[spell(&t.0)]);
    let held = bundle.files.get(&t.at("proj/a.vstpreset")).expect("recorded");
    assert_eq!(held.text.as_deref(), Some("state-text"));
}

#[test]
fn a_cut_heavier_than_the_budget_answers_none() {
    // decision 348: a canonical value walked per clip and per note is charged before it is walked
    let t = Tmp::new("fp-heavy");
    let d = doc((0..50).map(|i| track(&format!("t{i}"), &["amp"])).collect());
    let mut cache = Cache::new();
    let out = freeze_fingerprint(
        &world(&t),
        &mut cache,
        &d,
        &t.at("proj"),
        &mut Budget::new(50),
        song::PathRule::HOST,
    )
    .expect("a source");
    assert_eq!(out, None);
}
