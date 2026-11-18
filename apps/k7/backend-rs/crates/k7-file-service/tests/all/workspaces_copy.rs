//! A project copied between workspaces (1651 to 1654): every guard's sentence in the TypeScript's order, what lands
//! and what is left out, the playlists and modules it carries, links followed, and the escape a module's text tried.

use crate::support;

use k7_file_service::workspaces::WsError;
use k7_file_service::workspaces::copy::*;
use support::ws::TURNTABLES;
use support::wscopy::*;

fn read(a: &support::ws::Acct, ws: &str, rel: &str) -> String {
    std::fs::read_to_string(a.path(ws, rel)).unwrap()
}

fn has(a: &support::ws::Acct, ws: &str, rel: &str) -> bool {
    std::fs::symlink_metadata(a.path(ws, rel)).is_ok()
}

#[test]
fn a_project_lands_with_its_content_and_nothing_that_is_not() {
    let a = fx("ws-copy-lands");
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert_eq!(answer_of(&p), r#"{"project":"winter.k7"}"#);
    assert_eq!(
        read(&a, "albums", "winter.k7/project.k7x"),
        "project({ name: \"winter\" })\n"
    );
    assert_eq!(
        read(&a, "albums", "winter.k7/modules/bass.k7x"),
        "kind(\"sampler\")\n"
    );
    for gone in ["kick.wav.cache", "node_modules", ".hidden"] {
        assert!(!has(&a, "albums", &format!("winter.k7/{gone}")), "{gone}");
    }
    assert!(
        has(&a, "songs", "winter.k7/kick.wav.cache"),
        "the source is untouched"
    );
}

#[test]
fn the_plan_writes_nothing_and_lists_the_entries_in_the_order_cp_writes_them() {
    let a = fx("ws-copy-plan");
    let p = plan(&a, &ask("songs", "winter.k7", "albums", "w.k7")).unwrap();
    assert!(!has(&a, "albums", "w.k7"));
    let dest = a.path("albums", "w.k7");
    assert_eq!(p.entries[0], Entry::Folder(dest.clone()));
    let names: Vec<_> = p
        .entries
        .iter()
        .map(|e| match e {
            Entry::Folder(p) => format!("D {}", &p[dest.len()..]),
            Entry::File { to, .. } => format!("F {}", &to[dest.len()..]),
        })
        .collect();
    assert_eq!(
        names,
        ["D ", "D /modules", "F /modules/bass.k7x", "F /project.k7x"]
    );
    assert_eq!(p.project, "w.k7");
}

#[test]
fn a_project_at_depth_two_and_a_rename_keep_their_suffix() {
    let a = fx("ws-copy-deep");
    a.project("songs", "group/inner/deep.k7", "project({})\n");
    let p = copy(
        &a,
        &ask("songs", "group/inner/deep.k7", "albums", "  ..spring.k7 "),
    );
    assert_eq!(p.project, "spring.k7");
    assert!(has(&a, "albums", "spring.k7/project.k7x"));
    assert_eq!(
        copy(&a, &ask("songs", "winter.k7", "songs", "copy.k7")).project,
        "copy.k7"
    );
}

#[test]
fn every_refusal_has_the_typescripts_sentence_in_its_order() {
    let a = fx("ws-copy-refusals");
    a.s.dir("studio/workspace/songs/notes");
    a.s.dir("studio/workspace/songs/_recycle_bin/gone.k7");
    a.project("solo.k7", ".", "project({})\n");
    a.ws("Examples");
    a.project("albums", "taken.k7", "project({})\n");
    let cases: Vec<(support::wscopy::CopyAskOwned, &str)> = vec![
        (
            ("..", "winter.k7", "albums", "w.k7"),
            "invalid workspace name: ..",
        ),
        (
            ("songs", "winter.k7", "a/b", "w.k7"),
            "invalid workspace name: a/b",
        ),
        (
            ("nothing", "winter.k7", "albums", "w.k7"),
            "unknown workspace: nothing",
        ),
        (
            ("songs", "winter.k7", "nothing", "w.k7"),
            "unknown workspace: nothing",
        ),
        (
            ("songs", "../x.k7", "albums", "w.k7"),
            "path escapes the workspace: ../x.k7",
        ),
        (
            ("songs", "_recycle_bin/gone.k7", "albums", "w.k7"),
            "project is in the recycle bin: _recycle_bin/gone.k7",
        ),
        (
            ("songs", "notes", "albums", "w.k7"),
            "not a project (the folder must end in .k7): notes",
        ),
        (
            ("songs", "missing.k7", "albums", "w.k7"),
            "not a project (the folder must end in .k7): missing.k7",
        ),
        (
            ("songs", "winter.k7", "solo.k7", "w.k7"),
            "solo.k7 is itself a project — copy into a workspace that holds several",
        ),
        (
            ("songs", "winter.k7", "Examples", "w.k7"),
            "Examples holds the bundled demos and takes no new project — make it in a workspace of your own",
        ),
        (
            ("songs", "winter.k7", "albums", "  "),
            "project name required",
        ),
        (
            ("songs", "winter.k7", "albums", "..."),
            "project name required",
        ),
        (
            ("songs", "winter.k7", "albums", "w"),
            "a copy of winter.k7 must keep its suffix: \"w\" does not",
        ),
        (
            ("songs", "winter.k7", "albums", "w.v1"),
            "a copy of winter.k7 must keep its suffix: \"w.v1\" does not",
        ),
        (
            ("songs", "winter.k7", "albums", "a/b"),
            "a copy of winter.k7 must keep its suffix: \"a_b\" does not",
        ),
        (
            ("songs", "winter.k7", "albums", "taken.k7"),
            "\"taken.k7\" already exists",
        ),
    ];
    for ((from, path, to, name), want) in cases {
        assert_eq!(
            refusal(&a, &ask(from, path, to, name)),
            want,
            "{from} {path} {to} {name}"
        );
    }
}

#[test]
fn the_guards_run_in_order_a_bad_source_name_before_a_missing_destination() {
    let a = fx("ws-copy-order");
    // Source end first: its name, its presence; then the destination's; then the project, then the destination's own rules.
    assert_eq!(
        refusal(&a, &ask("..", "x", "nothing", "w")),
        "invalid workspace name: .."
    );
    assert_eq!(
        refusal(&a, &ask("nothing", "x", "..", "w")),
        "unknown workspace: nothing"
    );
    assert_eq!(
        refusal(&a, &ask("songs", "notes", "nothing", "w")),
        "unknown workspace: nothing"
    );
}

#[test]
fn a_name_the_other_case_of_a_workspace_is_resolved_to_its_folder() {
    let a = fx("ws-copy-case");
    let folds = std::fs::metadata(a.s.path("studio/workspace/ALBUMS")).is_ok();
    let r = plan(&a, &ask("SONGS", "winter.k7", "ALBUMS", "w.k7"));
    if folds {
        let p = r.unwrap();
        assert!(
            p.dest.ends_with("/albums") && p.src.ends_with("/songs"),
            "{p:?}"
        );
    } else {
        assert_eq!(
            r.err(),
            Some(WsError::Unknown {
                name: "SONGS".into()
            })
        );
    }
}

#[test]
fn the_playlists_a_surface_names_come_along_unless_the_destination_has_them() {
    let a = fx("ws-copy-playlists");
    deck(&a);
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "winter.k7"));
    assert_eq!(p.playlists, ["_playlists/Fred.k7x"]);
    assert_eq!(
        answer_of(&p),
        r#"{"project":"winter.k7","playlists":["_playlists/Fred.k7x"]}"#
    );
    assert_eq!(
        read(&a, "albums", "_playlists/Fred.k7x"),
        "items([{ song: \"a\" }])\n"
    );
    // The destination's own set list wins; the second copy carries none.
    a.s.file(
        "studio/workspace/albums/_playlists/Fred.k7x",
        "items([{ song: \"b\" }])\n",
    );
    let again = copy(&a, &ask("songs", "winter.k7", "albums", "second.k7"));
    assert!(again.playlists.is_empty());
    assert_eq!(answer_of(&again), r#"{"project":"second.k7"}"#);
    assert!(read(&a, "albums", "_playlists/Fred.k7x").contains("\"b\""));
}

#[test]
fn a_playlist_the_source_lacks_or_a_name_that_leaves_the_folder_is_skipped() {
    let a = fx("ws-copy-playlists-skip");
    a.surface("songs", "winter.k7", "decks", TURNTABLES);
    assert!(
        copy(&a, &ask("songs", "winter.k7", "albums", "a.k7"))
            .playlists
            .is_empty()
    );
    a.surface(
        "songs",
        "winter.k7",
        "decks",
        "type(\"turntables\")\nlayout(\"split\")\nplaylist(\"../escape\")\n",
    );
    a.s.file("studio/workspace/songs/escape.k7x", "x");
    assert!(
        copy(&a, &ask("songs", "winter.k7", "albums", "b.k7"))
            .playlists
            .is_empty()
    );
    a.surface(
        "songs",
        "winter.k7",
        "decks",
        "type(\"turntables\")\nlayout(\"split\")\nplaylist(\"a\\\\..\\\\..\\\\..\\\\escape\")\n",
    );
    a.s.file("studio/workspace/songs/_playlists/a/x.k7x", "x");
    assert!(
        copy(&a, &ask("songs", "winter.k7", "albums", "c.k7"))
            .playlists
            .is_empty()
    );
    assert!(std::fs::metadata(a.s.path("studio/workspace/escape.k7x")).is_err());
    // One level up stays in the workspace on both ends, so nothing but the guard keeps another project's file out.
    a.surface(
        "songs",
        "winter.k7",
        "decks",
        "type(\"turntables\")\nlayout(\"split\")\nplaylist(\"..\\\\other.k7\\\\project\")\n",
    );
    a.project("songs", "other.k7", "project({})\n");
    assert!(
        copy(&a, &ask("songs", "winter.k7", "albums", "d.k7"))
            .playlists
            .is_empty()
    );
    assert!(std::fs::metadata(a.path("albums", "other.k7")).is_err());
}

#[test]
fn two_surfaces_naming_one_playlist_carry_it_once() {
    let a = fx("ws-copy-playlists-once");
    deck(&a);
    a.surface("songs", "winter.k7", "decks2", TURNTABLES);
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "w.k7"));
    assert_eq!(p.playlists, ["_playlists/Fred.k7x"]);
}

#[test]
fn two_surfaces_naming_one_playlist_by_two_cases_carry_what_the_disk_says() {
    let a = fx("ws-copy-playlists-case");
    deck(&a);
    a.surface(
        "songs",
        "winter.k7",
        "decks2",
        "type(\"turntables\")\nlayout(\"split\")\nplaylist(\"fred\")\n",
    );
    let p = copy(&a, &ask("songs", "winter.k7", "albums", "w.k7"));
    // `existsSync` after the first copy: a disk that folds case holds `fred.k7x` then; one that does not has none to copy.
    assert_eq!(p.playlists, ["_playlists/Fred.k7x"]);
}
