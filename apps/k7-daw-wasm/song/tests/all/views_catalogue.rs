// views_catalogue.rs — the module picker's two lists, after moduleCatalogue.test.ts and listBaseModuleRefs.
// A reader would get wrong that a row without a file is dropped, and an engine row without a root is kept: decision 620.

use crate::support;
use crate::views_support;

use song::views::catalogue::{CatalogueModule, Listed, base_modules, module_catalogue};
use song::{Cache, Library, Roots, Severity};
use support::{mem, mem_world_roots};
use views_support::fast;

const PLUGIN: &str = "plugin({ type: \"internal\", name: \"reverb\" })\n";

fn catalogue(files: &[(&str, &str)], roots: &Roots) -> Listed<CatalogueModule> {
    let src = mem(files);
    let v = Cache::new().vocabulary(&src, roots).expect("a vocabulary");
    module_catalogue(&src, roots, &v).expect("a catalogue")
}

fn workspace() -> Vec<(&'static str, &'static str)> {
    vec![
        ("ws/_modules/audio/reverb/module.k7x", PLUGIN),
        (
            "ws/_modules/audio/amp/nam/module.k7x",
            "plugin({ type: \"internal\", name: \"nam\" })\nextends(\"@k7-core/audio/x\")\n",
        ),
        ("ws/_modules/io/IN/module.k7x", "io(\"in\")\n"),
        ("ws/_modules/io/IN/in-1.k7x", "channel([1])\n"),
        ("ws/_modules/io/IN/in-2.k7x", "channel([2])\n"),
        ("ws/proj/project.k7x", ""),
    ]
}

fn row<'a>(l: &'a Listed<CatalogueModule>, r: &str) -> Option<&'a CatalogueModule> {
    l.items.iter().find(|m| m.ref_ == r)
}

#[test]
fn lists_a_module_at_either_depth_its_catalogue_allows() {
    // moduleCatalogue.test.ts: lists a module at either depth its catalogue allows (619, 620).
    let l = catalogue(&workspace(), &mem_world_roots());
    for r in [
        "@workspace/audio/reverb",
        "@workspace/audio/amp/nam",
        "@workspace/io/IN",
    ] {
        assert!(row(&l, r).is_some(), "{r}");
    }
    assert!(!l.capped && l.findings.is_empty());
}

#[test]
fn lists_the_generics_only_counting_the_nodes_instead() {
    // moduleCatalogue.test.ts: lists the GENERICS only, counting the nodes instead (620).
    let l = catalogue(&workspace(), &mem_world_roots());
    assert!(row(&l, "@workspace/io/IN:in-1").is_none());
    assert_eq!(row(&l, "@workspace/io/IN").map(|m| m.nodes), Some(2));
}

#[test]
fn points_at_the_absolute_file_the_builder_opens() {
    // moduleCatalogue.test.ts: points at the file the builder opens (620); absolute, as the host relativizes it.
    let l = catalogue(&workspace(), &mem_world_roots());
    let nam = row(&l, "@workspace/audio/amp/nam").expect("a row");
    assert_eq!(nam.path.as_deref(), Some("/p/ws/_modules/audio/amp/nam/module.k7x"));
    assert_eq!(
        (nam.name.as_str(), nam.namespace.as_str(), nam.category.as_str()),
        ("audio/amp/nam", "workspace", "audio")
    );
    assert_eq!(
        (nam.kind.as_str(), nam.plugin.as_deref(), nam.extends.as_deref()),
        ("plugin", Some("nam"), Some("@k7-core/audio/x"))
    );
}

#[test]
fn the_projects_own_modules_are_not_the_catalogue() {
    // moduleCatalogue.test.ts: leaves the project's OWN modules out (620).
    let mut files = workspace();
    files.push(("ws/proj/nodes/mine.k7x", PLUGIN));
    let l = catalogue(&files, &mem_world_roots());
    assert!(
        l.items
            .iter()
            .all(|m| m.ref_.starts_with('@') && !m.ref_.starts_with("@project/"))
    );
}

#[test]
fn a_project_with_no_shared_library_still_offers_the_engines_modules() {
    // moduleCatalogue.test.ts: still offers the ENGINE's modules to a project with no shared library (619).
    let roots = Roots {
        project: "/p/proj".into(),
        ..Roots::default()
    };
    let l = catalogue(&[("proj/project.k7x", "")], &roots);
    let refs: Vec<_> = l.items.iter().map(|m| m.ref_.as_str()).collect();
    assert!(refs.contains(&"@meta/IN") && refs.contains(&"@meta/OUT"));
    assert!(
        refs.iter()
            .all(|r| r.starts_with("@engine/") || r.starts_with("@meta/"))
    );
    // The io roles of `@meta` carry no file: no path, and `hasUi` is false.
    let inn = row(&l, "@meta/IN").expect("IN");
    assert!(inn.path.is_none() && !inn.has_ui && inn.namespace == "meta");
}

#[test]
fn a_ref_whose_file_is_absent_or_whose_namespace_has_no_root_is_dropped() {
    // decision 620: a namespaced ref with no root and a file not there are dropped.
    let roots = Roots {
        project: "/p/proj".into(),
        libraries: vec![Library {
            name: "lib".into(),
            root: "/p/lib".into(),
            editable: false,
        }],
        ..Roots::default()
    };
    let l = catalogue(
        &[("proj/project.k7x", ""), ("lib/modules/audio/a/module.k7x", PLUGIN)],
        &roots,
    );
    assert!(row(&l, "@lib/audio/a").is_some());
    let mut with_gap = Roots {
        libraries: roots.libraries.clone(),
        ..roots.clone()
    };
    with_gap.libraries.push(Library {
        name: "ghost".into(),
        root: "/p/ghost".into(),
        editable: false,
    });
    let l = catalogue(
        &[("proj/project.k7x", ""), ("lib/modules/audio/a/module.k7x", PLUGIN)],
        &with_gap,
    );
    assert!(l.items.iter().all(|m| m.namespace != "ghost"));
}

#[test]
fn a_panel_and_a_ui_are_told() {
    // moduleCatalogue.ts: `panel` from the module's ui grid, `hasUi` from its being set (620, 621).
    let ui = "plugin({ type: \"internal\", name: \"eq\" })\nui({ cols: 4, rows: 2, widgets: {} })\n";
    let mut files = workspace();
    files.push(("ws/_modules/audio/eq/module.k7x", ui));
    let src = mem(&files);
    let roots = mem_world_roots();
    let v = Cache::new().vocabulary(&src, &roots).expect("a vocabulary");
    let l = module_catalogue(&src, &roots, &v).expect("a catalogue");
    let eq = row(&l, "@workspace/audio/eq").expect("eq");
    assert!(eq.has_ui);
    assert_eq!(eq.panel.map(|p| (p.cols, p.rows)), Some((4, 2)));
    assert_eq!(serde_json::to_value(eq).expect("json")["hasUi"], true);
    assert!(!row(&l, "@workspace/audio/reverb").expect("reverb").has_ui);
    // `base_modules` is the same walk with the panel: the plugin refs, no io, sorted.
    let b = base_modules(&v);
    let eq = b
        .items
        .iter()
        .find(|m| m.ref_ == "@workspace/audio/eq")
        .expect("eq base");
    assert_eq!(eq.panel.map(|p| p.cols), Some(4));
    assert!(b.items.iter().all(|m| !m.ref_.starts_with("@workspace/io")));
    let refs: Vec<_> = b.items.iter().map(|m| m.ref_.clone()).collect();
    let mut sorted = refs.clone();
    sorted.sort();
    assert_eq!(refs, sorted);
}

#[test]
fn two_thousand_refs_of_one_namespace_are_listed_in_time() {
    // hostile table (module_catalogue): 2,000 refs read through the memory source (quadratic past that, `source/mem.rs`); views_hostile.rs builds 100,000 by hand.
    let names: Vec<String> = (0..2_000)
        .map(|i| format!("ws/_modules/audio/m{i}/module.k7x"))
        .collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), PLUGIN)).collect();
    let src = mem(&files);
    let roots = mem_world_roots();
    let v = Cache::new().vocabulary(&src, &roots).expect("a vocabulary");
    let l = fast("2,000 refs", || {
        module_catalogue(&src, &roots, &v).expect("a catalogue")
    });
    println!("catalogue spent {} units", l.spent);
    assert!(l.items.len() >= 2_000 && !l.capped);
    let b = fast("2,000 bases", || base_modules(&v));
    assert!(b.items.len() >= 2_000 && !b.capped);
}

#[test]
fn long_refs_are_capped_by_their_bytes() {
    // hostile table (module_catalogue, base_modules): the output counts bytes, so a long ref spends its weight.
    // A bundle key holds 8 KiB, so the weight is many refs of 7,000 bytes.
    let long = "x".repeat(7_000);
    let names: Vec<String> = (0..3_500)
        .map(|i| format!("ws/_modules/audio/{i}{long}/module.k7x"))
        .collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), PLUGIN)).collect();
    let src = mem(&files);
    let roots = mem_world_roots();
    let v = Cache::new().vocabulary(&src, &roots).expect("a vocabulary");
    let l = fast("3,500 refs of 7 KB", || {
        module_catalogue(&src, &roots, &v).expect("a catalogue")
    });
    assert!(
        l.capped && l.spent > 1_000_000 / 2,
        "{} over {} items",
        l.spent,
        l.items.len()
    );
    assert_eq!(l.findings[0].severity, Severity::Warning);
    assert_eq!(l.findings[0].file, "project.k7x");
}

#[test]
fn a_root_named_twice_is_read_once_and_the_later_root_of_a_namespace_wins() {
    // decision 619: a root listed twice is one; two roots of one namespace answer the later one's folder.
    let roots = Roots {
        project: "/p/proj".into(),
        libraries: vec![
            Library {
                name: "lib".into(),
                root: "/p/a".into(),
                editable: false,
            },
            Library {
                name: "lib".into(),
                root: "/p/a".into(),
                editable: false,
            },
            Library {
                name: "lib".into(),
                root: "/p/b".into(),
                editable: false,
            },
        ],
        ..Roots::default()
    };
    let files = [
        ("proj/project.k7x", ""),
        ("a/modules/audio/m/module.k7x", PLUGIN),
        ("b/modules/audio/n/module.k7x", PLUGIN),
    ];
    let l = catalogue(&files, &roots);
    // Only the later root's folder is opened for the namespace: `m` has no file there and is dropped.
    assert!(row(&l, "@lib/audio/n").is_some());
    assert!(row(&l, "@lib/audio/m").is_none());
}
