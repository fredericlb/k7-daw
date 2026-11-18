//! The modules and the links of a copy (1652, 1653, 1654): the `@workspace` modules a project names and those they name,
//! the escape a module's text tried, links followed as content, and what a copy that fails midway leaves.

use crate::support;

use k7_file_service::space::Declined;
use k7_file_service::tree::Limits;
use k7_file_service::workspaces::WsError;
use k7_file_service::workspaces::copy::*;
use support::wscopy::*;
use support::{can_link, link, link_file};

fn read(a: &support::ws::Acct, ws: &str, rel: &str) -> String {
    std::fs::read_to_string(a.path(ws, rel)).unwrap()
}

fn has(a: &support::ws::Acct, ws: &str, rel: &str) -> bool {
    std::fs::symlink_metadata(a.path(ws, rel)).is_ok()
}

#[test]
fn the_modules_a_project_names_come_along_and_the_ones_they_name() {
    let a = fx("ws-copy-modules");
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/melody.k7x",
        "id(\"a\").extends(\"@workspace/instruments/synth\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/instruments/synth/module.k7x",
        "extends(\"@workspace/fx/shine.k7x\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/fx/shine.k7x",
        "kind(\"fx\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/other/unrelated.k7x",
        "kind(\"x\")\n",
    );
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert_eq!(
        p.modules,
        ["_modules/instruments/synth", "_modules/fx/shine.k7x"]
    );
    assert_eq!(
        answer_of(&p),
        r#"{"project":"winter.k7","modules":["_modules/instruments/synth","_modules/fx/shine.k7x"]}"#
    );
    assert!(has(&a, "albums", "_modules/instruments/synth/module.k7x"));
    assert!(!has(&a, "albums", "_modules/other"));
    // 1654: the project first, the modules last.
    let to = |e: &Entry| match e {
        Entry::Folder(p) | Entry::File { to: p, .. } => p.clone(),
    };
    assert!(to(&p.entries[0]).ends_with("/albums/winter.k7"));
    assert!(to(p.entries.last().unwrap()).ends_with("/_modules/fx/shine.k7x"));
}

#[test]
fn a_module_the_destination_has_is_left_alone() {
    let a = fx("ws-copy-modules-have");
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/melody.k7x",
        "extends(\"@workspace/fx/shine\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/fx/shine.k7x",
        "kind(\"fx\")\n",
    );
    a.s.file(
        "studio/workspace/albums/_modules/fx/shine.k7x",
        "kind(\"mine\")\n",
    );
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert!(p.modules.is_empty());
    assert_eq!(
        read(&a, "albums", "_modules/fx/shine.k7x"),
        "kind(\"mine\")\n"
    );
}

#[test]
fn a_module_files_refs_stay_inside_modules_1690() {
    let a = fx("ws-copy-escape");
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/melody.k7x",
        "extends(\"@workspace/m\")\nextends(\"@workspace/n\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/m.k7x",
        "extends(\"@workspace/a/../../other.k7/notes.txt\")\n",
    );
    // A `..` that stays inside `_modules` is still no ref: the filter is on the segment, not on where it lands.
    a.s.file(
        "studio/workspace/songs/_modules/n.k7x",
        "extends(\"@workspace/a/../b\")\n",
    );
    a.s.file("studio/workspace/songs/_modules/b.k7x", "kind(\"b\")\n");
    a.s.file("studio/workspace/songs/other.k7/notes.txt", "private\n");
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert_eq!(p.modules, ["_modules/m.k7x", "_modules/n.k7x"]);
    assert!(!has(&a, "albums", "_modules/b.k7x"));
    assert!(!has(&a, "albums", "other.k7"));
    assert!(p.entries.iter().all(|e| match e {
        Entry::File { to, .. } | Entry::Folder(to) =>
            to.contains("/winter.k7") || to.contains("/_modules"),
    }));
}

#[test]
fn a_project_files_ref_climbing_out_is_dropped() {
    let a = fx("ws-copy-climb");
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/c.k7x",
        "extends(\"@workspace/../other.k7/x\")\nextends(\"@workspace/x/../y\")\n",
    );
    a.s.file("studio/workspace/songs/other.k7/x", "x");
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert!(p.modules.is_empty());
}

#[test]
fn a_module_naming_itself_or_a_long_chain_ends() {
    let a = fx("ws-copy-chain");
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/c.k7x",
        "extends(\"@workspace/m0\")\n",
    );
    for i in 0..1000 {
        a.s.file(
            &format!("studio/workspace/songs/_modules/m{i}.k7x"),
            &format!("extends(\"@workspace/m{}\")\n", i + 1),
        );
    }
    a.s.file(
        "studio/workspace/songs/_modules/m1000.k7x",
        "extends(\"@workspace/m1000\")\n",
    );
    let p = plan(&a, &ask("songs", "winter.k7", "albums", "winter.k7")).unwrap();
    assert_eq!(p.modules.len(), 1001);
    assert_eq!(p.modules[0], "_modules/m0.k7x");
    assert_eq!(p.modules[1000], "_modules/m1000.k7x");
}

#[test]
fn a_module_that_is_only_a_folder_and_a_file_of_the_same_stem_prefers_the_folder() {
    let a = fx("ws-copy-stem");
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/c.k7x",
        "extends(\"@workspace/dup\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/dup/module.k7x",
        "kind(\"folder\")\n",
    );
    a.s.file(
        "studio/workspace/songs/_modules/dup.k7x",
        "kind(\"file\")\n",
    );
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert_eq!(p.modules, ["_modules/dup"]);
    assert!(!has(&a, "albums", "_modules/dup.k7x"));
}

#[test]
fn links_inside_the_project_are_copied_as_content() {
    let a = fx("ws-copy-links");
    if !can_link(&a.s) {
        return;
    }
    let outside = a.s.dir("outside");
    a.s.file("outside/f.txt", "inside the link");
    link(&outside, &a.path("songs", "winter.k7/linked")).unwrap();
    let file = a.s.file("target.txt", "a file link");
    link_file(&file, &a.path("songs", "winter.k7/filelink.txt")).unwrap();
    copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert_eq!(
        read(&a, "albums", "winter.k7/linked/f.txt"),
        "inside the link"
    );
    assert_eq!(read(&a, "albums", "winter.k7/filelink.txt"), "a file link");
    assert!(
        !std::fs::symlink_metadata(a.path("albums", "winter.k7/linked"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn a_link_loop_is_declined_before_a_byte_is_written() {
    let a = fx("ws-copy-loop");
    if !can_link(&a.s) {
        return;
    }
    link(
        &a.path("songs", "winter.k7"),
        &a.path("songs", "winter.k7/modules/up"),
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    // The host stops a path through ~32 links first (ELOOP, `EIO` to the shim); the depth bound stops what it does not.
    let r = plan(&a, &ask("songs", "winter.k7", "albums", "w.k7")).err();
    assert!(
        matches!(
            r,
            Some(
                WsError::Declined(Declined::TooDeep)
                    | WsError::Io {
                        syscall: "stat",
                        ..
                    }
            )
        ),
        "{r:?}"
    );
    assert!(!has(&a, "albums", "w.k7"));
    let bound = if cfg!(target_env = "musl") { 60 } else { 15 };
    assert!(t0.elapsed() < std::time::Duration::from_secs(bound));
}

#[test]
fn a_real_nest_past_the_depth_bound_is_declined_before_a_byte_is_written() {
    let a = fx("ws-copy-deep-nest");
    std::fs::create_dir_all(a.path("songs", &format!("winter.k7/{}", vec!["n"; 40].join("/"))))
        .unwrap();
    let few = Limits {
        max_depth: 16,
        ..Limits::default()
    };
    let c = ask("songs", "winter.k7", "albums", "w.k7");
    assert_eq!(
        plan_copy(&a.home(), false, &c, &few).err(),
        Some(WsError::Declined(Declined::TooDeep))
    );
    assert!(!has(&a, "albums", "w.k7"));
}

#[test]
fn a_dangling_link_fails_the_plan_with_the_code_the_copy_would_fail_with() {
    let a = fx("ws-copy-dangling");
    if !can_link(&a.s) {
        return;
    }
    link_file(
        &a.s.path("nowhere.txt"),
        &a.path("songs", "winter.k7/dangling.txt"),
    )
    .unwrap();
    match plan(&a, &ask("songs", "winter.k7", "albums", "w.k7")) {
        Err(WsError::Io { code, .. }) => assert_eq!(code, "ENOENT"),
        other => panic!("{other:?}"),
    }
    assert!(!has(&a, "albums", "w.k7"));
}

#[test]
fn a_destination_that_appears_between_the_plan_and_the_copy_is_written_into() {
    let a = fx("ws-copy-race");
    let p = plan(&a, &ask("songs", "winter.k7", "albums", "w.k7")).unwrap();
    a.s.file("studio/workspace/albums/w.k7/mine.txt", "kept");
    a.s.file("studio/workspace/albums/w.k7/project.k7x", "overwritten");
    apply_copy(&p).unwrap();
    assert_eq!(read(&a, "albums", "w.k7/mine.txt"), "kept");
    assert_eq!(
        read(&a, "albums", "w.k7/project.k7x"),
        "project({ name: \"winter\" })\n"
    );
}

#[test]
fn a_source_removed_during_the_copy_answers_the_error_and_leaves_the_part_written() {
    let a = fx("ws-copy-gone");
    let p = plan(&a, &ask("songs", "winter.k7", "albums", "w.k7")).unwrap();
    std::fs::remove_file(a.path("songs", "winter.k7/project.k7x")).unwrap();
    match apply_copy(&p) {
        Err(WsError::Io { code, syscall, .. }) => {
            assert_eq!((code, syscall), ("ENOENT", "copyfile"))
        }
        other => panic!("{other:?}"),
    }
    assert!(
        has(&a, "albums", "w.k7/modules/bass.k7x"),
        "what was written stays"
    );
}

#[test]
fn the_entries_are_counted_before_anything_is_written() {
    let a = fx("ws-copy-limits");
    for i in 0..200 {
        a.s.file(
            &format!("studio/workspace/songs/winter.k7/many/f{i}.txt"),
            "x",
        );
    }
    let few = Limits {
        max_seen: 100,
        ..Limits::default()
    };
    let c = ask("songs", "winter.k7", "albums", "w.k7");
    assert_eq!(
        plan_copy(&a.home(), false, &c, &few).err(),
        Some(WsError::Declined(Declined::TooMany))
    );
    assert!(!has(&a, "albums", "w.k7"));
    a.s.file(
        "studio/workspace/songs/winter.k7/chains/c.k7x",
        &"x".repeat(2000),
    );
    let small = Limits {
        max_text: 1000,
        ..Limits::default()
    };
    assert_eq!(
        plan_copy(&a.home(), false, &c, &small).err(),
        Some(WsError::Declined(Declined::TooLarge))
    );
}

#[test]
fn the_names_a_vector_cannot_reach_are_held_here() {
    assert_eq!(clean_name("  ..a/b\\c.k7 "), "a_b_c.k7");
    assert_eq!(clean_name("\u{feff}.x\u{a0}"), "x");
    assert!(
        is_doc_file("Fredk7x")
            && is_doc_file("a.K7X")
            && !is_doc_file("\nk7x")
            && !is_doc_file("k7x")
    );
    assert_eq!(
        playlist_path_of(" /a/b/ ").as_deref(),
        Some("_playlists/a/b.k7x")
    );
    assert_eq!(playlist_path_of("a/../b"), None);
}

#[cfg(unix)]
#[test]
fn a_backslash_name_folding_out_of_its_folder_is_not_copied() {
    let a = fx("ws-copy-backslash");
    a.s.file("studio/workspace/songs/keep.txt", "precious");
    // Read as `/` by the shim, this name is `songs/keep.txt` on both ends: a copy onto itself truncates it.
    let same = format!(
        "{}/a\\..\\..\\..\\songs\\keep.txt",
        a.path("songs", "winter.k7")
    );
    std::fs::write(&same, "x").unwrap();
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "w.k7"));
    assert!(
        p.entries
            .iter()
            .all(|e| !format!("{e:?}").contains("keep.txt")),
        "{p:?}"
    );
    assert_eq!(read(&a, "songs", "keep.txt"), "precious");
    let b = fx("ws-copy-backslash-out");
    b.project("songs", "deep/w2.k7", "project({})\n");
    b.s.file("studio/workspace/escaped.txt", "from the workspaces folder");
    let out = format!(
        "{}/a\\..\\..\\..\\..\\escaped.txt",
        b.path("songs", "deep/w2.k7")
    );
    std::fs::write(&out, "x").unwrap();
    copy(&b, &ask("songs", "deep/w2.k7", "albums", "x.k7"));
    assert!(std::fs::metadata(b.s.path("studio/escaped.txt")).is_err());
}
