//! The libraries' families (1415 to 1418): which libraries an account holds, a family's folder in each, the chains
//! (a ref held both ways is the file's, a graph file makes a folder a chain) and the assemblies (a dangling link throws).

use crate::support;

use k7_file_service::libraries::ACCOUNT_LIBRARY;
use k7_file_service::listings::families::{account_libraries, family_roots, library_names};
use k7_file_service::listings::{Listed, Listing, Remote, list};
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use serde_json::Value;
use support::{Scratch, can_link, link, link_file};

fn run(s: &Scratch, which: Listing, limits: &Limits) -> Result<Listed, Declined> {
    let h = s.home();
    s.dir("ws");
    list(
        &Space::open(&h, &s.path("ws")),
        which,
        &Remote::default(),
        limits,
    )
}

fn refs(s: &Scratch, which: Listing, key: &str) -> Vec<(String, String)> {
    let l = run(s, which, &Limits::default()).unwrap();
    serde_json::from_str::<Value>(&l.json).unwrap()[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["ref"].as_str().unwrap().to_string(),
                r["path"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn the_libraries_are_the_named_folders_sorted_and_a_link_is_none() {
    let s = Scratch::new("fam-names");
    for d in ["zed", "k7-core", "Upper", "meta", "-x", "ok-2"] {
        s.dir(&format!("libraries/{d}"));
    }
    s.file("libraries/a-file", "");
    if can_link(&s) {
        s.dir("elsewhere");
        link(&s.path("elsewhere"), &s.path("libraries/linked")).unwrap();
    }
    // 1415: a dirent's type decides, no link is followed, and a reserved or ill-spelt name is none.
    assert_eq!(library_names(&s.home()), ["k7-core", "ok-2", "zed"]);
}

#[test]
fn the_account_library_comes_first_and_only_a_family_that_exists_is_a_root() {
    let s = Scratch::new("fam-roots");
    s.dir("studio/library/chains");
    s.dir("libraries/k7-core/chains");
    s.dir("libraries/zed/modules");
    let h = s.home();
    let libs = account_libraries(&h, None).unwrap();
    let roots = family_roots(&h, &libs, "chains");
    let names: Vec<&str> = roots.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, [ACCOUNT_LIBRARY, "k7-core"]);
    assert_eq!(roots[1].root, s.path("libraries/k7-core/chains"));
}

#[test]
fn a_ref_held_as_a_file_and_as_a_folder_is_the_files() {
    let s = Scratch::new("fam-both");
    s.file("libraries/k7-core/chains/dup.k7x", "");
    s.file("libraries/k7-core/chains/dup/graph.k7x", "");
    s.file("libraries/k7-core/chains/folder/graph.k7x", "");
    s.file("libraries/k7-core/chains/folder/nodes/n.k7x", "");
    assert_eq!(
        refs(&s, Listing::LibraryChains, "chains"),
        [
            (
                "@k7-core/dup".to_string(),
                "_libraries/k7-core/chains/dup.k7x".to_string()
            ),
            (
                "@k7-core/folder".to_string(),
                "_libraries/k7-core/chains/folder".to_string()
            ),
        ]
    );
}

#[test]
fn a_folder_chains_nodes_sort_by_unit_and_its_refs_by_code_point() {
    let s = Scratch::new("fam-order");
    for n in ["\u{1F600}", "\u{ff5e}", "a", "B"] {
        s.file(&format!("libraries/k7-core/chains/fold/nodes/{n}.k7x"), "");
    }
    s.file("libraries/k7-core/chains/fold/graph.k7x", "");
    s.file("libraries/k7-core/chains/fold/nodes/skip.txt", "");
    s.file("libraries/k7-core/chains/\u{1F600}.k7x", "");
    s.file("libraries/k7-core/chains/\u{ff5e}.k7x", "");
    let l = run(&s, Listing::LibraryChains, &Limits::default()).unwrap();
    let v: Value = serde_json::from_str(&l.json).unwrap();
    // .sort() puts the astral name before U+FF5E; a ref sorts the other way round.
    assert_eq!(
        v["chains"][0]["nodes"],
        serde_json::json!(["B.k7x", "a.k7x", "\u{1F600}.k7x", "\u{ff5e}.k7x"])
    );
    let order: Vec<&str> = v["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["ref"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        ["@k7-core/fold", "@k7-core/\u{ff5e}", "@k7-core/\u{1F600}"]
    );
}

#[test]
fn a_graph_file_at_the_root_stops_the_walk_and_a_graph_that_is_a_folder_makes_no_chain() {
    let s = Scratch::new("fam-graph");
    s.file("libraries/k7-core/chains/graph.k7x", "");
    s.file("libraries/k7-core/chains/a.k7x", "");
    s.file("libraries/zed/chains/g/graph.k7x/inner.k7x", "");
    s.file("libraries/zed/chains/g/after.k7x", "");
    let got: Vec<String> = refs(&s, Listing::LibraryChains, "chains")
        .into_iter()
        .map(|r| r.0)
        .collect();
    assert_eq!(got, ["@zed/g/after", "@zed/g/graph.k7x/inner"]);
}

#[test]
fn a_junction_is_no_chain_folder() {
    let s = Scratch::new("fam-link");
    if !can_link(&s) {
        return;
    }
    s.file("libraries/k7-core/chains/real.k7x", "");
    s.file("elsewhere/e.k7x", "");
    s.file("elsewhere/f.k7x", "");
    link(
        &s.path("elsewhere"),
        &s.path("libraries/k7-core/chains/alias"),
    )
    .unwrap();
    link_file(
        &s.path("elsewhere/e.k7x"),
        &s.path("libraries/k7-core/chains/l.k7x"),
    )
    .unwrap();
    // A link is neither a folder to walk nor a file to list, as `host/fs.rs`' `kind` reads it.
    assert_eq!(refs(&s, Listing::LibraryChains, "chains").len(), 1);
    // A graph file that is a link makes no folder chain either.
    s.file("libraries/k7-core/chains/c/x.k7x", "");
    link_file(
        &s.path("elsewhere/e.k7x"),
        &s.path("libraries/k7-core/chains/c/graph.k7x"),
    )
    .unwrap();
    let got: Vec<String> = refs(&s, Listing::LibraryChains, "chains")
        .into_iter()
        .map(|r| r.0)
        .collect();
    assert_eq!(got, ["@k7-core/c/x", "@k7-core/real"]);
}

#[test]
fn the_chains_folder_may_itself_be_a_link() {
    let s = Scratch::new("fam-root-link");
    if !can_link(&s) {
        return;
    }
    s.file("elsewhere/e.k7x", "");
    s.dir("libraries/k7-core");
    link(&s.path("elsewhere"), &s.path("libraries/k7-core/chains")).unwrap();
    assert_eq!(refs(&s, Listing::LibraryChains, "chains").len(), 1);
}

#[test]
fn an_unreadable_chain_folder_costs_nothing() {
    let s = Scratch::new("fam-empty");
    s.dir("libraries/k7-core");
    s.file("libraries/zed/chains", "a file, not a folder");
    assert!(refs(&s, Listing::LibraryChains, "chains").is_empty());
}

#[test]
fn an_assembly_is_a_folder_holding_the_file_and_nothing_inside_it_is_another() {
    let s = Scratch::new("fam-asm");
    s.file("studio/library/assemblies/turntable/assembly.k7x", "");
    s.file("studio/library/assemblies/turntable/inner/assembly.k7x", "");
    s.file("studio/library/assemblies/group/sub/assembly.k7x", "");
    s.file("studio/library/assemblies/loose.k7x", "");
    s.file("libraries/k7-core/assemblies/live/assembly.k7x", "");
    let l = run(&s, Listing::LibraryAssemblies, &Limits::default()).unwrap();
    let v: Value = serde_json::from_str(&l.json).unwrap();
    let got: Vec<(&str, &str)> = v["assemblies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["ref"].as_str().unwrap(), a["file"].as_str().unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            (
                "@account/group/sub",
                "_libraries/account/assemblies/group/sub/assembly.k7x"
            ),
            (
                "@account/turntable",
                "_libraries/account/assemblies/turntable/assembly.k7x"
            ),
            (
                "@k7-core/live",
                "_libraries/k7-core/assemblies/live/assembly.k7x"
            ),
        ]
    );
    assert_eq!(l.rows.len(), 3);
}

#[test]
fn a_dangling_link_among_the_assemblies_is_what_the_typescript_throws_on() {
    let s = Scratch::new("fam-throw");
    if !can_link(&s) {
        return;
    }
    s.file("libraries/k7-core/assemblies/ok/assembly.k7x", "");
    link(
        &s.path("nowhere"),
        &s.path("libraries/k7-core/assemblies/gone"),
    )
    .unwrap();
    // 1418: declined, where the TypeScript answered 500.
    assert_eq!(
        run(&s, Listing::LibraryAssemblies, &Limits::default()).unwrap_err(),
        Declined::WouldThrow
    );
}

#[test]
fn the_walks_count_every_entry_and_every_level() {
    let s = Scratch::new("fam-limits");
    for i in 0..40 {
        s.file(&format!("libraries/k7-core/chains/c{i}.k7x"), "");
    }
    let few = Limits {
        max_seen: 20,
        ..Limits::default()
    };
    assert_eq!(
        run(&s, Listing::LibraryChains, &few).unwrap_err(),
        Declined::TooMany
    );
    s.dir(&format!("libraries/zed/assemblies/{}", "d/".repeat(6)));
    let shallow = Limits {
        max_depth: 3,
        ..Limits::default()
    };
    assert_eq!(
        run(&s, Listing::LibraryAssemblies, &shallow).unwrap_err(),
        Declined::TooDeep
    );
}
