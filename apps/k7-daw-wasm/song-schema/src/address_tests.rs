// address_tests.rs — the project address's grammar, case by case; the cases are the old TypeScript tests' inputs (projects.test.ts).

use super::*;

fn launch(path: &str) -> Option<(String, String)> {
    parse_launch(path, "/").map(|l| (l.repo, l.project))
}

#[test]
fn a_name_claims_the_suffix_only_at_its_end_and_never_bare() {
    assert!(is_project("mysong.k7"));
    assert!(is_project("prise v1.k7"));
    // `.v1` and `.m9` are shapes inside a `.k7`, not folder kinds.
    for name in ["le clip.v1", "prise v1.v1", "myset.m9"] {
        assert!(!is_project(name), "{name}");
    }
    for name in ["mysong", "songs", "mysong.k7.bak", "k7", "m9"] {
        assert!(!is_project(name), "{name}");
    }
    assert!(!is_project(".k7"));
    assert!(!is_project("prise v1"));
    assert!(!is_project("show.pl8"));
    assert!(!is_project("mysong.K7"));
}

#[test]
fn a_label_is_the_name_without_the_suffix() {
    assert_eq!(project_label("mysong.k7"), "mysong");
    assert_eq!(project_label("live set.k7"), "live set");
    assert_eq!(project_label("mysong"), "mysong");
    assert_eq!(project_label("v1.2.k7"), "v1.2");
}

#[test]
fn a_project_path_encodes_each_segment() {
    assert_eq!(launch_path("band", "winter.k7"), "/band/winter.k7");
    assert_eq!(launch_path("band", "le clip.k7"), "/band/le%20clip.k7");
}

#[test]
fn a_surface_hangs_off_the_project_and_names_it() {
    let at = external_of(&launch_path("band", "winter.k7"), "desk");
    assert_eq!(at, "/band/winter.k7/@external/desk");
    // The project parses as it did without the surface below it.
    assert_eq!(
        launch(&at),
        Some(("band".to_owned(), "winter.k7".to_owned()))
    );
    assert_eq!(project_basename(&at, "/"), "/band/winter.k7");
}

#[test]
fn a_surface_is_read_back_by_name_not_by_position() {
    let at = |p: &str| parse_external(p, "/");
    assert_eq!(at("/band/winter.k7/@external/desk").as_deref(), Some("desk"));
    assert_eq!(
        at("/band/songs/winter.k7/@external/desk").as_deref(),
        Some("desk")
    );
    assert_eq!(at("/band/@external/desk").as_deref(), Some("desk"));
}

#[test]
fn the_editors_own_addresses_and_a_truncated_one_name_no_surface() {
    for p in [
        "/band/winter.k7",
        "/band/winter.k7/timeline",
        "/band/winter.k7/chains-builder/process/normalize",
        "/band/winter.k7/@external",
        "/band/winter.k7/@external/",
    ] {
        assert_eq!(parse_external(p, "/"), None, "{p}");
    }
}

#[test]
fn a_surface_name_that_needs_encoding_round_trips_and_the_at_stays_literal() {
    let at = external_of(&launch_path("band", "winter.k7"), "le desk");
    assert!(at.contains("/@external/le%20desk"));
    assert_eq!(parse_external(&at, "/").as_deref(), Some("le desk"));
    let plain = external_of(&launch_path("band", "w.k7"), "desk");
    assert!(plain.contains(&format!("/{EXTERNAL_SEGMENT}/")));
    assert!(!plain.contains("%40"));
}

#[test]
fn an_address_outside_the_mount_names_no_surface() {
    assert_eq!(parse_external("/pl8/band/w.k7/@external/desk", "/sub/"), None);
}

#[test]
fn a_drop_carries_a_self_contained_project_and_nothing_else() {
    assert!(is_droppable_project("Winter Sketch.dawproject"));
    assert!(is_droppable_project("WINTER.DAWPROJECT"));
    // A `.als` names its samples by path, so dropped alone it would import a silent project.
    for name in ["winter.als", "winter.wav", "dawproject"] {
        assert!(!is_droppable_project(name), "{name}");
    }
}

#[test]
fn the_bench_is_one_project_and_a_library_chain_in_front_of_it_is_the_only_extra() {
    assert_eq!(bench_chain_of(BENCH_PROJECT), Some(None));
    assert_eq!(bench_chain_of("chains/bass/amp.k7"), None);
    assert_eq!(bench_chain_of("modules.k7"), None);
    assert_eq!(bench_chain_of("live"), None);
    let seeded = bench_project(Some("vocals/warm"));
    assert_eq!(bench_chain_of(&seeded), Some(Some("vocals/warm".to_owned())));
    for bad in [
        format!("../x/{BENCH_PROJECT}"),
        format!("a//b/{BENCH_PROJECT}"),
        format!("song.k7/{BENCH_PROJECT}"),
        format!("/{BENCH_PROJECT}"),
    ] {
        assert_eq!(bench_chain_of(&bad), None, "{bad}");
    }
}

#[test]
fn the_benchs_address_survives_the_round_trip_the_home_page_makes_it_take() {
    let at = launch_path(BENCH_REPO, &bench_project(Some("vocals/warm")));
    let back = parse_launch(&at, "/").unwrap();
    assert_eq!(
        bench_chain_of(&back.project),
        Some(Some("vocals/warm".to_owned()))
    );
    let bare = parse_launch(&launch_path(BENCH_REPO, BENCH_PROJECT), "/").unwrap();
    assert_eq!(bare.repo, BENCH_REPO);
    assert!(bench_chain_of(&bare.project).is_some());
}

#[test]
fn a_local_project_ends_by_the_ordinary_rule_and_splits_into_workspace_and_project() {
    assert_eq!(
        launch("/@local/Examples/synth-demo.k7"),
        Some(("@local".to_owned(), "Examples/synth-demo.k7".to_owned()))
    );
    let target = |p: &str| local_target(p).map(|t| (t.workspace, t.project));
    assert_eq!(
        target("Examples/synth-demo.k7"),
        Some(("Examples".to_owned(), "synth-demo.k7".to_owned()))
    );
    // A workspace that IS a project: `.` is what parse_launch answers when a repository is one.
    assert_eq!(
        target("hiver.k7"),
        Some(("hiver.k7".to_owned(), ".".to_owned()))
    );
    assert_eq!(
        target("Examples/albums/hiver.k7"),
        Some(("Examples".to_owned(), "albums/hiver.k7".to_owned()))
    );
}

#[test]
fn a_workspace_name_that_is_not_one_is_refused() {
    for bad in ["../elsewhere/x.k7", "./x.k7", ""] {
        assert_eq!(local_target(bad), None, "{bad:?}");
    }
}

#[test]
fn a_local_address_round_trips_its_own_encoding() {
    let at = local_project_path("mes projets", "hiver.k7");
    assert_eq!(at, "/@local/mes%20projets/hiver.k7");
    let back = parse_launch(&at, "/").unwrap();
    let t = local_target(&back.project).unwrap();
    assert_eq!(
        (t.workspace.as_str(), t.project.as_str()),
        ("mes projets", "hiver.k7")
    );
    // A single-project workspace names itself once.
    let whole = local_project_path("hiver.k7", ".");
    assert_eq!(whole, "/@local/hiver.k7");
    let t = local_target(&parse_launch(&whole, "/").unwrap().project).unwrap();
    assert_eq!((t.workspace.as_str(), t.project.as_str()), ("hiver.k7", "."));
}

#[test]
fn the_sentinels_begin_with_an_at_and_differ() {
    for sentinel in [LOCAL_REPO, BENCH_REPO, LAN_REPO] {
        assert!(sentinel.starts_with('@'));
    }
    assert_ne!(LOCAL_REPO, BENCH_REPO);
}

#[test]
fn the_homes_pages_are_found_at_the_mount_and_below() {
    for (p, base) in [
        ("/libraries", "/"),
        ("/settings/", "/"),
        ("/tasks", "/"),
        ("/about", "/"),
        ("/k7/settings", "/k7/"),
        ("/settings/engine", "/"),
        ("/k7/settings/engine", "/k7/"),
        ("/settings/x.k7", "/"),
        ("/libraries/anything", "/"),
        ("/media", "/"),
        ("/media/songs", "/"),
        ("/media/songs/Miles/So-What/song.yaml", "/"),
        ("/k7/media/find", "/k7/"),
        ("/surfaces", "/"),
    ] {
        assert!(is_home_page(p, base), "{p}");
    }
    // Only the head: a workspace may hold a project of that name.
    for p in [
        "/external",
        "/",
        "/@local/settings/x.k7",
        "/@local/ws/settings",
        "/songs/media",
        "/surfaces2",
    ] {
        assert!(!is_home_page(p, "/"), "{p}");
    }
}

#[test]
fn a_surface_of_another_device_round_trips_nested_or_whole() {
    let path = lan_surface_path("e-9f", "band", "mix.k7", "desk");
    assert_eq!(path, format!("/{LAN_REPO}/e-9f/band/mix.k7/@external/desk"));
    let read = |p: &str| parse_lan_surface(p, "/").map(|s| (s.engine_id, s.workspace, s.project, s.surface));
    let s = |a: &str, b: &str, c: &str, d: &str| Some((a.into(), b.into(), c.into(), d.into()));
    assert_eq!(read(&path), s("e-9f", "band", "mix.k7", "desk"));
    let nested = lan_surface_path("e-9f", "band", "songs/mix.k7", "a b");
    assert_eq!(read(&nested), s("e-9f", "band", "songs/mix.k7", "a b"));
    let whole = lan_surface_path("e", "mix.k7", ".", "d");
    let got = read(&whole).unwrap();
    assert_eq!((got.1.as_str(), got.3.as_str()), ("mix.k7", "d"));
}

#[test]
fn an_address_that_is_no_lan_surface_reads_as_none() {
    for p in [
        "/@lan/e/band/mix.k7",
        "/@lan/e/band/mix.k7/@external",
        "/@local/band/mix.k7/@external/desk",
        "/@lan/e/@external/desk",
    ] {
        assert_eq!(parse_lan_surface(p, "/"), None, "{p}");
    }
}
