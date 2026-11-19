//! Hostile input to the listings and the names (1634, 1635): a thousand workspaces, a root of dot entries, names of
//! 16 KiB, a tree too wide or too deep, files at their limits. Each answered without a hang; what can be counted is.

use crate::support;

use std::time::{Duration, Instant};

use k7_file_service::space::{Declined, MAX_REL};
use k7_file_service::tree::Limits;
use k7_file_service::workspaces::listing::{rows, summaries};
use k7_file_service::workspaces::names::*;
use support::ws::{Acct, TIMELINE, WORKSPACES};

/// A backstop against a hang, not a measure: a loaded agent's slow turn is never the code's.
const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

/// The best of three runs of `f`, and its last answer.
fn best<T>(mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut best = Duration::MAX;
    let mut last = None;
    for _ in 0..3 {
        let t0 = Instant::now();
        let v = f();
        best = best.min(t0.elapsed());
        last = Some(v);
    }
    (best, last.unwrap())
}

#[test]
fn fifty_workspaces_and_a_root_of_dot_entries_list_by_count() {
    let a = Acct::new("ws-hostile-many");
    let dir = a.s.path(WORKSPACES);
    for i in 0..50 {
        std::fs::create_dir(format!("{dir}/w{i:04}")).unwrap();
    }
    for i in 0..5_000 {
        std::fs::File::create(format!("{dir}/.d{i}")).unwrap();
    }
    let (took, got) = best(|| summaries(&a.home(), &Limits::default()).unwrap());
    assert_eq!(got.len(), 50);
    assert!(took < BOUND, "{took:?}");
    // The walk counts every entry it meets, hidden or not, against the limit.
    let few = Limits {
        max_seen: 2_000,
        ..Limits::default()
    };
    assert_eq!(summaries(&a.home(), &few), Err(Declined::TooMany));
}

#[test]
fn a_name_of_16_kib_is_refused_without_a_walk() {
    let a = Acct::new("ws-hostile-name");
    let long = "a".repeat(16 * 1024);
    assert!(!name_ok(&format!("é{long}")));
    assert!(name_ok(&long));
    assert_eq!(on_disk(&a.s.path(WORKSPACES), &long), None);
}

#[test]
fn a_workspace_of_many_folders_is_declined_by_count() {
    let a = Acct::new("ws-hostile-wide");
    let root = a.ws("wide");
    for i in 0..3000 {
        std::fs::create_dir(format!("{root}/d{i}")).unwrap();
    }
    let few = Limits {
        max_seen: 1000,
        ..Limits::default()
    };
    assert_eq!(rows(&a.home(), None, &few).err(), Some(Declined::TooMany));
    assert_eq!(
        project_dirs_under(&root, &few, &mut 0),
        Err(Declined::TooMany)
    );
    assert_eq!(
        project_dirs_under(&root, &Limits::default(), &mut 0)
            .unwrap()
            .len(),
        0
    );
}

#[cfg(unix)]
#[test]
fn a_nest_300_deep_is_declined_and_the_stack_holds() {
    let a = Acct::new("ws-hostile-deep");
    let root = a.ws("deep");
    std::fs::create_dir_all(format!("{root}/{}", vec!["n"; 300].join("/"))).unwrap();
    let t0 = Instant::now();
    assert_eq!(
        project_dirs_under(&root, &Limits::default(), &mut 0),
        Err(Declined::TooDeep)
    );
    assert!(t0.elapsed() < BOUND);
    let deep = Limits {
        max_depth: 400,
        ..Limits::default()
    };
    assert_eq!(project_dirs_under(&root, &deep, &mut 0).unwrap().len(), 0);
}

#[test]
fn a_project_file_at_the_limit_reads_and_one_past_it_declines() {
    let a = Acct::new("ws-hostile-big");
    let body = format!("project({{ name: \"{}\" }})\n", "x".repeat(1 << 20));
    a.project("w", "big.k7", &body);
    let size = body.len() as u64;
    let at = Limits {
        max_text: size,
        ..Limits::default()
    };
    let (took, ok) = best(|| rows(&a.home(), None, &at));
    assert!(ok.is_ok() && took < BOUND, "{took:?}");
    let under = Limits {
        max_text: size - 1,
        ..Limits::default()
    };
    assert_eq!(
        rows(&a.home(), None, &under).err(),
        Some(Declined::TooLarge)
    );
}

#[test]
fn a_project_file_of_16_mib_is_read_whole_by_default() {
    let a = Acct::new("ws-hostile-16m");
    a.project(
        "w",
        "huge.k7",
        &format!("project({{ name: \"{}\" }})\n", "y".repeat(16 << 20)),
    );
    let t0 = Instant::now();
    let (r, _) = rows(&a.home(), None, &Limits::default()).unwrap();
    assert_eq!(r[0].projects.len(), 1);
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn ten_thousand_tags_and_a_thousand_surfaces_are_listed() {
    let a = Acct::new("ws-hostile-tags");
    let tags: Vec<String> = (0..10_000).map(|i| format!("\"t{i}\"")).collect();
    a.project(
        "w",
        "t.k7",
        &format!("project({{ tags: [{}] }})\n", tags.join(",")),
    );
    for i in 0..1000 {
        a.surface("w", "t.k7", &format!("s{i:04}"), TIMELINE);
    }
    let (took, got) = best(|| rows(&a.home(), None, &Limits::default()).unwrap().0);
    let p = &got[0].projects[0];
    assert_eq!((p.tags.len(), p.surfaces.len()), (10_000, 1000));
    assert!(took < BOUND, "{took:?}");
}

#[test]
fn a_marker_of_16_kib_reads_and_a_longer_one_declines() {
    let a = Acct::new("ws-hostile-marker");
    a.project("w", "p.k7", "project({})\n");
    a.marker("w", &"d/".repeat(MAX_REL / 2 - 4));
    assert!(rows(&a.home(), None, &Limits::default()).is_ok());
    a.marker("w", &"d/".repeat(MAX_REL));
    assert_eq!(
        rows(&a.home(), None, &Limits::default()).err(),
        Some(Declined::LongPath)
    );
    // White space around a short path is trimmed whatever its length, as the TypeScript trims it.
    a.marker("w", &format!("{}p.k7", " ".repeat(MAX_REL * 3)));
    let (r, _) = rows(&a.home(), None, &Limits::default()).unwrap();
    assert!(r[0].projects[0].active);
    a.marker("w", &"z".repeat(2 << 20));
    assert_eq!(
        rows(&a.home(), None, &Limits::default()).err(),
        Some(Declined::LongPath)
    );
}

#[test]
fn a_bundle_of_2000_files_is_stamped_and_mirrored_and_a_16_mib_stamps_file_reads_as_none() {
    use k7_file_service::workspaces::demos::{mirror, plan_mirror, tree_stamp};
    let a = Acct::new("ws-hostile-bundle");
    let src = a.s.dir("bundle/big.k7");
    for d in 0..20 {
        std::fs::create_dir_all(format!("{src}/d{d}")).unwrap();
        for f in 0..100 {
            std::fs::write(format!("{src}/d{d}/f{f}.txt"), "x").unwrap();
        }
    }
    let bundle = a.s.path("bundle");
    let ex = a.s.path("studio/workspace/Examples");
    a.s.file(
        "studio/workspace/Examples/.studio-demos.json",
        &"x".repeat(16 << 20),
    );
    let (took, stamp) = best(|| tree_stamp(&src));
    assert_eq!(stamp.len(), 40);
    assert!(took < BOUND, "{took:?}");
    assert_eq!(plan_mirror(&bundle, &ex).copy, ["big.k7"]);
    // Counted, not timed: the copy's entries are bounded by the plan, and its writes are the disk's pace.
    assert_eq!(mirror(&bundle, &ex), ["big.k7"]);
    assert!(std::fs::metadata(format!("{ex}/big.k7/d19/f99.txt")).is_ok());
    assert!(plan_mirror(&bundle, &ex).copy.is_empty());
    a.s.file("studio/workspace/Examples/.studio-demos.json", "[1, 2, 3]");
    assert_eq!(plan_mirror(&bundle, &ex).copy, ["big.k7"]);
}

#[test]
fn a_project_of_500_files_is_planned_and_copied_within_the_bound() {
    use k7_file_service::workspaces::copy::{CopyAsk, apply_copy, plan_copy};
    let a = Acct::new("ws-hostile-copy");
    let big = a.project("songs", "big.k7", "project({})\n");
    for d in 0..5 {
        std::fs::create_dir_all(format!("{big}/d{d}")).unwrap();
        for f in 0..100 {
            std::fs::write(format!("{big}/d{d}/f{f}.bin"), "x").unwrap();
        }
    }
    a.ws("albums");
    let ask = CopyAsk {
        from: "songs",
        path: "big.k7",
        to: "albums",
        name: "big.k7",
    };
    let t0 = Instant::now();
    let plan = plan_copy(&a.home(), &ask, &Limits::default()).unwrap();
    assert!(t0.elapsed() < BOUND, "{:?}", t0.elapsed());
    // Counted, not timed: writing 500 files on a loaded disk says nothing of a hang.
    assert_eq!(plan.entries.len(), 1 + 5 + 500 + 1);
    apply_copy(&plan).unwrap();
    assert!(std::fs::metadata(a.path("albums", "big.k7/d4/f99.bin")).is_ok());
}
