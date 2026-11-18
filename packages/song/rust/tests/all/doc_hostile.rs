// doc_hostile.rs — the compile against projects that are small to write and large to build (rustCompilerDoc.md, "Hostile input").
// A reader would get wrong that each case asserts the answer too: a finding set or a document shape, and the time, fifteen seconds (sixty on musl).

use crate::doc_support;

use doc_support::{AMP, compiled, lane, stopped};
use song::doc::{BUDGET, Compiled};
use std::time::{Duration, Instant};

fn hang_bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

fn timed(what: &str, f: impl FnOnce() -> Compiled) -> Compiled {
    let t = Instant::now();
    let c = f();
    assert!(t.elapsed() < hang_bound(), "{what}: {:?}", t.elapsed());
    c
}

fn run(files: Vec<(String, String)>) -> Compiled {
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    compiled(&refs)
}

fn modules_line(pairs: &[(String, String)]) -> String {
    let body: Vec<String> = pairs.iter().map(|(k, v)| format!("\"{k}\": [{v}]")).collect();
    format!("modules({{ {} }})\n", body.join(", "))
}

// 317, 323: 200 steps naming one Faust source of 900 KB read it once and weigh it at every copy, so the walk is capped.
#[test]
fn one_faust_source_named_by_many_steps_is_read_once_and_weighed_each_time() {
    let mut files = vec![
        ("nodes/gtr.k7x".to_owned(), lane("")),
        ("nodes/dsp.dsp".to_owned(), "process = _;\n".repeat(64_000)),
    ];
    let mut pairs = Vec::new();
    for i in 0..200 {
        files.push((
            format!("nodes/f{i}.k7x"),
            "plugin({ type: \"faust\", name: \"f\" })\n".to_owned(),
        ));
        pairs.push((
            if i == 0 {
                "gtr".to_owned()
            } else {
                format!("f{}", i - 1)
            },
            format!("\"f{i}\""),
        ));
    }
    pairs.push(("f199".to_owned(), "\"MIX\"".to_owned()));
    files.push(("graph.k7x".to_owned(), modules_line(&pairs)));
    let c = timed("200 steps of one Faust source", || run(files));
    let f = c.engine.as_ref().expect_err("capped");
    assert_eq!(
        f.diagnostics.len(),
        1,
        "{:?}",
        f.diagnostics
            .iter()
            .map(|d| (&d.file, d.message.chars().take(60).collect::<String>()))
            .collect::<Vec<_>>()
    );
    assert!(f.diagnostics[0].file.starts_with("nodes/f"));
    assert!(c.spent.doc > BUDGET, "{:?}", c.spent);
}

// 312: 20,000 chains that own nothing are walked once each.
#[test]
fn chains_owning_nothing_are_linear() {
    let mut files = vec![
        ("nodes/gtr.k7x".to_owned(), lane("")),
        ("nodes/amp.k7x".to_owned(), AMP.to_owned()),
    ];
    let mut pairs = vec![("gtr".to_owned(), "\"MIX\"".to_owned())];
    for i in 0..20_000 {
        files.push((format!("nodes/x{i}.k7x"), AMP.to_owned()));
        pairs.push((format!("x{i}"), "\"y\"".to_owned()));
    }
    files.push(("graph.k7x".to_owned(), modules_line(&pairs)));
    let c = timed("20,000 chains owning nothing", || run(files));
    assert!(c.engine.is_ok(), "{:?}", c.diagnostics.first());
}

// 311: 2,500 events naming 2,500 refs with no half are 1,000 ERRORs and one more, and the walk stops.
#[test]
fn unwired_events_are_one_thousand_errors_and_one_more() {
    let mut files = vec![("nodes/amp.k7x".to_owned(), AMP.to_owned())];
    let mut events = Vec::new();
    for i in 0..2_500 {
        files.push((format!("sequences/s{i}.k7x"), "notes([t(0).d(1).k(60)])\n".to_owned()));
        events.push(format!("{{ ref: \"s{i}\", start: \"1:1\" }}"));
    }
    files.push((
        "nodes/gtr.k7x".to_owned(),
        lane(&format!("events([{}])\n", events.join(", "))),
    ));
    files.push((
        "graph.k7x".to_owned(),
        "modules({ \"gtr#audio\": [\"amp\"], amp: [\"MIX\"] })\n".to_owned(),
    ));
    let c = timed("2,500 unwired events", || run(files));
    let n = c
        .engine
        .as_ref()
        .expect_err("a failure")
        .diagnostics
        .iter()
        .filter(|d| d.file == "nodes/gtr.k7x" || d.file == "project.k7x")
        .count();
    assert!((1_000..=1_003).contains(&n), "{n}");
}

// Tracks: 3,000 lanes in one group, all sending to one bus.
#[test]
fn a_group_of_many_lanes_and_a_bus_fan_in_stay_linear() {
    let names: Vec<String> = (0..3_000).map(|i| format!("l{i}")).collect();
    let mut files = vec![
        ("nodes/amp.k7x".to_owned(), AMP.to_owned()),
        ("nodes/bus.k7x".to_owned(), AMP.to_owned()),
    ];
    let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    files.push((
        "nodes/band.k7x".to_owned(),
        format!("extends(\"@meta/group-track\")\ntracks([{}])\n", quoted.join(", ")),
    ));
    let mut pairs = Vec::new();
    for n in &names {
        files.push((format!("nodes/{n}.k7x"), lane("")));
        pairs.push((n.clone(), "\"bus\"".to_owned()));
    }
    pairs.push(("bus".to_owned(), "\"MIX\"".to_owned()));
    files.push(("graph.k7x".to_owned(), modules_line(&pairs)));
    let c = timed("3,000 lanes in a group into one bus", || run(files));
    let d = c.engine.expect("a document");
    assert_eq!(d.tracks.len(), 3_001);
    assert!(
        d.tracks
            .iter()
            .take(3_000)
            .all(|t| t.group == "band" && t.sends.len() == 1)
    );
    assert_eq!(d.groups.len(), 1);
}

// Recorder: 3,000 surface files are read once each and none of them a timeline.
#[test]
fn many_surface_files_are_read_and_a_nest_is_one_finding() {
    let mut files = vec![
        ("nodes/gtr.k7x".to_owned(), lane("")),
        (
            "graph.k7x".to_owned(),
            "modules({ gtr: [\"MIX\"], MIX: [\"OUT\"] })\n".to_owned(),
        ),
    ];
    for i in 0..3_000 {
        files.push((
            format!("surfaces/s{i}.k7x"),
            "type(\"viewport\")\nlayout(\"floating\")\n".to_owned(),
        ));
    }
    files.push((
        "surfaces/deep.k7x".to_owned(),
        format!("type(\"timeline\")\nlayout(\"split\")\nmapping({})", "{a:".repeat(300)),
    ));
    let c = timed("3,000 surfaces", || run(files));
    let d = c.engine.expect("a document");
    assert_eq!(
        d.master_plugins.len(),
        1,
        "the timeline surface that does not read is no timeline"
    );
    assert!(c.diagnostics.iter().any(|f| f.file == "surfaces/deep.k7x"));
}

// Capacity (measured): 128 lanes of 200 events, 16 plugins a lane of 60 params, 16 buses, 64 VSTs, 3,000 automations.
#[test]
fn a_full_size_project_compiles_and_spends_a_third_of_no_budget() {
    let files = doc_support::capacity_files();
    let c = timed("a full-size project", || run(files));
    let d = c
        .engine
        .as_ref()
        .unwrap_or_else(|f| panic!("{:?}", f.diagnostics.iter().take(3).collect::<Vec<_>>()));
    let s = c.spent;
    eprintln!(
        "capacity: {} tracks, {} freezes, spent {s:?}",
        d.tracks.len(),
        c.freezes.len()
    );
    assert_eq!(d.tracks.iter().filter(|t| t.role == "regular").count(), 128);
    assert!(
        s.doc * 3 <= BUDGET && s.baker * 3 <= song::curves::BUDGET && s.freeze * 3 <= song::freeze::BUDGET,
        "{s:?}"
    );
    // 2f's placer weighs a clip with its notes: 25,600 clips of 16 notes spend 46% of its 1,000,000 (a question in the report).
    assert!(s.placer * 2 <= song::clips::BUDGET, "{s:?}");
    assert!(stopped(&c).is_empty());
}

/// 400 frozen modules, each at a render that is there, all fed by one lane named `lane_name`.
fn frozen_behind(lane_name: &str) -> Vec<(String, String)> {
    let mut files = vec![("r.wav".to_owned(), "RIFF".to_owned())];
    // A file name past a bundle key's 8 KiB is no real file, so a long lane is named by the graph alone.
    if lane_name.len() < 200 {
        files.push((format!("nodes/{lane_name}.k7x"), lane("")));
    }
    let names: Vec<String> = (0..400).map(|i| format!("\"m{i}\"")).collect();
    for i in 0..400 {
        files.push((
            format!("nodes/m{i}.k7x"),
            "plugin({ type: \"internal\", name: \"Chorus\" })\nfrozen(\"r.wav\")\n".to_owned(),
        ));
    }
    let mut pairs = vec![(lane_name.to_owned(), names.join(", "))];
    pairs.extend((0..400).map(|i| (format!("m{i}"), "\"OUT\"".to_owned())));
    files.push(("graph.k7x".into(), modules_line(&pairs)));
    files
}

// 348: a lane named by a megabyte is copied into the plan of each module it feeds, each copy paid by its bytes, so 400 of them stop the manual freeze.
#[test]
fn a_name_of_a_megabyte_in_every_plan_is_paid_by_its_bytes() {
    let c = timed("short name", || run(frozen_behind("k")));
    assert!(
        c.engine.is_ok() && c.spent.freeze < song::freeze::BUDGET / 10,
        "{:?}",
        c.spent
    );
    let c = timed("a megabyte", || run(frozen_behind(&"k".repeat(1 << 20))));
    assert!(c.engine.is_err(), "ok with {:?} spent", c.spent);
    assert_eq!(stopped(&c), ["project.k7x"]);
}
