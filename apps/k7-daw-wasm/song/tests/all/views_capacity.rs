// views_capacity.rs — the room a real-size project has: no limit is reached, and the spend and the time are printed.
// A reader would get wrong that a limit is a number to pass: each view's budget has to let this much through untouched.

use crate::support;
use crate::views_support;
use crate::views_world;

use song::clips::ClipDoc;
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::views::BUDGET;
use song::views::catalogue::{base_modules, module_catalogue};
use song::views::hints::hints_of;
use song::views::io_ports::io_ports;
use song::views::lengths::{ByHeader, ByTable, Lengths};
use song::views::web::{fold_nests, web_clip};
use song::{Cache, DiskSource};
use std::time::Instant;
use support::{Spy, Tmp};
use views_support::{bound, fast, wav};

const PLUGIN: &str = "plugin({ type: \"internal\", name: \"reverb\" })\n";
const PANEL: &str = "plugin({ type: \"internal\", name: \"eq\" })\nui({ cols: 4, rows: 2, widgets: {} })\n";

#[test]
fn a_library_of_five_thousand_modules_and_a_big_project_hit_no_limit() {
    // capacity table: 5,000 modules (200 panels), 2,000 nodes, 500 clips, 2,000 audio files; no view is capped.
    let mut owned: Vec<(String, String)> =
        vec![("project.k7x".into(), "project({ name: \"big\", tempo: 120 })\n".into())];
    for i in 0..5_000 {
        let text = if i < 200 { PANEL } else { PLUGIN };
        owned.push((format!("ws/_modules/audio/m{i}/module.k7x"), text.to_owned()));
    }
    for i in 0..2_000 {
        let text = format!(
            "extends(\"@workspace/audio/m{}\")\nparams({{ Level: {i} }})\n",
            i % 5_000
        );
        owned.push((format!("nodes/n{i}.k7x"), text));
    }
    for i in 0..500 {
        owned.push((format!("clips/c{i}.k7x"), format!("source(\"audio/a{i}.wav\")\n")));
    }
    for i in 0..2_000 {
        owned.push((format!("audio/a{i}.wav"), "RIFF".into()));
    }
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = views_world::world(&files);
    let roots = views_world::roots();
    let mut cache = Cache::new();
    let t = Instant::now();
    let inputs = load_inputs(&src, &mut cache, &roots, Rig::default()).expect("inputs");
    let loaded = t.elapsed();
    assert!(loaded < bound(), "loading took {loaded:?}");
    let h = fast("hints", || hints_of(&src, &mut cache, &roots, &inputs).expect("hints"));
    let io = fast("io ports", || io_ports(&inputs));
    let (cat, base) = fast("catalogue", || {
        (
            module_catalogue(&src, &roots, &inputs.vocabulary).expect("catalogue"),
            base_modules(&inputs.vocabulary),
        )
    });
    println!(
        "capacity: load {loaded:?}; hints spent {} (modules {}, nodes {}, clips {}, media {}); io {}; catalogue {} ({} rows); base {} ({} rows)",
        h.spent,
        h.hints.base_modules.len(),
        inputs.nodes.nodes.len(),
        h.hints.clips.len(),
        h.hints.media.values().map(Vec::len).sum::<usize>(),
        io.spent,
        cat.spent,
        cat.items.len(),
        base.spent,
        base.items.len()
    );
    for (what, spent, capped) in [
        ("hints", h.spent, h.capped),
        ("io ports", io.spent, io.capped),
        ("catalogue", cat.spent, cat.capped),
        ("base modules", base.spent, base.capped),
    ] {
        assert!(!capped && spent < BUDGET, "{what} spent {spent}");
    }
    assert!(h.spent > 10_000, "{}", h.spent);
    assert!(h.findings.is_empty() && h.hints.base_modules.len() >= 5_000);
    assert_eq!(h.hints.clips.len(), 500);
    assert_eq!(h.hints.media["audio"].len(), 2_000);
    assert_eq!(
        h.hints.paths.iter().filter(|(k, _)| k.starts_with("module:")).count(),
        2_000
    );
    assert_eq!(base.items.iter().filter(|b| b.panel.is_some()).count(), 200);
}

#[test]
fn two_thousand_wavs_are_read_once_and_a_second_pass_is_all_cache() {
    // capacity table: 2,000 distinct WAVs measured on disk, twice: one read per file the first time, none the second.
    let t = Tmp::new("capacity-wavs");
    for i in 0..2_000 {
        let name = format!("audio/a{i}.wav");
        std::fs::create_dir_all(t.0.join("audio")).expect("folder");
        std::fs::write(t.0.join(&name), wav(0.01 + f64::from(i % 50) / 100.0)).expect("a wav");
    }
    t.age();
    let root = support::spell(&t.0);
    let disk = DiskSource::new(std::slice::from_ref(&root)).expect("source");
    let spy = Spy {
        inner: &disk,
        reads: std::sync::Mutex::new(Vec::new()),
    };
    let paths: Vec<String> = (0..2_000).map(|i| format!("{root}/audio/a{i}.wav")).collect();
    let mut cache = Cache::new();
    let mut run = |label: &str| {
        let start = Instant::now();
        let mut by = ByHeader::new(&spy, &mut cache);
        by.measure_all(paths.iter().map(String::as_str)).expect("measures");
        let (lengths, findings) = by.finish();
        println!("capacity: {label} {:?}", start.elapsed());
        (lengths, findings)
    };
    let (first, warned) = run("first pass");
    assert_eq!((first.len(), warned.len()), (2_000, 0));
    assert_eq!(spy.reads.lock().expect("lock").len(), 2_000);
    let (second, warned) = run("second pass");
    assert_eq!((second.len(), warned.len()), (2_000, 0));
    assert_eq!(
        spy.reads.lock().expect("lock").len(),
        2_000,
        "the second pass read again"
    );
    assert_eq!(first, second);
}

#[test]
fn two_hundred_lanes_of_two_thousand_clips_draw_from_a_table_in_time() {
    // capacity table: 200 lanes of 2,000 clips of length 0 and a table of 2,000 lengths, through ByTable.
    let table: Lengths = (0..2_000)
        .map(|i| (format!("/a/a{i}.wav"), 1.0 + f64::from(i % 7)))
        .collect();
    let lane: Vec<ClipDoc> = (0..2_000)
        .map(|i| {
            views_support::clip(serde_json::json!({
                "id": format!("c{i}@{i}"), "name": format!("c{i}"), "source": format!("/a/a{i}.wav"), "start": i,
            }))
        })
        .collect();
    let start = Instant::now();
    let mut drawn = 0;
    let mut zero = 0;
    for _ in 0..200 {
        let mut m = ByTable(&table);
        let web = fold_nests(&lane, &mut |c: &ClipDoc| web_clip(c, Some(120.0), &mut m)).expect("folds");
        drawn += web.len();
        zero += web.iter().filter(|w| w.length == 0.0).count();
    }
    let took = start.elapsed();
    println!("capacity: 200 lanes of 2,000 clips in {took:?}");
    assert!(took < bound(), "{took:?}");
    assert_eq!((drawn, zero), (400_000, 0));
}
