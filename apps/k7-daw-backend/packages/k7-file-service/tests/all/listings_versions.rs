//! The versions (1430): what each library plays, what it needs, and the newest this k7 could move to, here or at the
//! origin; the pins and manifests in YAML are declined; the constants equal the TypeScript's.

use crate::support;

use k7_file_service::libraries::Version;
use k7_file_service::listings::levels::module_libraries;
use k7_file_service::listings::versions::{
    LIBRARY_LEVEL, Remote, UNPINNED_VERSION, VersionRow, versions_at,
};
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use serde_json::Value;
use support::Scratch;

fn manifest(version: u32, requires: u32) -> String {
    format!("name(\"lib\")\nversion({version})\nrequires({requires})\n")
}

fn rows(
    s: &Scratch,
    remote: &Remote,
) -> Result<std::collections::HashMap<String, VersionRow>, Declined> {
    versions_at(&s.home(), &s.path("ws"), remote, &Limits::default())
}

fn remote(name: &str, versions: &[(u32, u32)]) -> Remote {
    let mut r = Remote::default();
    r.by_name.insert(name.into(), versions.to_vec());
    r
}

#[test]
fn a_pinned_version_says_what_it_requires_and_what_is_newer() {
    let s = Scratch::new("ver-pinned");
    s.file("ws/_library-pins.k7x", "k7-core(2)\n");
    for (v, req) in [(1, 1), (2, 1), (3, 1), (4, LIBRARY_LEVEL + 1)] {
        s.file(
            &format!("libraries/k7-core/{v}/library.k7x"),
            &manifest(v, req),
        );
    }
    let got = rows(&s, &Remote::default()).unwrap();
    // Version 4 asks for a level this k7 lacks, so 3 is the newest it runs.
    assert_eq!(
        got["k7-core"],
        VersionRow {
            pinned: Version::Number(2),
            requires: Some(1),
            upgrade: Some(3),
            used: None
        }
    );
}

#[test]
fn a_draft_pin_has_no_requirement_and_no_upgrade() {
    let s = Scratch::new("ver-draft");
    s.file("ws/_library-pins.k7x", "k7-core(\"draft\")\n");
    s.file("libraries/k7-core/1/library.k7x", &manifest(1, 1));
    s.file("libraries/k7-core/9/library.k7x", &manifest(9, 1));
    let got = rows(&s, &remote("k7-core", &[(12, 1)])).unwrap();
    assert_eq!(
        got["k7-core"],
        VersionRow {
            pinned: Version::Draft,
            requires: None,
            upgrade: None,
            used: None
        }
    );
}

#[test]
fn an_unpinned_library_and_one_in_a_single_folder_are_version_one() {
    let s = Scratch::new("ver-bare");
    s.file("libraries/bare/modules/audio/b/module.k7x", "");
    s.file("libraries/versioned/1/library.k7x", &manifest(1, 1));
    s.file("libraries/versioned/2/library.k7x", &manifest(2, 1));
    let got = rows(&s, &Remote::default()).unwrap();
    assert_eq!(got["bare"].pinned, Version::Number(1));
    assert_eq!((got["bare"].requires, got["bare"].upgrade), (None, None));
    assert_eq!(got["versioned"].upgrade, Some(2));
}

#[test]
fn the_origins_versions_count_when_they_run_here() {
    let s = Scratch::new("ver-remote");
    s.file("libraries/k7-core/1/library.k7x", &manifest(1, 1));
    // 5 asks for a level this k7 lacks; 4 does not.
    let got = rows(&s, &remote("k7-core", &[(4, 1), (5, LIBRARY_LEVEL + 1)])).unwrap();
    assert_eq!(got["k7-core"].upgrade, Some(4));
    let got = rows(&s, &remote("k7-core", &[(5, LIBRARY_LEVEL + 1)])).unwrap();
    assert_eq!(got["k7-core"].upgrade, None);
}

#[test]
fn a_manifest_that_parses_badly_is_none_but_one_beside_yaml_is_declined() {
    let s = Scratch::new("ver-manifest");
    s.file("ws/_library-pins.k7x", "garbled(2)\n");
    s.file("libraries/garbled/1/library.k7x", "name(");
    s.file("libraries/garbled/2/library.k7x", &manifest(2, 1));
    // No manifest at all, and none that names a version, are a version that asks for nothing.
    s.file("libraries/garbled/3/library.k7x", "name(\"x\")\n");
    let got = rows(&s, &Remote::default()).unwrap();
    assert_eq!(got["garbled"].upgrade, Some(3));
}

#[test]
fn a_levels_row_carries_its_version_in_the_literals_order() {
    let s = Scratch::new("ver-row");
    s.dir("ws");
    s.file("ws/_library-pins.k7x", "k7-core(1)\n");
    s.file("libraries/k7-core/1/library.k7x", &manifest(1, 1));
    s.file("libraries/k7-core/2/library.k7x", &manifest(2, 1));
    let h = s.home();
    let l = module_libraries(
        &Space::open(&h, &s.path("ws")),
        &Remote::default(),
        &Limits::default(),
    )
    .unwrap();
    assert!(
        l.json
            .contains(r#""version":{"pinned":1,"requires":1,"upgrade":2}}"#),
        "{}",
        l.json
    );
    let v: Value = serde_json::from_str(&l.json).unwrap();
    assert_eq!(v["libraries"][0]["ns"], "k7-core");
}

#[test]
fn the_constants_are_the_song_crates() {
    assert_eq!(LIBRARY_LEVEL, song_edit::shared_folders::LIBRARY_LEVEL);
    assert_eq!(
        UNPINNED_VERSION.to_string(),
        song_edit::shared_folders::UNPINNED_VERSION
    );
}
