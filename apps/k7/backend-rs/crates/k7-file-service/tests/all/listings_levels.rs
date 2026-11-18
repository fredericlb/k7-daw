//! The module levels (1420 to 1424): the order, a library's sound families, one lookup a row (1422), a folder that
//! cannot be listed (1423), the engine's row from its scan (1424), and a drive root spelt on any host.

use crate::support;

use k7_file_service::libraries::{Home, Library, library_of_calls};
use k7_file_service::listings::levels::module_roots;
use k7_file_service::listings::{Listed, Listing, Remote, list};
use k7_file_service::rel::to_rel;
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use serde_json::Value;
use support::{Scratch, can_link, link, link_file};

fn run(s: &Scratch, limits: &Limits) -> Result<Listed, Declined> {
    s.dir("ws");
    let h = s.home(false);
    list(
        &Space::open(&h, &s.path("ws")),
        Listing::ModuleLibraries,
        &Remote::default(),
        limits,
    )
}

fn levels(s: &Scratch) -> Vec<Value> {
    let l = run(s, &Limits::default()).unwrap();
    let mut v = serde_json::from_str::<Value>(&l.json).unwrap()["libraries"]
        .as_array()
        .unwrap()
        .clone();
    // The engine's row is the scan's, and a machine with no scan lists the built-ins.
    assert_eq!(v.pop().unwrap()["ns"], "engine");
    v
}

fn ns(rows: &[Value]) -> Vec<&str> {
    rows.iter().map(|r| r["ns"].as_str().unwrap()).collect()
}

#[test]
fn the_workspace_comes_first_then_the_account_then_each_library_and_a_library_with_no_modules_is_a_row()
 {
    let s = Scratch::new("lv-order");
    s.file("_modules/audio/own/module.k7x", "");
    s.file("studio/library/modules/audio/mine/module.k7x", "");
    s.file("libraries/zed/modules/synth/z/module.k7x", "");
    s.file("libraries/k7-core/modules/audio/eq/module.k7x", "");
    s.dir("libraries/empty");
    let rows = levels(&s);
    // 1421: the shared folder, the family roots in order, then every library not yet a row.
    assert_eq!(
        ns(&rows),
        ["workspace", "account", "k7-core", "zed", "empty"]
    );
    assert_eq!(rows[0]["path"], "../_modules");
    assert_eq!(rows[4]["exists"], false);
    assert_eq!(rows[4]["nodes"], serde_json::json!([]));
}

#[test]
fn only_a_library_of_the_account_gains_its_sound_families() {
    let s = Scratch::new("lv-sounds");
    s.file("_modules/audio/own/module.k7x", "");
    s.file("studio/library/modules/audio/mine/module.k7x", "");
    s.file("libraries/k7-core/modules/audio/eq/module.k7x", "");
    s.file("libraries/k7-core/samples/drums/snare.wav", "");
    s.file("libraries/k7-core/irs/hall.wav", "");
    let rows = levels(&s);
    // 1422: not the workspace's row, not the account's.
    for r in &rows[..2] {
        for family in ["samples", "irs", "wavetables", "models"] {
            assert!(r.get(family).is_none(), "{} {family}", r["ns"]);
        }
    }
    let lib = &rows[2];
    let keys: Vec<&str> = lib
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "ns",
            "path",
            "exists",
            "nodes",
            "samples",
            "irs",
            "wavetables",
            "models",
            "version"
        ]
    );
    assert_eq!(
        lib["samples"][0]["path"],
        "_libraries/k7-core/samples/drums"
    );
    assert_eq!(lib["wavetables"], serde_json::json!([]));
}

#[test]
fn a_levels_paths_are_spelt_by_one_lookup_a_row_never_one_an_entry() {
    let s = Scratch::new("lv-calls");
    for i in 0..300 {
        s.file(
            &format!("libraries/k7-core/modules/audio/m{i}/module.k7x"),
            "",
        );
    }
    s.dir("ws");
    // The counter is this thread's: the tests running beside this one cannot move it.
    let before = library_of_calls();
    let rows = levels(&s);
    let calls = library_of_calls() - before;
    assert!(rows[0]["nodes"][0]["children"].as_array().unwrap().len() == 300);
    // 600 entries were written; a handful of rows and families is what asked.
    assert!(calls < 60, "{calls} lookups");
}

#[test]
fn a_folder_that_cannot_be_listed_costs_that_folder_only() {
    // Unix only: a mode keeps a folder from being read, and root ignores it; Windows reads every folder.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if support::is_root() {
            return;
        }
        let s = Scratch::new("lv-unlistable");
        s.file("_modules/audio/ok/module.k7x", "");
        let locked = s.dir("_modules/audio/locked");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let l = run(&s, &Limits::default()).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        // 1423: the TypeScript throws on it, and the row stands with that folder empty.
        assert_eq!(l.unlisted, std::slice::from_ref(&locked));
        let v: Value = serde_json::from_str(&l.json).unwrap();
        let children = &v["libraries"][0]["nodes"][0]["children"];
        assert_eq!(children[0]["name"], "locked");
        assert_eq!(children[0]["children"], serde_json::json!([]));
    }
}

#[test]
fn the_engines_row_comes_from_its_scan_grouped_and_sorted() {
    let s = Scratch::new("lv-engine");
    s.file(
        "studio/exts.json",
        r#"{"entries":[{"plugins":[
            {"name":"Serum","format":"VST3","role":"instrument","instrument":true},
            {"name":"Valhalla Room","format":"VST3","role":"audioEffect"},
            {"name":"Valhalla Room","format":"AU","role":"audioEffect"},
            {"name":"Midi Thing","format":"VST3","role":"midiEffect"},
            {"name":"!!!","format":"VST3"}]}]}"#,
    );
    let l = run(&s, &Limits::default()).unwrap();
    assert!(l.json.ends_with(
        concat!(
            r#"{"ns":"engine","path":"","exists":true,"readOnly":true,"nodes":["#,
            r#"{"name":"audio","path":"@engine/audio","dir":true,"children":[{"name":"valhalla-room","path":"@engine/valhalla-room","dir":false,"kind":"text"}]},"#,
            r#"{"name":"instruments","path":"@engine/instruments","dir":true,"children":[{"name":"serum","path":"@engine/serum","dir":false,"kind":"text"}]},"#,
            r#"{"name":"transformers","path":"@engine/transformers","dir":true,"children":[{"name":"midi-thing","path":"@engine/midi-thing","dir":false,"kind":"text"}]}]}]}"#
        )
    ), "{}", l.json);
}

#[test]
fn a_scan_that_cannot_say_lists_the_built_ins() {
    let s = Scratch::new("lv-builtin");
    let l = run(&s, &Limits::default()).unwrap();
    let v: Value = serde_json::from_str(&l.json).unwrap();
    assert_eq!(v["libraries"][0]["ns"], "engine");
    assert!(v["libraries"][0]["nodes"].as_array().unwrap().len() >= 2);
}

#[test]
fn a_scan_the_song_crate_would_read_short_is_declined() {
    // 1424: a link as the scan is read by the TypeScript and refused by the song crate's source.
    let s = Scratch::new("lv-exts-link");
    s.file(
        "elsewhere.json",
        r#"{"entries":[{"plugins":[{"name":"Serum","format":"VST3"}]}]}"#,
    );
    s.dir("studio");
    if link_file(&s.path("elsewhere.json"), &s.path("studio/exts.json")).is_ok() {
        assert_eq!(
            run(&s, &Limits::default()).unwrap_err(),
            Declined::NotRegular
        );
    }
    // Past the song crate's cap a scan reads empty, where the TypeScript lists every plugin.
    let s = Scratch::new("lv-exts-cap");
    let plugins: Vec<String> = (0..=song::modules::SCAN_CAP)
        .map(|i| format!(r#"{{"name":"P{i}","format":"VST3"}}"#))
        .collect();
    s.file(
        "studio/exts.json",
        &format!(r#"{{"entries":[{{"plugins":[{}]}}]}}"#, plugins.join(",")),
    );
    assert_eq!(run(&s, &Limits::default()).unwrap_err(), Declined::TooMany);
}

#[test]
fn the_open_projects_marker_and_a_library_one_are_declined() {
    // 1420: a workspace still holding the marker it once kept inside itself is declined.
    let s = Scratch::new("lv-marker");
    s.file("ws/.studio-active", "song.k7");
    assert_eq!(
        run(&s, &Limits::default()).unwrap_err(),
        Declined::LegacyMarker
    );
    let s = Scratch::new("lv-library-active");
    s.dir("ws");
    s.file(".active-projects/ws", "_libraries/k7-core");
    assert_eq!(
        run(&s, &Limits::default()).unwrap_err(),
        Declined::LibraryActive
    );
}

#[test]
fn the_limits_weigh_the_whole_listing_not_each_row() {
    let s = Scratch::new("lv-limits");
    for lib in ["k7-core", "zed"] {
        for i in 0..30 {
            s.file(
                &format!("libraries/{lib}/modules/audio/m{i}/module.k7x"),
                "",
            );
        }
    }
    // Each row walks 61 entries; one `seen` runs across both.
    let one_row = Limits {
        max_seen: 100,
        ..Limits::default()
    };
    assert_eq!(run(&s, &one_row).unwrap_err(), Declined::TooMany);
    let roomy = Limits {
        max_seen: 130,
        ..Limits::default()
    };
    assert!(run(&s, &roomy).is_ok());
    let bytes = Limits {
        max_bytes: 3000,
        ..Limits::default()
    };
    assert_eq!(run(&s, &bytes).unwrap_err(), Declined::TooLarge);
    // The song crate parses a tree one level deeper than `/tree` (1443).
    let l = run(&s, &Limits::default()).unwrap();
    assert_eq!(l.depth, 2 + 2);
}

#[test]
fn a_link_among_the_levels_is_a_leaf_and_the_row_is_not_followed_through_it() {
    let s = Scratch::new("lv-link");
    if !can_link(&s) {
        return;
    }
    s.file("elsewhere/e.txt", "");
    s.file("_modules/audio/m/module.k7x", "");
    link(&s.path("elsewhere"), &s.path("_modules/audio/alias")).unwrap();
    let rows = levels(&s);
    let kids = rows[0]["nodes"][0]["children"].as_array().unwrap();
    assert_eq!(
        kids.iter()
            .map(|k| k["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["m", "alias"]
    );
    assert_eq!(kids[1]["dir"], false);
}

#[test]
fn a_drive_root_spells_every_level() {
    // Windows is data: nothing here exists on this host, and every spelling is lexical.
    let home = Home {
        studio: "C:/Users/a/.k7".into(),
        libraries: "C:/Users/a/.k7/libraries".into(),
        admin: false,
    };
    let space = Space::open(&home, "C:/Users/a/.k7/workspace/songs");
    let libs = [Library {
        name: "k7-core".into(),
        root: "C:/Users/a/.k7/libraries/k7-core".into(),
    }];
    let roots = module_roots(&home, space.root(), &libs);
    assert_eq!(
        roots,
        [(
            "k7-core".to_string(),
            "C:/Users/a/.k7/libraries/k7-core/modules".to_string()
        )]
    );
    let spelt: Vec<String> = [
        "C:/Users/a/.k7/libraries/k7-core/modules",
        "C:/Users/a/.k7/library/modules/audio/x",
        "C:/Users/a/.k7/workspace/_modules",
        "C:/Users/a/.k7/workspace/songs/_playlists/a.k7x",
        "C:/Users/a/.k7/workspace/songs",
    ]
    .iter()
    .map(|p| to_rel(&space, p))
    .collect();
    assert_eq!(
        spelt,
        [
            "_libraries/k7-core/modules",
            "_libraries/account/modules/audio/x",
            "../_modules",
            "_playlists/a.k7x",
            "."
        ]
    );
}
