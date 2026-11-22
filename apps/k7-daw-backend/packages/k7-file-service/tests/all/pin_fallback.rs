//! A workspace pinning a library version this machine lacks: the compile, the listings and the presets read the newest
//! version held, the module listing says so, and the pins file is left as it was. A held pin reads exactly as it says.

use std::collections::BTreeMap;

use crate::support::Scratch;
use k7_file_service::confine::Disk;
use k7_file_service::libraries::{Version, library_root, played_version};
use k7_file_service::listings::families::account_libraries;
use k7_file_service::listings::levels::module_libraries;
use k7_file_service::listings::versions::{Remote, VersionRow, versions_at};
use k7_file_service::song_ask::{Input, song_roots};
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;

const PINS: &str = "studio/workspace/ws/_library-pins.k7x";

/// A device holding k7-core 2 and 8, and a workspace pinning `pins`.
fn device(name: &str, pins: &str) -> Scratch {
    let s = Scratch::new(name);
    for v in [2, 8] {
        s.file(
            &format!("libraries/k7-core/{v}/library.k7x"),
            &format!("name(\"k7-core\")\nversion({v})\nrequires(1)\n"),
        );
        s.file(
            &format!("libraries/k7-core/{v}/modules/audio/gain/module.k7x"),
            "",
        );
    }
    s.file(PINS, pins);
    s.dir("studio/workspace/ws/song");
    s
}

fn compiled_root(s: &Scratch) -> String {
    let project = s.path("studio/workspace/ws/song");
    let env = BTreeMap::new();
    let input = Input {
        home: &s.home(),
        project: &project,
        code_root: "/code",
        env: &env,
    };
    let roots = song_roots(&input, &Disk).unwrap();
    roots
        .libraries
        .iter()
        .find(|l| l.name == "k7-core")
        .unwrap()
        .root
        .clone()
}

fn row(s: &Scratch) -> VersionRow {
    let rows = versions_at(
        &s.home(),
        &s.path("studio/workspace/ws"),
        &Remote::default(),
        &Limits::default(),
    )
    .unwrap();
    rows["k7-core"].clone()
}

#[test]
fn a_missing_pin_compiles_against_the_newest_held_and_says_so() {
    let s = device("fallback-missing", "k7-core(1)\n");
    assert_eq!(compiled_root(&s), s.path("libraries/k7-core/8"));
    let from = s.path("studio/workspace/ws/song");
    assert_eq!(
        library_root(&s.home(), "k7-core", Some(&from)).unwrap(),
        s.path("libraries/k7-core/8")
    );
    let libs = account_libraries(&s.home(), Some(&from)).unwrap();
    assert_eq!(libs[0].root, s.path("libraries/k7-core/8"));
    assert_eq!(
        played_version(&s.home(), "k7-core", Version::Number(1)),
        Version::Number(8)
    );
    assert_eq!(
        row(&s),
        VersionRow {
            pinned: Version::Number(1),
            requires: Some(1),
            upgrade: None,
            used: Some(8)
        }
    );
    let h = s.home();
    let listed = module_libraries(
        &Space::open(&h, &s.path("studio/workspace/ws")),
        &Remote::default(),
        &Limits::default(),
    )
    .unwrap();
    assert!(
        listed
            .json
            .contains(r#""version":{"pinned":1,"requires":1,"used":8}"#),
        "{}",
        listed.json
    );
    // Nothing above rewrote the pin: moving it is the person's change.
    assert_eq!(
        std::fs::read_to_string(s.path(PINS)).unwrap(),
        "k7-core(1)\n"
    );
}

#[test]
fn a_held_pin_and_a_draft_pin_read_as_today() {
    let s = device("fallback-held", "k7-core(2)\n");
    assert_eq!(compiled_root(&s), s.path("libraries/k7-core/2"));
    assert_eq!(
        row(&s),
        VersionRow {
            pinned: Version::Number(2),
            requires: Some(1),
            upgrade: Some(8),
            used: None
        }
    );
    s.dir("libraries/k7-core/draft");
    s.file(PINS, "k7-core(\"draft\")\n");
    assert_eq!(compiled_root(&s), s.path("libraries/k7-core/draft"));
    assert_eq!(row(&s).used, None);
}
