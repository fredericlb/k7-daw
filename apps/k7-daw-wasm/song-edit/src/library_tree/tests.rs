// The LIBRARIES tab's rows and path rules: the cases of the page's libraryTree tests, minus the people-order sort (the page's collator).

use super::*;

fn dir(name: &str, path: &str, children: Vec<Value>) -> Value {
    dir_row(name, path, children)
}

fn file(name: &str, path: &str) -> Value {
    json!({"name": name, "path": path, "dir": false, "kind": "text"})
}

/// `<at>/audio/<name>`: a module under its type folder, which is how every level files one.
fn audio(at: &str, name: &str) -> Value {
    dir(
        "audio",
        &format!("{at}/audio"),
        vec![dir(
            name,
            &format!("{at}/audio/{name}"),
            vec![file("module.k7x", &format!("{at}/audio/{name}/module.k7x"))],
        )],
    )
}

fn lib(ns: &str, path: &str, nodes: Vec<Value>) -> LibraryLevel {
    serde_json::from_value(json!({"ns": ns, "path": path, "exists": true, "nodes": nodes})).expect("a level")
}

/// What the file service answers for a project with both levels present.
fn all() -> Vec<LibraryLevel> {
    vec![
        lib("workspace", "_modules", vec![audio("_modules", "tape-delay")]),
        lib(
            "k7-core",
            "_libraries/k7-core/modules",
            vec![audio("_libraries/k7-core/modules", "reverb")],
        ),
    ]
}

/// `@engine` as the service answers it: no folder, read-only, synthesised type groups.
fn engine() -> LibraryLevel {
    serde_json::from_value(json!({
        "ns": "engine", "path": "", "exists": true, "readOnly": true,
        "nodes": [
            dir("audio", "@engine/audio", vec![file("bitcrush", "@engine/bitcrush"), file("nam", "@engine/nam")]),
            dir("instruments", "@engine/instruments", vec![file("serum", "@engine/serum")]),
        ]
    }))
    .expect("the engine level")
}

fn tree(levels: &[LibraryLevel]) -> Vec<Value> {
    library_tree(levels, &[], &[])
}

fn names(rows: &[Value]) -> Vec<&str> {
    rows.iter().filter_map(|r| r["name"].as_str()).collect()
}

fn children(row: &Value) -> &[Value] {
    row["children"].as_array().map_or(&[], Vec::as_slice)
}

#[test]
fn a_row_per_level_in_the_order_it_was_given() {
    // The service answers nearest-first and this does not re-sort it: two places deciding precedence is how they come to disagree.
    assert_eq!(names(&tree(&all())), ["@workspace", "@k7-core"]);
}

#[test]
fn each_row_points_at_the_real_folder() {
    let rows = tree(&all());
    assert_eq!(rows[0]["path"], "_modules");
    // `@k7-core` is the REPOSITORY, whose `modules/` is the level.
    assert_eq!(rows[1]["path"], "_libraries/k7-core");
    assert_eq!(children(&rows[1])[0]["path"], "_libraries/k7-core/modules");
}

#[test]
fn a_subtree_passes_through_unfolded_and_a_modules_path_is_kept() {
    let answer = all();
    let rows = tree(&answer);
    assert_eq!(children(&rows[0]), answer[0].nodes.as_slice());
    assert_eq!(children(&children(&rows[0])[0])[0]["path"], "_modules/audio/tape-delay");
}

#[test]
fn workspace_is_drawn_even_with_no_folder_yet_and_an_empty_level_too() {
    // A level you can put a module in is a real answer, and hiding it would leave nowhere to start on a new project.
    let fresh = [lib("workspace", "_modules", vec![])];
    assert_eq!(names(&tree(&fresh)), ["@workspace"]);
    assert!(children(&tree(&fresh)[0]).is_empty());
}

#[test]
fn a_library_with_no_modules_yet_is_drawn() {
    let no_modules = [
        lib("workspace", "_modules", vec![audio("_modules", "a")]),
        lib("k7-core", "_libraries/k7-core/modules", vec![]),
    ];
    assert_eq!(names(&tree(&no_modules)), ["@workspace", "@k7-core"]);
}

#[test]
fn every_library_the_account_has_is_its_own_repository_row() {
    let two = [
        lib("drums", "_libraries/drums/modules", vec![]),
        lib("k7-core", "_libraries/k7-core/modules", vec![]),
    ];
    let rows = tree(&two);
    let seen: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r["name"].as_str().unwrap_or(""), r["path"].as_str().unwrap_or("")))
        .collect();
    assert_eq!(
        seen,
        [("@drums", "_libraries/drums"), ("@k7-core", "_libraries/k7-core")]
    );
}

#[test]
fn engine_gets_a_row_though_it_has_no_folder_and_its_leaves_keep_their_refs() {
    let rows = tree(&[engine()]);
    assert_eq!(rows[0]["name"], "@engine");
    // Its own label stands in for the path: there is no folder to name it by.
    assert_eq!(rows[0]["path"], "@engine");
    let leaves: Vec<&str> = children(&children(&rows[0])[0])
        .iter()
        .filter_map(|n| n["path"].as_str())
        .collect();
    assert_eq!(leaves, ["@engine/bitcrush", "@engine/nam"]);
}

#[test]
fn the_engine_level_is_frozen_rows_and_all_and_nothing_of_a_writable_one() {
    let mut levels = vec![engine()];
    levels.extend(all());
    let frozen = read_only_paths(&levels);
    for p in ["@engine", "@engine/audio", "@engine/nam"] {
        assert!(frozen.iter().any(|f| f == p), "{p}");
    }
    assert!(
        !frozen
            .iter()
            .any(|f| f == "_modules" || f == "_libraries/k7-core/modules")
    );
}

fn root_ns(levels: &[LibraryLevel], path: &str) -> Option<String> {
    root_of(levels, path).map(|i| levels[i].ns.clone())
}

#[test]
fn a_row_finds_the_level_it_belongs_to_at_any_depth() {
    let l = all();
    assert_eq!(root_ns(&l, "_modules").as_deref(), Some("workspace"));
    assert_eq!(root_ns(&l, "_modules/audio").as_deref(), Some("workspace"));
    assert_eq!(
        root_ns(&l, "_libraries/k7-core/modules/audio/delay").as_deref(),
        Some("k7-core")
    );
}

#[test]
fn a_read_only_level_a_path_in_no_level_and_a_prefix_find_no_root() {
    let mut libs = vec![engine()];
    libs.extend(all());
    assert_eq!(root_of(&libs, "@engine"), None);
    assert_eq!(root_of(&libs, "@engine/audio"), None);
    assert_eq!(root_of(&all(), "song.k7/clips"), None);
    assert_eq!(root_of(&all(), "_modulesX/audio"), None);
}

#[test]
fn a_module_is_named_the_way_a_chain_does() {
    let r = |f: &str| library_module_ref(f);
    assert_eq!(
        r("_libraries/k7-core/modules/audio/reverb/module.k7x").as_deref(),
        Some("@k7-core/audio/reverb")
    );
    // A workspace module has no `modules/` inside it.
    assert_eq!(
        r("_modules/audio/tape-delay/module.k7x").as_deref(),
        Some("@workspace/audio/tape-delay")
    );
    assert_eq!(
        r("_libraries/k7-core/modules/audio/delay/tape-delay/ui.k7x").as_deref(),
        Some("@k7-core/audio/delay/tape-delay")
    );
    assert_eq!(
        r("_libraries/k7-core/modules/audio/tape.delay/module.k7x").as_deref(),
        Some("@k7-core/audio/tape.delay")
    );
    assert_eq!(
        r("_libraries/drums/modules/audio/kick/module.k7x").as_deref(),
        Some("@drums/audio/kick")
    );
}

#[test]
fn a_path_that_is_no_shared_module_has_no_ref() {
    let r = |f: &str| library_module_ref(f);
    assert_eq!(r("song.k7/nodes/lead.k7x"), None);
    assert_eq!(r("_libraries/k7-core/chains/live/vocals/graph.k7x"), None);
}

#[test]
fn the_library_is_read_off_a_libraries_path() {
    assert_eq!(
        library_of_path("_libraries/k7-core/modules/audio/x/module.k7x").as_deref(),
        Some("k7-core")
    );
    assert_eq!(
        library_of_path("_libraries/drums/chains/kit/graph.k7x").as_deref(),
        Some("drums")
    );
    assert_eq!(library_of_path("_modules/audio/x/module.k7x"), None);
    assert_eq!(library_of_path("_libraries/engine/modules/x.k7x"), None);
    assert_eq!(library_of_path("_libraries"), None);
}

#[test]
fn only_the_accounts_library_is_a_library_path_and_a_whole_segment_at_that() {
    // `_modules` is shared too, but a save aimed at it forks into the workspace.
    assert!(is_library_path("_libraries/k7-core/modules/audio/reverb/module.k7x"));
    assert!(!is_library_path("_modules/audio/reverb/module.k7x"));
    assert!(!is_library_path("song.k7/nodes/lead.k7x"));
    assert!(!is_library_path("_librariesX/modules/a/module.k7x"));
}

fn chain(r: &str, path: &str, nodes: Option<Vec<&str>>) -> LibraryChainRow {
    LibraryChainRow {
        ref_: r.into(),
        path: path.into(),
        nodes: nodes.map(|n| n.into_iter().map(str::to_owned).collect()),
    }
}

fn levels() -> Vec<LibraryLevel> {
    vec![lib("k7-core", "_libraries/k7-core/modules", vec![])]
}

// Already in the order people read them: the page sorts, the rows keep it.
fn chains() -> Vec<LibraryChainRow> {
    vec![
        chain(
            "@k7-core/live/vocals",
            "_libraries/k7-core/chains/live/vocals",
            Some(vec![]),
        ),
        chain(
            "@k7-core/process/normalize",
            "_libraries/k7-core/chains/process/normalize",
            Some(vec!["normalize.k7x"]),
        ),
    ]
}

fn assemblies() -> Vec<LibraryAssemblyRow> {
    vec![LibraryAssemblyRow {
        ref_: "@k7-core/band/four-piece".into(),
        path: "_libraries/k7-core/assemblies/band/four-piece".into(),
        file: "_libraries/k7-core/assemblies/band/four-piece/assembly.k7x".into(),
    }]
}

fn repo() -> Value {
    library_tree(&levels(), &chains(), &assemblies()).remove(0)
}

#[test]
fn the_repository_is_one_row_with_its_families_under_it_file_folders_included() {
    assert_eq!(library_tree(&levels(), &chains(), &assemblies()).len(), 1);
    let repo = repo();
    assert_eq!(
        (repo["name"].as_str(), repo["path"].as_str()),
        (Some("@k7-core"), Some("_libraries/k7-core"))
    );
    let seen: Vec<(&str, &str)> = children(&repo)
        .iter()
        .map(|c| (c["name"].as_str().unwrap_or(""), c["path"].as_str().unwrap_or("")))
        .collect();
    let want = [
        "modules",
        "chains",
        "assemblies",
        "samples",
        "irs",
        "wavetables",
        "models",
    ];
    let own = |n: &str| {
        if n == "modules" {
            "_libraries/k7-core/modules".to_owned()
        } else {
            format!("_libraries/k7-core/{n}")
        }
    };
    assert_eq!(seen.iter().map(|s| s.0).collect::<Vec<_>>(), want);
    for (name, path) in seen {
        assert_eq!(path, own(name));
    }
}

#[test]
fn the_module_levels_own_path_and_subtree_are_kept_for_new_module_to_resolve_against() {
    let answer = vec![lib(
        "k7-core",
        "_libraries/k7-core/modules",
        vec![audio("_libraries/k7-core/modules", "reverb")],
    )];
    let rows = tree(&answer);
    let modules = &children(&rows[0])[0];
    assert_eq!(children(modules), answer[0].nodes.as_slice());
    assert_eq!(
        root_ns(&answer, modules["path"].as_str().unwrap_or("")).as_deref(),
        Some("k7-core")
    );
    // The repository row is no level: nothing is created at its root.
    assert_eq!(root_of(&answer, "_libraries/k7-core"), None);
}

#[test]
fn each_chain_is_a_folder_holding_its_nodes_at_their_real_paths() {
    let repo = repo();
    let rows = children(&children(&repo)[1]);
    assert_eq!(names(rows), ["live/vocals", "process/normalize"]);
    assert!(rows.iter().all(|c| c["dir"] == true));
    assert_eq!(rows[1]["path"], "_libraries/k7-core/chains/process/normalize");
    // The path is what tells a node's row from its chain's when one is clicked, so it is the real one and not a label.
    let node = &children(&rows[1])[0];
    assert_eq!(node["name"], "normalize.k7x");
    assert_eq!(
        node["path"],
        "_libraries/k7-core/chains/process/normalize/nodes/normalize.k7x"
    );
    assert_eq!(node["dir"], false);
    // One node away from having some; a leaf row would say it could never.
    assert!(children(&rows[0]).is_empty() && rows[0]["children"].is_array());
}

#[test]
fn a_chain_file_is_a_leaf_at_its_file() {
    let file_chain = [chain(
        "@k7-core/stems",
        "_libraries/k7-core/chains/stems.k7x",
        Some(vec![]),
    )];
    let rows = library_tree(&levels(), &file_chain, &[]);
    let leaf = &children(&children(&rows[0])[1])[0];
    assert_eq!(leaf["name"], "stems");
    assert_eq!(leaf["path"], "_libraries/k7-core/chains/stems.k7x");
    assert_eq!(leaf["dir"], false);
    assert!(leaf.get("children").is_none());
}

#[test]
fn each_assembly_template_is_a_leaf_at_its_folder() {
    let repo = repo();
    let leaf = &children(&children(&repo)[2])[0];
    assert_eq!(leaf["name"], "band/four-piece");
    assert_eq!(leaf["path"], "_libraries/k7-core/assemblies/band/four-piece");
    assert_eq!(leaf["dir"], false);
}

#[test]
fn every_family_is_drawn_when_a_library_holds_none_yet() {
    let rows = tree(&levels());
    assert_eq!(
        names(children(&rows[0])),
        [
            "modules",
            "chains",
            "assemblies",
            "samples",
            "irs",
            "wavetables",
            "models"
        ]
    );
}

#[test]
fn a_drop_lands_only_in_a_file_folder_or_inside_one_and_says_which() {
    assert_eq!(file_folder_of("_libraries/k7-core/samples"), Some("samples"));
    assert_eq!(file_folder_of("_libraries/k7-core/wavetables/akwf"), Some("wavetables"));
    assert_eq!(file_folder_of("_libraries/k7-core/irs/fender"), Some("irs"));
    assert_eq!(
        file_folder_of("_libraries/k7-core/models/guitarml proteus"),
        Some("models")
    );
    assert_eq!(file_folder_of("_libraries/k7-core/modules"), None);
    assert_eq!(file_folder_of("song.k7/samples"), None);
    assert_eq!(file_folder_of("_libraries/k7-core/samplesX"), None);
}

#[test]
fn each_chain_and_template_is_filed_under_its_own_librarys_row_named_within_it() {
    let mut two = levels();
    two.push(lib("drums", "_libraries/drums/modules", vec![]));
    let mut all_chains = chains();
    all_chains.push(chain("@drums/kit/room", "_libraries/drums/chains/kit/room", None));
    let rows = library_tree(&two, &all_chains, &assemblies());
    let chains_of = |row: &Value| {
        names(children(&children(row)[1]))
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(chains_of(&rows[0]), ["live/vocals", "process/normalize"]);
    assert_eq!(chains_of(&rows[1]), ["kit/room"]);
    assert!(children(&children(&rows[1])[2]).is_empty());
}
