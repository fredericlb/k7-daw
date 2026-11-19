//! The libraries' roots (1030 to 1035): the names held to the TypeScript's, the pins walk and its refusals, the
//! order `libraryRoot` decides in, and a library found without a search over libraries.

use crate::support;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use k7_file_service::libraries::*;
use k7_file_service::space::Declined;
use support::Scratch;

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn ts(rel: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../../../../{rel}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn knows_the_names_and_versions_the_typescript_does() {
    let types = ts("packages/song-schema/src/moduleTypes.ts");
    assert!(
        types.contains(&format!("const LIBRARY_NAME = /{NAME_PATTERN}/;")),
        "the name rule"
    );
    assert!(types.contains("FIXED_NAMESPACES = [\"engine\", \"workspace\"]"));
    assert!(types.contains("export const ACCOUNT_LIBRARY = \"account\";"));
    assert!(
        types.contains("\"studio\", // an older name of @meta")
            && types.contains("\"mediatek\", // an older name of the default library")
    );
    assert!(
        ts("packages/song-schema/src/metaRefs.ts").contains("export const META_NS = \"@meta\";")
    );
    let reserved: Vec<&str> = RESERVED.to_vec();
    assert_eq!(
        reserved,
        [
            "engine",
            "workspace",
            "account",
            "meta",
            "studio",
            "mediatek"
        ]
    );
    let versions = ts("packages/song-schema/src/libraryVersions.ts");
    assert!(
        versions.contains(&format!("const VERSION_DIR = /{VERSION_PATTERN}/;")),
        "the version rule"
    );
    assert!(
        versions.contains("DRAFT_VERSION = \"draft\"")
            && versions.contains("LIBRARY_PINS_FILE = \"_library-pins.k7x\"")
    );
}

#[test]
fn a_home_is_the_accounts_environment_spelt_with_slashes() {
    let env = |kv: &[(&str, &str)]| {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>()
    };
    let h = Home::of(&env(&[("STUDIO_HOME", "/data/k7/"), ("HOME", "/h")]), true);
    assert_eq!(
        (h.studio.as_str(), h.libraries.as_str(), h.admin),
        ("/data/k7", "/data/k7/libraries", true)
    );
    let h = Home::of(
        &env(&[("HOME", "/home/me"), ("STUDIO_LIBRARIES_HOME", "/hub/libs")]),
        false,
    );
    assert_eq!(
        (h.studio.as_str(), h.libraries.as_str()),
        ("/home/me/.k7", "/hub/libs")
    );
    let h = Home::of(&env(&[("STUDIO_HOME", "C:\\Users\\me\\k7")]), false);
    assert_eq!(
        (h.studio.as_str(), h.libraries.as_str()),
        ("C:/Users/me/k7", "C:/Users/me/k7/libraries")
    );
    assert_eq!(Home::of(&env(&[]), false).studio, "/.k7");
}

#[test]
fn the_nearest_pins_file_above_wins_and_a_bad_one_stops_the_walk() {
    let s = Scratch::new("pins");
    s.file(
        "a/_library-pins.k7x",
        "k7-core(3)\nmine(\"draft\")\nzero(0)\nhalf(1.5)\nflag(true)\nname(\"x\")\n",
    );
    s.dir("a/b/c");
    let pins = workspace_pins(&s.path("a/b/c")).unwrap();
    assert_eq!(pins.get("k7-core"), Some(&Version::Number(3)));
    assert_eq!(pins.get("mine"), Some(&Version::Draft));
    for dropped in ["zero", "half", "flag", "name", "constructor"] {
        assert_eq!(pins.get(dropped), None, "{dropped}");
    }
    // Unparseable pins are no pins, and the walk goes no further up.
    s.file("a/b/_library-pins.k7x", "k7-core(\n");
    assert!(workspace_pins(&s.path("a/b/c")).unwrap().is_empty());
    // The walk ends at the disk's root, which holds no pins file.
    assert!(workspace_pins("/").unwrap().is_empty());
    assert!(
        workspace_pins(&s.path("nowhere/at/all"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_legacy_file_and_a_lossy_text_are_the_typescripts_alone() {
    let s = Scratch::new("pins-legacy");
    s.file("w/_library-pins.yaml", "k7-core(2)\n");
    s.dir("w/p");
    assert_eq!(workspace_pins(&s.path("w/p")), Err(Declined::LegacyPins));
    // Found first, whatever stands higher.
    s.file("_library-pins.k7x", "k7-core(5)\n");
    assert_eq!(workspace_pins(&s.path("w/p")), Err(Declined::LegacyPins));
    // The k7x beside the legacy file wins: it is the upgraded one.
    s.file("w/_library-pins.k7x", "k7-core(4)\n");
    assert_eq!(
        workspace_pins(&s.path("w/p")).unwrap().get("k7-core"),
        Some(&Version::Number(4))
    );
    s.file("lossy/_library-pins.k7x", "k7-core(\"\\ud800\")\n");
    assert_eq!(workspace_pins(&s.path("lossy")), Err(Declined::LossyText));
}

#[test]
fn a_big_pins_file_is_read_whole_and_fast() {
    let s = Scratch::new("pins-big");
    let text: String = (0..6_000)
        .map(|i| format!("lib-{i}({})\n", i % 9 + 1))
        .collect();
    assert!(text.len() > 60_000);
    s.file("w/_library-pins.k7x", &text);
    let t0 = Instant::now();
    let pins = workspace_pins(&s.path("w")).unwrap();
    assert_eq!(pins.len(), 6_000);
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_pin_names_own_keys_only() {
    // 1032: the TypeScript's `name in pins` is true for a prototype's key; here a name is pinned or it is not.
    let s = Scratch::new("pins-own");
    let h = s.home(false);
    s.dir("libraries/constructor");
    s.dir("w");
    assert_eq!(
        library_root(&h, "constructor", Some(&s.path("w"))).unwrap(),
        format!("{}/constructor", h.libraries)
    );
}

#[test]
fn a_library_root_is_decided_in_the_typescripts_order() {
    let s = Scratch::new("order");
    let (h, admin) = (s.home(false), s.home(true));
    let lib = |n: &str| format!("{}/{n}", h.libraries);
    s.dir("libraries/versioned/1");
    s.dir("libraries/versioned/2");
    s.dir("libraries/versioned/draft");
    s.dir("libraries/flat/modules");
    s.dir("studio/library");
    s.file(
        "w/_library-pins.k7x",
        "versioned(2)\nflat(7)\naccount(9)\ndrafted(\"draft\")\n",
    );
    s.dir("libraries/drafted/draft");
    s.dir("libraries/drafted/1");
    let from = s.path("w");
    // The account's folder first, whatever the pins say.
    assert_eq!(
        library_root(&h, "account", Some(&from)).unwrap(),
        format!("{}/library", h.studio)
    );
    // A pin, through the version folder; a flat library is every version of itself.
    assert_eq!(
        library_root(&h, "versioned", Some(&from)).unwrap(),
        format!("{}/2", lib("versioned"))
    );
    assert_eq!(library_root(&h, "flat", Some(&from)).unwrap(), lib("flat"));
    assert_eq!(
        library_root(&h, "drafted", Some(&from)).unwrap(),
        format!("{}/draft", lib("drafted"))
    );
    // No pin: version 1, or an admin's draft overlay.
    assert_eq!(
        library_root(&h, "versioned", None).unwrap(),
        format!("{}/1", lib("versioned"))
    );
    assert_eq!(
        library_root(&admin, "versioned", None).unwrap(),
        format!("{}/draft", lib("versioned"))
    );
    assert_eq!(library_root(&admin, "flat", None).unwrap(), lib("flat"));
    // The pin outranks the overlay.
    assert_eq!(
        library_root(&admin, "versioned", Some(&from)).unwrap(),
        format!("{}/2", lib("versioned"))
    );
    // Standing inside a version of the library, that version.
    s.dir("w2");
    let inside = format!("{}/1/modules", lib("versioned"));
    assert_eq!(
        library_root(&h, "versioned", Some(&inside)).unwrap(),
        format!("{}/1", lib("versioned"))
    );
}

#[test]
fn a_library_is_found_from_a_path_without_a_search() {
    let s = Scratch::new("of");
    let (h, admin) = (s.home(false), s.home(true));
    let lib = |n: &str| format!("{}/{n}", h.libraries);
    s.dir("libraries/versioned/1/x");
    s.dir("libraries/versioned/draft");
    s.dir("libraries/flat/x");
    s.dir("studio/library/y");
    let l = |h: &Home, p: &str| library_of(h, p).map(|l| (l.name, l.root));
    assert_eq!(
        l(&h, &format!("{}/y/z", h.studio.clone() + "/library")),
        Some(("account".into(), format!("{}/library", h.studio)))
    );
    assert_eq!(
        l(&h, &format!("{}/1/x/f", lib("versioned"))),
        Some(("versioned".into(), format!("{}/1", lib("versioned"))))
    );
    assert_eq!(
        l(&h, &lib("versioned")),
        Some(("versioned".into(), lib("versioned")))
    );
    assert_eq!(
        l(&h, &format!("{}/9/f", lib("versioned"))),
        Some(("versioned".into(), format!("{}/9", lib("versioned"))))
    );
    assert_eq!(
        l(&h, &format!("{}/zzz/f", lib("versioned"))),
        Some(("versioned".into(), lib("versioned")))
    );
    assert_eq!(
        l(&h, &format!("{}/x", lib("flat"))),
        Some(("flat".into(), lib("flat")))
    );
    // An admin's overlay answers for a path in the draft; the published version still answers to itself.
    assert_eq!(
        l(&admin, &format!("{}/draft/f", lib("versioned"))),
        Some(("versioned".into(), format!("{}/draft", lib("versioned"))))
    );
    assert_eq!(
        l(&admin, &format!("{}/1/x", lib("versioned"))),
        Some(("versioned".into(), format!("{}/1", lib("versioned"))))
    );
    for no in [
        h.libraries.clone(),
        "/elsewhere/x".into(),
        format!("{}/Bad_Name/x", h.libraries),
        format!("{}/meta/x", h.libraries),
    ] {
        assert_eq!(l(&h, &no), None, "{no}");
    }
}

#[test]
fn a_library_with_no_version_folders_and_one_with_two_thousand_cost_one_listing() {
    let s = Scratch::new("many");
    let h = s.home(false);
    s.dir("libraries/flat/modules");
    let many = s.dir("libraries/many");
    for i in 1..=2_000 {
        std::fs::create_dir(format!("{many}/{i}")).unwrap();
    }
    let t0 = Instant::now();
    assert_eq!(
        library_root(&h, "flat", None).unwrap(),
        format!("{}/flat", h.libraries)
    );
    assert_eq!(
        library_root(&h, "many", None).unwrap(),
        format!("{}/many/1", h.libraries)
    );
    for _ in 0..200 {
        library_of(&h, &format!("{}/many/77/x", h.libraries)).unwrap();
    }
    assert!(t0.elapsed() < BOUND, "{:?}", t0.elapsed());
}

#[test]
fn the_account_s_folder_answers_before_a_pin() {
    let s = Scratch::new("account-first");
    let h = s.home(false);
    s.file("w/_library-pins.k7x", "account(3)\n");
    assert_eq!(
        library_root(&h, "account", Some(&s.path("w"))).unwrap(),
        format!("{}/library", h.studio)
    );
    // …and a pins file the Rust reader refuses does not matter to the account's library.
    s.file("w2/_library-pins.yaml", "x(1)\n");
    assert_eq!(
        library_root(&h, "account", Some(&s.path("w2"))).unwrap(),
        format!("{}/library", h.studio)
    );
    assert_eq!(
        library_root(&h, "other", Some(&s.path("w2"))),
        Err(Declined::LegacyPins)
    );
}
