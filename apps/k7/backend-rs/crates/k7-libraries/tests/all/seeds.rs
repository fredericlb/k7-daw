//! seeds.rs — an app update's newer seed is added as one more version once, and the husks of a runaway are swept.

use std::fs;
use std::path::Path;

use k7_libraries::Store;

use crate::support::Root;

const MIX_V1: &str = "plugin(\"v1\")\n";

fn seed_in(root: &Root) -> String {
    let seed = root.0.join("seed");
    fs::create_dir_all(seed.join("k7-core/modules")).unwrap();
    fs::create_dir_all(seed.join("k7-core/samples")).unwrap();
    fs::write(seed.join("k7-core/modules/mix.k7x"), MIX_V1).unwrap();
    fs::write(seed.join("k7-core/samples/.gitkeep"), "").unwrap();
    seed.to_string_lossy().into_owned()
}

/// The repo's k7-core moves to version 2, as an app update's bundle does.
fn seed_declares_2(seed: &str) {
    let at = Path::new(seed).join("k7-core");
    fs::write(at.join("modules/mix.k7x"), "plugin(\"v2\")\n").unwrap();
    fs::write(
        at.join("library.k7x"),
        "name(\"k7-core\")\nversion(2)\nrequires(1)\nnotes(\"fixed\")\n",
    )
    .unwrap();
}

fn folders(store: &Store) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(store.home.join("k7-core"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn store(root: &Root, seed: &str) -> Store {
    Store::new(&root.0.join("data"), Some(seed))
}

#[test]
fn a_machine_adds_the_seed_once_however_often_it_is_ensured() {
    let root = Root::new();
    let seed = seed_in(&root);
    let store = store(&root, &seed);
    store.ensure(None, false);
    seed_declares_2(&seed);
    for _ in 0..3 {
        store.ensure(None, false);
    }
    assert_eq!(folders(&store), ["1", "2"]);
    assert!(store.home.join("k7-core/2/library.k7x").exists());
}

/// A machine that ran a looping hub: versions 2..6 copied, then undone to their folders and `.gitkeep`.
fn runaway(root: &Root, seed: &str) {
    let before = store(root, seed);
    before.ensure(None, false);
    for v in 2..=6 {
        let at = before.home.join(format!("k7-core/{v}"));
        fs::create_dir_all(at.join("samples")).unwrap();
        fs::create_dir_all(at.join("modules/fx")).unwrap();
        fs::write(at.join("samples/.gitkeep"), "").unwrap();
    }
}

#[test]
fn husks_are_swept_where_the_seed_takes_its_own_number() {
    let root = Root::new();
    let seed = seed_in(&root);
    runaway(&root, &seed);
    seed_declares_2(&seed);
    let store = store(&root, &seed);
    let notes = store.ensure(None, false);
    assert_eq!(notes[0].state, "seeded");
    assert!(
        notes[0].note.starts_with("version 2 added "),
        "{}",
        notes[0].note
    );
    assert!(
        notes[0].note.contains("5 empty version"),
        "{}",
        notes[0].note
    );
    assert_eq!(folders(&store), ["1", "2"]);
    let versions: Vec<u64> = store
        .versions("k7-core")
        .iter()
        .map(|m| m.version)
        .collect();
    assert_eq!(versions, [1, 2]);
}

#[test]
fn a_version_holding_a_file_is_never_swept() {
    let root = Root::new();
    let seed = seed_in(&root);
    runaway(&root, &seed);
    let at = root.0.join("data/libraries/k7-core");
    fs::write(at.join("3/modules/fx/kept.k7x"), "x(1)\n").unwrap();
    let store = store(&root, &seed);
    store.ensure(None, false);
    assert_eq!(folders(&store), ["1", "3"]);
}

#[test]
fn a_machine_with_only_the_seed_lists_it_and_holds_no_other_version() {
    let root = Root::new();
    let seed = seed_in(&root);
    let store = store(&root, &seed);
    let notes = store.ensure(None, false);
    assert_eq!(notes[0].state, "seeded", "{}", notes[0].note);
    let view = store.view(&notes);
    let listed: Vec<&str> = view["libraries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["k7-core"]);
    assert_eq!(folders(&store), ["1"]);
}
