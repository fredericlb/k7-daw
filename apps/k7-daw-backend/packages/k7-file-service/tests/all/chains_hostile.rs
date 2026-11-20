//! Hostile chains (the work order's table): a walk that is deep or wide, a ref of 64 KiB, a thousand chains of one last segment.

use k7_file_service::chains::{ChainSettings, ColdRun, find_by, resolve_cold_run};
use k7_file_service::space::Declined;
use k7_file_service::tree::Limits;

use crate::support::chains::World;

const VOX: &str =
    "cold(true)\nplugin({ type: \"offline\", name: \"vox\" })\nparams({ onnx: \"vox.onnx\" })\n";
const ONE: &str = "steps([id(\"s\").extends(\"@k7-core/cold/vox\"), to([chain(\"@out\")])])\n";

#[test]
fn a_thousand_chains_of_one_last_segment_are_found_without_a_quadratic_walk() {
    let names: Vec<String> = (0..1000)
        .map(|i| format!("libraries/k7-core/chains/g{i:04}/lead.k7x"))
        .collect();
    let mut entries: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), ONE)).collect();
    entries.push(("libraries/k7-core/modules/cold/vox/module.k7x", VOX));
    let w = World::new("chains-hostile-many", &entries);
    let c = w.chains();
    assert_eq!(c.refs().len(), 1000);
    let at = find_by(&c, None, "lead").expect("the first by ref");
    assert_eq!(c.refs()[at], "@k7-core/g0000/lead");
}

#[test]
fn a_ref_of_sixty_four_kib_and_one_that_climbs_answer_a_sentence() {
    let w = World::new(
        "chains-hostile-ref",
        &[
            ("libraries/k7-core/modules/cold/vox/module.k7x", VOX),
            ("libraries/k7-core/chains/a.k7x", ONE),
        ],
    );
    let c = w.chains();
    let none = ChainSettings::default();
    for chain in [
        "x".repeat(64 << 10),
        "../a".to_owned(),
        "@engine/x".to_owned(),
    ] {
        match resolve_cold_run(&c, &none, "stems", &chain) {
            Ok(ColdRun::Refused { status: 400, error }) => assert!(
                error.contains("no chain"),
                "{}",
                &error[..40.min(error.len())]
            ),
            other => panic!("{other:?}"),
        }
    }
    let wants = "w".repeat(64 << 10);
    assert!(matches!(
        resolve_cold_run(&c, &none, &wants, "a"),
        Ok(ColdRun::Refused { status: 400, .. })
    ));
}

#[test]
fn a_chains_folder_three_hundred_deep_is_declined_not_walked_to_the_end() {
    let mut deep = String::from("libraries/k7-core/chains");
    for _ in 0..300 {
        deep.push_str("/d");
    }
    deep.push_str("/c.k7x");
    let w = World::new("chains-hostile-deep", &[(&deep, ONE)]);
    let got =
        k7_file_service::chains::Chains::new(&w.source, &w.roots, &w.vocab, &Limits::default());
    assert_eq!(got.err(), Some(Declined::TooDeep));
}

#[test]
fn a_thousand_cold_chains_are_listed_against_one_vocabulary() {
    // The front's test holds the build count (`the_vocabulary_is_built_once...`); this one holds the size.
    let names: Vec<String> = (0..1000)
        .map(|i| format!("libraries/k7-core/chains/c{i:04}.k7x"))
        .collect();
    let mut entries: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), ONE)).collect();
    entries.push(("libraries/k7-core/modules/cold/vox/module.k7x", VOX));
    let w = World::new("chains-hostile-vocab", &entries);
    let c = w.chains();
    let list = k7_file_service::chains::cold_chains(&c).expect("a list");
    assert_eq!(list.as_array().unwrap().len(), 1000);
}

#[test]
fn a_graph_file_at_the_chains_root_stops_the_libraries_walk_as_the_typescript_does() {
    let w = World::new(
        "chains-hostile-root-graph",
        &[
            ("libraries/k7-core/chains/graph.k7x", "steps("),
            ("libraries/k7-core/chains/a.k7x", ONE),
            ("libraries/k7-core/modules/cold/vox/module.k7x", VOX),
        ],
    );
    assert!(w.chains().refs().is_empty());
}
