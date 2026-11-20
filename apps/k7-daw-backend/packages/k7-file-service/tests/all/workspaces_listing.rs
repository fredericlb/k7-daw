//! The home page's listings (1633 to 1637): a count per workspace, every workspace with its projects, their facts, and
//! the JSON they are written as; the open project, tags, surfaces, `editedAt` and the demo flag, on a real disk.

use crate::support;

use k7_file_service::space::Declined;
use k7_file_service::tree::Limits;
use k7_file_service::workspaces::listing::*;
use k7_file_service::workspaces::*;
use support::ws::{Acct, TIMELINE, TURNTABLES, touch};
use support::{can_link, link};

fn limits() -> Limits {
    Limits::default()
}

fn all(a: &Acct, demos: Option<&str>) -> Vec<Row> {
    rows(&a.home(), demos, &limits()).unwrap().0
}

fn project<'a>(rows: &'a [Row], ws: &str, path: &str) -> &'a Project {
    let row = rows.iter().find(|r| r.name == ws).unwrap();
    row.projects
        .iter()
        .find(|p| p.path == path)
        .unwrap_or_else(|| panic!("{ws}/{path}: {row:?}"))
}

#[test]
fn the_names_held_equal_to_web_protocols() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap();
    let text = std::fs::read_to_string(repo.join("apps/k7-daw-frontend/packages/web-protocol/projects.ts")).unwrap();
    assert!(text.contains(&format!("EXAMPLES_WORKSPACE = \"{EXAMPLES}\"")));
    assert!(text.contains(&format!("BENCH_WORKSPACE = \"{BENCH}\"")));
}

#[test]
fn summaries_count_each_workspace_and_a_single_project_is_one() {
    let a = Acct::new("ws-list-summaries");
    a.project("songs", "a.k7", "project({})\n");
    a.project("songs", "group/b.k7", "project({})\n");
    a.project("songs", "group/b.k7/nested/c.k7", "project({})\n");
    a.ws("empty");
    a.project("solo.k7", ".", "project({})\n");
    a.ws(".hidden");
    a.ws("Banc");
    let got = summaries(&a.home(), &limits()).unwrap();
    let want = [
        ("empty", false, 0),
        ("solo.k7", true, 1),
        ("songs", false, 3),
    ];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, (name, single, count)) in got.iter().zip(want) {
        assert_eq!((g.name.as_str(), g.single, g.count), (name, single, count));
    }
    assert_eq!(
        summaries_json(&got),
        r#"{"workspaces":[{"name":"empty","isSingleProject":false,"projectCount":0},{"name":"solo.k7","isSingleProject":true,"projectCount":1},{"name":"songs","isSingleProject":false,"projectCount":3}]}"#
    );
}

#[test]
fn a_workspace_that_is_a_project_lists_itself_as_dot() {
    let a = Acct::new("ws-list-single");
    a.project("solo.k7", ".", "project({ tags: [\"x\"] })\n");
    a.project("solo.k7", "inner.k7", "project({})\n");
    let r = all(&a, None);
    assert_eq!(r[0].projects.len(), 1);
    let p = project(&r, "solo.k7", ".");
    assert_eq!(
        (p.app, p.tags.as_slice()),
        ("k7", ["x".to_string()].as_slice())
    );
}

#[test]
fn tags_are_the_non_empty_strings_and_a_bad_file_has_none() {
    let a = Acct::new("ws-list-tags");
    a.project(
        "w",
        "tagged.k7",
        "project({ tags: [\"a\", \"\", \"b\", 3, null, \"Live\", \"live\"] })\n",
    );
    a.project("w", "broken.k7", "project({ tags: [");
    a.project("w", "string.k7", "project({ tags: \"a\" })\n");
    a.ws("w");
    std::fs::create_dir_all(a.path("w", "nofile.k7")).unwrap();
    let r = all(&a, None);
    assert_eq!(
        project(&r, "w", "tagged.k7").tags,
        ["a", "b", "Live", "live"]
    );
    for p in ["broken.k7", "string.k7", "nofile.k7"] {
        assert!(project(&r, "w", p).tags.is_empty(), "{p}");
    }
}

#[test]
fn a_text_the_reader_would_read_otherwise_declines_the_listing() {
    let a = Acct::new("ws-list-lossy");
    a.project("w", "lone.k7", "project({ tags: [\"\\ud83d\"] })\n");
    assert_eq!(
        rows(&a.home(), None, &limits()).err(),
        Some(Declined::LossyText)
    );
    let b = Acct::new("ws-list-large");
    b.project(
        "w",
        "big.k7",
        &format!("project({{ name: \"{}\" }})\n", "x".repeat(2000)),
    );
    let small = Limits {
        max_text: 1000,
        ..limits()
    };
    assert_eq!(
        rows(&b.home(), None, &small).err(),
        Some(Declined::TooLarge)
    );
    let exact = Limits {
        max_text: std::fs::metadata(b.path("w", "big.k7/project.k7x"))
            .unwrap()
            .len(),
        ..limits()
    };
    assert!(rows(&b.home(), None, &exact).is_ok());
}

#[test]
fn surfaces_are_the_decoders_in_its_order_and_a_bad_file_is_skipped() {
    let a = Acct::new("ws-list-surfaces");
    a.project("w", "s.k7", "project({})\n");
    a.surface("w", "s.k7", "b-deck", TURNTABLES);
    a.surface("w", "s.k7", "a-line", TIMELINE);
    a.surface("w", "s.k7", "broken", "type(");
    a.project("w", "none.k7", "project({})\n");
    let r = all(&a, None);
    let got: Vec<_> = project(&r, "w", "s.k7")
        .surfaces
        .iter()
        .map(|s| (s.id.as_str(), s.kind.as_str(), s.playlist.as_deref()))
        .collect();
    assert_eq!(
        got,
        [
            ("a-line", "timeline", None),
            ("b-deck", "turntables", Some("Fred"))
        ]
    );
    assert!(project(&r, "w", "none.k7").surfaces.is_empty());
    let json = surfaced_json(&r);
    assert_eq!(
        json,
        r#"{"projects":[{"workspace":"w","path":"s.k7","app":"k7","active":false,"surfaces":[{"id":"a-line","type":"timeline"},{"id":"b-deck","type":"turntables"}]}]}"#
    );
}

#[test]
fn a_surfaces_folder_linked_out_declines_the_listing() {
    let a = Acct::new("ws-list-outside");
    if !can_link(&a.s) {
        return;
    }
    a.project("w", "s.k7", "project({})\n");
    let outside = a.s.dir("outside");
    a.s.file("outside/x.k7x", TIMELINE);
    link(&outside, &a.path("w", "s.k7/surfaces")).unwrap();
    assert_eq!(
        rows(&a.home(), None, &limits()).err(),
        Some(Declined::SurfacesOutside)
    );
}

#[test]
fn the_open_project_is_the_markers_resolved_path() {
    let a = Acct::new("ws-list-active");
    a.project("w", "one.k7", "project({})\n");
    a.project("w", "two.k7", "project({})\n");
    a.marker("w", "  two.k7\n");
    let r = all(&a, None);
    assert!(!project(&r, "w", "one.k7").active);
    assert!(project(&r, "w", "two.k7").active);
    a.marker("w", "./two.k7/../one.k7");
    let r = all(&a, None);
    assert!(project(&r, "w", "one.k7").active && !project(&r, "w", "two.k7").active);
    for none in ["", "   \n", "_libraries/k7-core", "gone.k7", "/etc"] {
        a.marker("w", none);
        let r = all(&a, None);
        assert!(r[0].projects.iter().all(|p| !p.active), "{none:?}");
    }
    std::fs::remove_file(a.s.path("studio/workspace/.active-projects/w")).unwrap();
    assert!(all(&a, None)[0].projects.iter().all(|p| !p.active));
}

#[test]
fn edited_at_is_the_newest_mtime_two_levels_down_rounded() {
    let a = Acct::new("ws-list-edited");
    let p = a.project("w", "p.k7", "project({})\n");
    let top = format!("{p}/project.k7x");
    touch(&top, 1_700_000_000, 500_000);
    let second = a.s.file("studio/workspace/w/p.k7/nodes/n.k7x", "x");
    touch(&second, 1_700_000_100, 400_000);
    let third = a.s.file("studio/workspace/w/p.k7/nodes/deep/d.k7x", "x");
    touch(&third, 1_800_000_000, 0);
    let dot = a.s.file("studio/workspace/w/p.k7/.hidden", "x");
    touch(&dot, 1_900_000_000, 0);
    let dotted = a.s.file("studio/workspace/w/p.k7/.dir/f", "x");
    touch(&dotted, 1_900_000_000, 0);
    // 1,700,000,100,000.4 ms rounds down; the third level and the dot entries are not read.
    assert_eq!(edited_at(&p), Some(1_700_000_100_000.0));
    touch(&second, 1_700_000_100, 500_000);
    assert_eq!(edited_at(&p), Some(1_700_000_100_001.0));
    let r = all(&a, None);
    assert!(rows_json(&r).contains("\"editedAt\":1700000100001"));
    assert_eq!(edited_at(&a.s.path("nowhere")), None);
    std::fs::create_dir_all(a.path("w", "empty.k7")).unwrap();
    assert_eq!(project(&all(&a, None), "w", "empty.k7").edited_at, None);
}

#[test]
fn the_demo_flag_is_a_bundled_name_one_segment_into_examples() {
    let a = Acct::new("ws-list-demo");
    let demos = a.s.dir("bundle");
    a.s.dir("bundle/synth.k7");
    a.s.dir("bundle/.hidden");
    a.project("Examples", "synth.k7", "project({})\n");
    a.project("Examples", "mine.k7", "project({})\n");
    a.project("Examples", "nest/synth.k7", "project({})\n");
    a.project("songs", "synth.k7", "project({})\n");
    let r = all(&a, Some(&demos));
    assert!(project(&r, "Examples", "synth.k7").demo);
    for (ws, p) in [
        ("Examples", "mine.k7"),
        ("Examples", "nest/synth.k7"),
        ("songs", "synth.k7"),
    ] {
        assert!(!project(&r, ws, p).demo, "{ws}/{p}");
    }
    assert!(
        !all(&a, None)
            .iter()
            .any(|r| r.projects.iter().any(|p| p.demo))
    );
}

#[test]
fn the_rows_are_written_in_the_literals_key_order_and_absent_keys_are_absent() {
    let a = Acct::new("ws-list-json");
    a.project("w", "p.k7", "project({ tags: [\"x\"] })\n");
    a.surface("w", "p.k7", "t", TIMELINE);
    touch(&a.path("w", "p.k7/project.k7x"), 1_700_000_000, 0);
    touch(&a.path("w", "p.k7/surfaces/t.k7x"), 1_700_000_000, 0);
    a.project("w", "q.k7", "project({})\n");
    std::fs::remove_file(a.path("w", "q.k7/project.k7x")).unwrap();
    a.marker("w", "p.k7");
    let (r, stamps) = rows(&a.home(), None, &limits()).unwrap();
    assert_eq!(
        rows_json(&r),
        r#"{"workspaces":[{"name":"w","isSingleProject":false,"projects":[{"path":"p.k7","app":"k7","tags":["x"],"surfaces":["timeline"],"active":true,"editedAt":1700000000000},{"path":"q.k7","app":"k7","surfaces":[],"active":false}]}]}"#
    );
    assert_eq!(stamps.folders.len(), 1);
    assert_eq!(stamps.folders[0].0, "w");
    assert!(stamps.root_mtime > 0.0 && stamps.folders[0].1 > 0.0);
}

#[test]
fn a_name_is_json_quoted_whole() {
    let rows = [Row {
        name: "a\"b\\c\u{1f600}".into(),
        single: false,
        projects: vec![],
    }];
    assert_eq!(
        rows_json(&rows),
        "{\"workspaces\":[{\"name\":\"a\\\"b\\\\c\u{1f600}\",\"isSingleProject\":false,\"projects\":[]}]}"
    );
}
