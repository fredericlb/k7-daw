//! The names of the workspaces (1630, 1632, 1634): the allow-list, the folders that are workspaces, the folder's own
//! spelling of a name, the project folders under a root; links, case, unreadable folders and a drive-letter home.

use crate::support;

use k7_file_service::libraries::Home;
use k7_file_service::space::Declined;
use k7_file_service::tree::Limits;
use k7_file_service::workspaces::names::*;
use support::ws::Acct;
use support::{can_link, link};

fn dirs(a: &Acct, ws: &str, project_dirs: &[&str]) -> String {
    let root = a.ws(ws);
    for p in project_dirs {
        a.s.dir(&format!("{}/{ws}/{p}", support::ws::WORKSPACES));
    }
    root
}

fn under(root: &str) -> Result<Vec<String>, Declined> {
    project_dirs_under(root, &Limits::default(), &mut 0)
}

#[test]
fn the_allow_list_is_assert_workspace_name() {
    for ok in [
        "a",
        "A1",
        "9",
        "a b",
        "a-b_c.d e",
        "a.",
        "a ",
        "Songs",
        "Examples",
        "Banc",
    ] {
        assert!(name_ok(ok), "{ok:?}");
    }
    for bad in [
        "", ".", "..", "...", " a", "-a", "_a", ".a", "a/b", "a\\b", "é", "日本", "a\n", "a\0",
        "a\u{a0}",
    ] {
        assert!(!name_ok(bad), "{bad:?}");
    }
    assert!(!name_ok(&"é".repeat(10)));
    assert!(name_ok(&"a".repeat(255)));
}

#[test]
fn folder_names_are_dirent_folders_sorted_by_code_point() {
    let a = Acct::new("ws-names-folders");
    for n in [
        "b",
        "C",
        "a",
        ".hidden",
        "Banc",
        "é",
        "\u{ff5e}",
        "\u{1f600}",
    ] {
        a.ws(n);
    }
    a.s.file(&format!("{}/afile", support::ws::WORKSPACES), "x");
    let got = folder_names(&a.s.path(support::ws::WORKSPACES));
    assert_eq!(got, ["C", "a", "b", "é", "\u{ff5e}", "\u{1f600}"]);
    assert!(folder_names(&a.s.path("nowhere")).is_empty());
}

#[test]
fn a_link_to_a_folder_is_no_workspace() {
    let a = Acct::new("ws-names-link");
    if !can_link(&a.s) {
        return;
    }
    a.ws("real");
    let dir = a.s.path(support::ws::WORKSPACES);
    link(&a.s.path("probe-target"), &format!("{dir}/linked")).unwrap();
    link(&a.ws("real"), &format!("{dir}/alias")).unwrap();
    assert_eq!(folder_names(&dir), ["real"]);
}

#[test]
fn on_disk_gives_the_folders_own_spelling_where_the_disk_folds_case() {
    let a = Acct::new("ws-names-case");
    let dir = a.s.path(support::ws::WORKSPACES);
    a.ws("Songs");
    assert_eq!(on_disk(&dir, "Songs").as_deref(), Some("Songs"));
    assert_eq!(on_disk(&dir, "absent"), None);
    assert_eq!(on_disk(&dir, &"x".repeat(20_000)), None);
    // What the disk says decides: a folding disk answers for `songs`, a case-sensitive one has no such folder.
    let folds = std::fs::metadata(format!("{dir}/songs")).is_ok();
    assert_eq!(on_disk(&dir, "songs").as_deref(), folds.then_some("Songs"));
}

#[test]
fn a_case_sensitive_disk_resolves_nothing_it_does_not_hold() {
    let held = ["Songs".to_string(), "songs".to_string(), "ZED".to_string()];
    assert_eq!(spelled_among(&held, "songs"), "songs");
    assert_eq!(spelled_among(&held, "Songs"), "Songs");
    assert_eq!(spelled_among(&held, "zed"), "ZED");
    assert_eq!(spelled_among(&held, "other"), "other");
    assert_eq!(spelled_among(&[], "x"), "x");
}

#[test]
fn project_dirs_are_walked_at_any_depth_in_code_point_order() {
    let a = Acct::new("ws-names-walk");
    let root = dirs(
        &a,
        "ws",
        &[
            "z.k7",
            "a.k7/songs/inner.k7",
            "group/b.k7",
            "group/deep/c.k7",
            "plain/notes",
            "\u{ff5e}.k7",
            "\u{1f600}.k7",
            "node_modules/x.k7",
            ".hid/y.k7",
            "_recycle_bin/gone.k7",
            "_history/h.k7",
            "peaks.cache/p.k7",
        ],
    );
    assert_eq!(
        under(&root).unwrap(),
        [
            "a.k7",
            "a.k7/songs/inner.k7",
            "group/b.k7",
            "group/deep/c.k7",
            "z.k7",
            "\u{ff5e}.k7",
            "\u{1f600}.k7"
        ]
    );
}

#[test]
fn a_project_suffix_alone_is_no_project_and_a_file_is_not_a_folder() {
    let a = Acct::new("ws-names-suffix");
    let root = dirs(&a, "ws", &[".k7", "x.v1"]);
    a.s.file(&format!("{}/ws/file.k7", support::ws::WORKSPACES), "x");
    assert_eq!(under(&root).unwrap(), Vec::<String>::new());
    assert!(!is_project_dir(&format!("{root}/.k7")));
    assert!(!is_project_dir(&format!("{root}/file.k7")));
}

#[test]
fn links_are_not_entered_and_a_loop_is_never_followed() {
    let a = Acct::new("ws-names-loops");
    if !can_link(&a.s) {
        return;
    }
    let root = dirs(&a, "ws", &["real.k7/inner", "elsewhere/x.k7"]);
    link(
        &format!("{root}/elsewhere"),
        &format!("{root}/real.k7/inner/loop.k7"),
    )
    .unwrap();
    link(&root, &format!("{root}/elsewhere/up")).unwrap();
    let mut seen = 0;
    let got = project_dirs_under(&root, &Limits::default(), &mut seen).unwrap();
    assert_eq!(got, ["elsewhere/x.k7", "real.k7"]);
    // Every entry the walk met is counted, and no link brought it back around.
    assert!(seen < 20, "{seen}");
}

#[cfg(unix)]
#[test]
fn an_unreadable_folder_costs_that_folder_only() {
    use std::os::unix::fs::PermissionsExt;
    if support::is_root() {
        return;
    }
    let a = Acct::new("ws-names-unreadable");
    let root = dirs(&a, "ws", &["locked/inner.k7", "open/ok.k7"]);
    let locked = format!("{root}/locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let got = under(&root);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(got.unwrap(), ["open/ok.k7"]);
}

#[test]
fn a_backslash_in_a_name_reads_as_a_separator_as_the_shim_joins_it() {
    let a = Acct::new("ws-names-backslash");
    let root = dirs(&a, "ws", &["plain.k7"]);
    // On a host that allows it: the dirent `x\y.k7` is spelled `x/y.k7` by the shim, which names nothing there.
    if std::fs::create_dir(format!("{root}/x\\y.k7")).is_err() {
        return;
    }
    assert_eq!(under(&root).unwrap(), ["plain.k7"]);
}

#[test]
fn the_walk_is_bounded_by_entries_and_by_depth() {
    let a = Acct::new("ws-names-bounds");
    let root = a.ws("ws");
    for i in 0..60 {
        a.s.dir(&format!("{}/ws/d{i:02}", support::ws::WORKSPACES));
    }
    let few = Limits {
        max_seen: 50,
        ..Limits::default()
    };
    assert_eq!(
        project_dirs_under(&root, &few, &mut 0),
        Err(Declined::TooMany)
    );
    let mut nest = String::from("n");
    for _ in 0..30 {
        nest.push_str("/n");
    }
    a.s.dir(&format!("{}/ws2/{nest}", support::ws::WORKSPACES));
    let shallow = Limits {
        max_depth: 12,
        ..Limits::default()
    };
    let r2 = a.s.path(&format!("{}/ws2", support::ws::WORKSPACES));
    assert_eq!(
        project_dirs_under(&r2, &shallow, &mut 0),
        Err(Declined::TooDeep)
    );
    assert!(under(&r2).unwrap().is_empty());
}

#[test]
fn a_drive_home_lists_as_the_shim_does() {
    let h = Home {
        studio: "C:/Work/studio".into(),
        libraries: "C:/Work/libs".into(),
    };
    assert_eq!(workspace_dir(&h), "C:/Work/studio/workspace");
    let h = Home::of(&[("USERPROFILE".to_string(), "C:\\Users\\me".to_string())].into());
    assert_eq!(workspace_dir(&h), "C:/Users/me/.k7/workspace");
    // The model has no disk to ask there: a drive that is not on this host lists nothing, and does not fail.
    assert!(folder_names(&workspace_dir(&h)).is_empty());
    assert!(under("C:/Users/me/.k7/workspace/x").unwrap().is_empty());
}
