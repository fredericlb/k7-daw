// roots.rs — the roots the host resolves: their order, their namespaces, their mounts, and what keys the vocabulary.
// A reader would get wrong that the crate never finds a root: where a library lives and which version is pinned are the host's facts.

use crate::support;

use song::{Library, Roots};
use support::{Tmp, roots};

// sharedFolders.test.ts, the order and namespace cases: workspace first, then the libraries as given, `@meta` last.
#[test]
fn module_roots_come_nearest_first_each_with_its_place_as_namespace() {
    let t = Tmp::new("roots-order");
    let r = roots(&t);
    let got: Vec<(String, String)> = r
        .module_roots()
        .expect("roots")
        .into_iter()
        .map(|m| (m.namespace, m.dir))
        .collect();
    let want = vec![
        ("workspace".to_owned(), t.at("ws/_modules")),
        ("account".to_owned(), t.at("lib/account/modules")),
        ("k7-core".to_owned(), t.at("lib/k7-core/modules")),
        ("meta".to_owned(), t.at("meta")),
    ];
    assert_eq!(got, want);
}

// sharedFolders.test.ts: a root listed twice is read once, under the first name that gave it; none declared, none read.
#[test]
fn a_root_listed_twice_is_read_once() {
    let t = Tmp::new("roots-dup");
    let mut r = roots(&t);
    r.libraries.push(Library {
        name: "again".into(),
        root: t.at("lib/k7-core"),
        editable: false,
    });
    let names: Vec<String> = r
        .module_roots()
        .expect("roots")
        .into_iter()
        .map(|m| m.namespace)
        .collect();
    assert_eq!(names, ["workspace", "account", "k7-core", "meta"]);
    let bare = Roots {
        project: t.at("ws/proj"),
        ..Roots::default()
    };
    assert!(bare.module_roots().expect("roots").is_empty());
}

// Decision 25: the vocabulary is keyed by the roots MINUS the project, so the projects of one workspace share it.
#[test]
fn the_vocabulary_key_leaves_the_project_out() {
    let t = Tmp::new("roots-key");
    let a = roots(&t);
    let mut b = a.clone();
    b.project = t.at("ws/another");
    b.renders = Some(t.at("home/.renders"));
    b.sample_banks = vec![t.at("samples")];
    assert_eq!(a.vocabulary_key(), b.vocabulary_key());
    let mut c = a.clone();
    c.libraries.pop();
    assert_ne!(a.vocabulary_key(), c.vocabulary_key());
    let mut d = a.clone();
    d.scan = None;
    assert_ne!(a.vocabulary_key(), d.vocabulary_key());
}

// What a Source must serve: folders for the roots, the scan as a file.
#[test]
fn the_mounts_are_folders_and_the_scan_file() {
    let t = Tmp::new("roots-mounts");
    let (dirs, files) = roots(&t).mounts().expect("mounts");
    assert!(dirs.contains(&t.at("ws/proj")) && dirs.contains(&t.at("meta")) && dirs.contains(&t.at("lib/k7-core")));
    assert_eq!(files, [t.at("home/exts.json")]);
}

// The host hands the roots as JSON, and a key it does not know is refused.
#[test]
fn the_roots_are_json() {
    let r: Roots = serde_json::from_str(
        r#"{"project":"/w/p","libraries":[{"name":"k7-core","root":"/l","editable":false}],"scan":"/h/exts.json"}"#,
    )
    .expect("roots");
    assert_eq!(r.libraries[0].name, "k7-core");
    assert!(serde_json::from_str::<Roots>(r#"{"project":"/w/p","pins":{}}"#).is_err());
}

// Decision 37 on Windows: roots given as `c:\Users\…` normalize to `C:/Users/…`.
#[cfg(windows)]
#[test]
fn windows_roots_are_spelled_as_the_source_spells_them() {
    let r = Roots {
        project: r"c:\Users\me\ws\proj".into(),
        workspace: Some(r"c:\Users\me\ws".into()),
        scan: Some(r"c:\Users\me\home\exts.json".into()),
        ..Roots::default()
    };
    assert_eq!(r.module_roots().expect("roots")[0].dir, "C:/Users/me/ws/_modules");
    let (dirs, files) = r.mounts().expect("mounts");
    assert_eq!(dirs[0], "C:/Users/me/ws/proj");
    assert_eq!(files, ["C:/Users/me/home/exts.json"]);
}

// Decisions 736 and 790: a folder readable with no namespace is mounted, carries no module root, and keys the vocabulary.
#[test]
fn also_readable_is_mounted_without_a_namespace_and_changes_the_vocabulary_key() {
    let t = Tmp::new("roots-also");
    let base = roots(&t);
    let mut r = base.clone();
    r.also_readable = vec![t.at("lib/account2"), t.at("lib/account/inside")];
    let (dirs, _) = r.mounts().expect("mounts");
    assert!(dirs.contains(&t.at("lib/account2")));
    // A folder inside a mount adds nothing: it is already read.
    assert!(!dirs.contains(&t.at("lib/account/inside")));
    assert_eq!(r.module_roots().expect("roots"), base.module_roots().expect("roots"));
    assert_ne!(r.vocabulary_key(), base.vocabulary_key());
}

// Decision 739: the rule of the machine that spelled the roots keys the vocabulary too.
#[test]
fn the_path_rule_keys_the_vocabulary() {
    let t = Tmp::new("roots-rule");
    let mut r = roots(&t);
    let host = r.vocabulary_key();
    r.paths = if song::PathRule::HOST.is_windows() {
        song::PathRule::Posix
    } else {
        song::PathRule::Windows
    };
    assert_ne!(r.vocabulary_key(), host);
}
