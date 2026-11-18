//! `workspace_info`, `info_json` and `project_info` (2032, 2070): what the poll and the open answer, key by key.

use crate::support;

use k7_file_service::projects::info::{project_info, workspace_info};
use k7_file_service::projects::{Info, info_json};
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use support::projects::songs;
use support::ws::Acct;

#[test]
fn the_info_is_written_in_the_literal_s_key_order() {
    let info = Info {
        root: "/h/ws".into(),
        root_is_project: false,
        active: Some("a.k7".into()),
        projects: vec!["a.k7".into(), "d\"x/b.k7".into()],
        active_is_demo: true,
    };
    assert_eq!(
        info_json(&info, "\"engineLoad\":null,\"rev\":3"),
        r#"{"root":"/h/ws","rootIsProject":false,"activeProject":"a.k7","projects":["a.k7","d\"x/b.k7"],"activeIsDemo":true,"engineLoad":null,"rev":3}"#
    );
    let none = Info {
        active: None,
        active_is_demo: false,
        projects: vec![],
        ..info
    };
    assert_eq!(
        info_json(&none, ""),
        r#"{"root":"/h/ws","rootIsProject":false,"activeProject":null,"projects":[]}"#
    );
}

#[test]
fn a_workspace_lists_its_projects_afresh_and_names_the_open_one() {
    let (a, root) = songs("pi-list");
    a.project("songs", "deep/b.k7", "");
    a.marker("songs", "deep/b.k7\n");
    let h = a.home();
    let info = workspace_info(&Space::open(&h, &root), None, &Limits::default()).unwrap();
    assert_eq!(info.projects, ["a.k7", "deep/b.k7"]);
    assert_eq!(info.active.as_deref(), Some("deep/b.k7"));
    assert!(!info.root_is_project && !info.active_is_demo);
    assert_eq!(info.root, root);
    a.project("songs", "late.k7", "");
    let again = workspace_info(&Space::open(&h, &root), None, &Limits::default()).unwrap();
    assert!(
        again.projects.contains(&"late.k7".to_string()),
        "no memo: the new folder is seen"
    );
}

#[test]
fn a_workspace_that_is_a_project_lists_dot_first() {
    let a = Acct::new("pi-root");
    a.project("r.k7", "x.k7", "");
    a.s.file("studio/workspace/r.k7/project.k7x", "");
    let h = a.home();
    let info = workspace_info(
        &Space::open(&h, &a.path("r.k7", "")),
        None,
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(info.projects, [".", "x.k7"]);
    assert_eq!(info.active.as_deref(), Some("."));
    assert!(info.root_is_project);
}

#[test]
fn only_a_bundled_demo_open_in_examples_is_marked() {
    let a = Acct::new("pi-demo");
    a.s.dir("demos/demo.k7");
    a.project("Examples", "demo.k7", "");
    a.project("Examples", "mine.k7", "");
    a.project("Examples", "deep/demo.k7", "");
    a.project("songs", "demo.k7", "");
    let demos = a.s.path("demos");
    let h = a.home();
    let flag = |ws: &str, open: &str| {
        a.marker(ws, &format!("{open}\n"));
        let root = a.path(ws, "");
        workspace_info(&Space::open(&h, &root), Some(&demos), &Limits::default())
            .unwrap()
            .active_is_demo
    };
    assert!(flag("Examples", "demo.k7"));
    assert!(!flag("Examples", "mine.k7"));
    assert!(!flag("Examples", "deep/demo.k7"));
    assert!(!flag("songs", "demo.k7"));
}

#[test]
fn a_walk_past_its_limit_or_a_library_marker_declines() {
    let (a, root) = songs("pi-declined");
    for n in 0..40 {
        a.s.dir(&format!("studio/workspace/songs/d{n}"));
    }
    let h = a.home();
    let tight = Limits {
        max_seen: 30,
        ..Limits::default()
    };
    assert_eq!(
        workspace_info(&Space::open(&h, &root), None, &tight),
        Err(Declined::TooMany)
    );
    a.marker("songs", "_libraries/k7-core/x.k7\n");
    assert_eq!(
        workspace_info(&Space::open(&h, &root), None, &Limits::default()),
        Err(Declined::LibraryActive)
    );
}

fn name_of(a: &Acct, root: &str) -> Result<String, Declined> {
    let h = a.home();
    project_info(&Space::open(&h, root))
}

#[test]
fn the_poll_names_the_project_by_its_own_file_else_its_folder() {
    let a = Acct::new("pi-name");
    let root = a.ws("songs");
    a.marker("songs", "a.k7\n");
    for (body, name) in [
        ("project({ name: \"Winter\" })\n", "Winter"),
        ("project({ name: 12 })\n", "a.k7"),
        ("project({ title: \"x\" })\n", "a.k7"),
        ("this is { not k7x", "a.k7"),
        ("project(\"just text\")\n", "a.k7"),
        ("project({ name: \"é 日本\" })\n", "é 日本"),
    ] {
        a.project("songs", "a.k7", body);
        let want = format!(
            "{{\"name\":{},\"root\":\"{root}/a.k7\"}}",
            serde_json::to_string(name).unwrap()
        );
        assert_eq!(name_of(&a, &root), Ok(want), "{body:?}");
    }
    std::fs::remove_file(format!("{root}/a.k7/project.k7x")).unwrap();
    assert!(
        name_of(&a, &root)
            .unwrap()
            .starts_with("{\"name\":\"a.k7\"")
    );
}

#[test]
fn with_no_project_open_the_poll_names_the_workspace() {
    let a = Acct::new("pi-none");
    let root = a.ws("songs");
    assert_eq!(
        name_of(&a, &root),
        Ok(format!("{{\"name\":\"songs\",\"root\":\"{root}\"}}"))
    );
}

#[test]
fn a_text_the_reader_would_read_otherwise_declines_the_poll() {
    let a = Acct::new("pi-lossy");
    let root = a.ws("songs");
    a.marker("songs", "a.k7\n");
    a.project("songs", "a.k7", "project({ name: \"\\ud83d\" })\n");
    assert_eq!(name_of(&a, &root), Err(Declined::LossyText));
}
