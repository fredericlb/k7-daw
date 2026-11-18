// modules_tree.rs — one root's walk and the vocabulary every root makes: keys, categories, engines, library nodes, findings.
// A reader would get wrong that nothing here is a snapshot: each case names the decision of docs/rustCompiler.md it holds.

use crate::support;

use song::modules::{ModuleKind, Vocabulary, base_module_refs, build_vocabulary_with};
use song::{Cache, Roots, Severity};
use support::{Tmp, disk, mem, plugin_text, roots};

fn vocab(t: &Tmp) -> Vocabulary {
    build(&roots(t))
}

fn build(r: &Roots) -> Vocabulary {
    let source = disk(r);
    let mut cache = Cache::new();
    build_vocabulary_with(&source, &mut cache, r, 100_000).expect("a vocabulary")
}

fn warned<'a>(v: &'a Vocabulary, file: &str) -> Vec<&'a song::Finding> {
    v.findings.iter().filter(|f| f.file == file).collect()
}

// Decision 32 and moduleNamespaceOf: the level is the root's PLACE, and the type folder is kept for io too.
#[test]
fn a_ref_carries_its_roots_namespace_and_the_type_folder() {
    let t = Tmp::new("tree-keys");
    t.write("ws/_modules/audio/delay/module.k7x", &plugin_text("Delay"));
    t.write("lib/account/modules/audio/fork/module.k7x", &plugin_text("Fork"));
    t.write("lib/k7-core/modules/audio/reverb/module.k7x", &plugin_text("Reverb"));
    t.write("lib/k7-core/modules/io/desk/module.k7x", "io(\"out\")\n");
    t.write("meta/instruments/piano/module.k7x", &plugin_text("Piano"));
    let v = vocab(&t);
    for key in [
        "@workspace/audio/delay",
        "@account/audio/fork",
        "@k7-core/audio/reverb",
        "@k7-core/io/desk",
        "@meta/instruments/piano",
        "@meta/doc",
        "@meta/track",
        "@meta/IN",
    ] {
        assert!(v.modules.contains_key(key), "{key} missing");
    }
    let reverb = &v.modules["@k7-core/audio/reverb"];
    assert_eq!(reverb.kind, Some(ModuleKind::Plugin));
    assert_eq!(reverb.category.as_deref(), Some("audio"));
    assert_eq!(reverb.file, "reverb");
    assert!(
        reverb
            .folder_abs
            .as_deref()
            .is_some_and(|p| p.ends_with("modules/audio/reverb"))
    );
    assert!(v.findings.is_empty(), "{:?}", v.findings);
    // moduleCategories.test.ts: a category module and a module of one folder are keyed alike.
    assert_eq!(v.modules["@k7-core/io/desk"].kind, Some(ModuleKind::Io));
}

// moduleCategories.test.ts and decision 20: one category level, only past a folder with no module file.
#[test]
fn a_category_is_one_level_deep_and_only_past_a_folder_without_a_module() {
    let t = Tmp::new("tree-category");
    t.write(
        "lib/k7-core/modules/audio/Effects/chorus/module.k7x",
        &plugin_text("Chorus"),
    );
    t.write(
        "lib/k7-core/modules/audio/Effects/deep/more/module.k7x",
        &plugin_text("Deep"),
    );
    t.write("lib/k7-core/modules/audio/real/module.k7x", &plugin_text("Real"));
    t.write("lib/k7-core/modules/audio/real/sub/module.k7x", &plugin_text("Sub"));
    let v = vocab(&t);
    assert!(v.modules.contains_key("@k7-core/audio/Effects/chorus"));
    assert!(v.modules.contains_key("@k7-core/audio/real"));
    for gone in [
        "@k7-core/audio/Effects/deep/more",
        "@k7-core/audio/real/sub",
        "@k7-core/audio/Effects/deep",
    ] {
        assert!(!v.modules.contains_key(gone), "{gone} was read");
    }
    // Decision 40: the folders the walk went through are what 2c looks panels up in, labelled by their library.
    let refs: Vec<&str> = v.folders.iter().map(|f| f.module_ref.as_str()).collect();
    assert!(refs.contains(&"@k7-core/audio/Effects/chorus") && refs.contains(&"@k7-core/audio/real"));
    assert!(v.folders.iter().all(|f| f.label == "k7-core"));
}

// engines.test.ts "loading": engines sorted, a broken one still named, and never an engine's engines.
#[test]
fn engines_are_modules_under_their_modules_and_never_nested() {
    let t = Tmp::new("tree-engines");
    let m = "lib/k7-core/modules/audio/amp";
    t.write(&format!("{m}/module.k7x"), &plugin_text("Amp"));
    t.write(&format!("{m}/engines/tracktion/module.k7x"), &plugin_text("Amp"));
    t.write(&format!("{m}/engines/NAM/module.k7x"), "paramz(1)\n");
    t.write(&format!("{m}/engines/ok/module.k7x"), &plugin_text("Ok"));
    t.write(
        &format!("{m}/engines/ok/engines/deeper/module.k7x"),
        &plugin_text("Deeper"),
    );
    t.write(&format!("{m}/engines/bad name/module.k7x"), &plugin_text("Bad"));
    t.write(&format!("{m}/engines/nomodule/readme.txt"), "x");
    let v = vocab(&t);
    let amp = &v.modules["@k7-core/audio/amp"];
    let refs = amp.engine_refs.as_ref().expect("engineRefs");
    assert_eq!(refs.keys().collect::<Vec<_>>(), ["NAM", "ok", "tracktion"]);
    assert_eq!(refs["tracktion"], "@k7-core/audio/amp/engines/tracktion");
    let e = &v.modules["@k7-core/audio/amp/engines/tracktion"];
    assert_eq!(e.engine_of.as_deref(), Some("@k7-core/audio/amp"));
    assert!(e.engine_refs.is_none());
    // The broken engine did not load, and is named all the same; the nested one is never read.
    assert!(!v.modules.contains_key("@k7-core/audio/amp/engines/NAM"));
    assert!(
        !v.modules.keys().any(|k| k.contains("deeper")),
        "an engine's engines were read"
    );
    assert_eq!(warned(&v, "@k7-core/audio/amp/engines/NAM/module.k7x").len(), 1);
    // baseModuleRefs: an engine is placed elsewhere.
    let base = base_module_refs(&v.modules);
    assert!(base.contains(&"@k7-core/audio/amp".to_owned()));
    assert!(!base.iter().any(|r| r.contains("/engines/")));
}

// Decision 19, moduleResolve's mergeAuthored and nodeFolder.test.ts: a library node is merged under its module, `ui` taken whole.
#[test]
fn a_library_node_is_merged_under_its_module() {
    let t = Tmp::new("tree-nodes");
    let m = "lib/k7-core/modules/audio/amp";
    t.write(
        &format!("{m}/module.k7x"),
        "plugin({ type: \"internal\", name: \"Amp\" })\nparams({ A: 1, C: 3 })\nui({ cols: 4, rows: 2, widgets: { w1: { type: \"knob\", col: 0, row: 0 } } })\n",
    );
    t.write(
        &format!("{m}/lead.k7x"),
        "params({ B: 2, C: 9 })\nui({ cols: 2, rows: 2, widgets: { w9: { type: \"knob\", col: 1, row: 1 } } })\n",
    );
    t.write(
        &format!("{m}/plain.k7x"),
        "content(\"a node with nothing of its own\")\n",
    );
    for own in ["ui", "controller", "presets", "dsp"] {
        t.write(&format!("{m}/{own}.k7x"), "x(1)\n");
    }
    let v = vocab(&t);
    let lead = &v.modules["@k7-core/audio/amp:lead"];
    assert_eq!(lead.node.as_deref(), Some("lead"));
    let json = serde_json::to_value(lead).expect("json");
    assert_eq!(json["params"], serde_json::json!({ "A": 1.0, "C": 9.0, "B": 2.0 }));
    // The node's grid, whole: the module's widget is not mixed in.
    assert_eq!(json["ui"]["widgets"].as_object().map(serde_json::Map::len), Some(1));
    assert!(json["ui"]["widgets"]["w9"].is_object());
    let plain = serde_json::to_value(&v.modules["@k7-core/audio/amp:plain"]).expect("json");
    assert!(
        plain["ui"]["widgets"]["w1"].is_object(),
        "a node without a grid takes the module's"
    );
    assert_eq!(plain["plugin"]["name"], "Amp");
    assert_eq!(plain["params"]["A"], 1.0);
    // The module's own files are not nodes, and a library node is no base module.
    assert!(
        !v.modules
            .keys()
            .any(|k| k.ends_with(":ui") || k.ends_with(":controller") || k.ends_with(":presets"))
    );
    assert!(!base_module_refs(&v.modules).iter().any(|r| r.contains(':')));
    // 2c reads the two junk files as a panel and a map: warnings, each at its own ref (decision 61).
    let files: Vec<&str> = v.findings.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(
        files,
        ["@k7-core/audio/amp/ui.k7x", "@k7-core/audio/amp/controller.k7x"]
    );
}

// Decision 31: node files in bytewise order of the base, where the TypeScript sorted `"base,name"` strings.
#[test]
fn library_nodes_come_in_bytewise_order_of_their_base() {
    let t = Tmp::new("tree-node-order");
    let m = "lib/k7-core/modules/audio/amp";
    t.write(&format!("{m}/module.k7x"), &plugin_text("Amp"));
    for n in ["b", "a-b", "a", "Z", "a+"] {
        t.write(&format!("{m}/{n}.k7x"), "content(\"x\")\n");
    }
    let v = vocab(&t);
    let nodes: Vec<&str> = v
        .modules
        .keys()
        .filter_map(|k| k.strip_prefix("@k7-core/audio/amp:"))
        .collect();
    assert_eq!(nodes, ["Z", "a", "a+", "a-b", "b"]);
}

// Decision 21: a library or engine module file that does not decode is a WARNING at its real place; the module is absent.
#[test]
fn a_broken_module_file_is_a_located_warning() {
    let t = Tmp::new("tree-broken");
    let m = "lib/k7-core/modules/audio/bad";
    t.write(
        &format!("{m}/module.k7x"),
        "plugin({ type: \"internal\" })\nparamz({})\n",
    );
    t.write(&format!("{m}/engines/tracktion/module.k7x"), &plugin_text("Bad"));
    t.write("lib/k7-core/modules/audio/good/module.k7x", &plugin_text("Good"));
    let v = vocab(&t);
    assert!(!v.modules.contains_key("@k7-core/audio/bad"));
    assert!(v.modules.contains_key("@k7-core/audio/good"));
    let w = warned(&v, "@k7-core/audio/bad/module.k7x");
    assert_eq!(w.len(), 1);
    assert_eq!((w[0].severity, w[0].line, w[0].col), (Severity::Warning, 2, 1));
    // Its engines are not read: the TypeScript returns before them.
    assert!(!v.modules.keys().any(|k| k.contains("/bad/engines")));
}

// libraryModuleIssues.test.ts, the first four: what a library module that would not read reports.
#[test]
fn the_library_issues_name_the_ref_and_say_why() {
    let t = Tmp::new("tree-issues");
    t.write(
        "lib/k7-core/modules/audio/typo/module.k7x",
        "plugin({ type: \"internal\", name: \"X\" })\nparamz({})\n",
    );
    t.write("lib/k7-core/modules/audio/syntax/module.k7x", "ui(\n");
    t.write("lib/k7-core/modules/audio/fine/module.k7x", &plugin_text("Fine"));
    let v = vocab(&t);
    let files: Vec<&str> = v.findings.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(files.len(), 2);
    assert!(files.contains(&"@k7-core/audio/typo/module.k7x") && files.contains(&"@k7-core/audio/syntax/module.k7x"));
    assert!(
        v.findings
            .iter()
            .all(|f| f.severity == Severity::Warning && f.message.starts_with("did not load"))
    );
    assert!(warned(&v, "@k7-core/audio/fine/module.k7x").is_empty());
}

// The user's ruling on decision 22, which decision 38 replaces: a library node file that does not read WARNS, like a broken module file.
#[test]
fn a_broken_library_node_file_is_a_warning_too() {
    let t = Tmp::new("tree-node-warn");
    let m = "lib/k7-core/modules/audio/amp";
    t.write(&format!("{m}/module.k7x"), &plugin_text("Amp"));
    t.write(&format!("{m}/bad.K7X"), "nonsense(1)\n");
    t.write(&format!("{m}/good.k7x"), "content(\"x\")\n");
    let v = vocab(&t);
    assert!(!v.modules.contains_key("@k7-core/audio/amp:bad"));
    assert!(v.modules.contains_key("@k7-core/audio/amp:good"));
    let w = warned(&v, "@k7-core/audio/amp/bad.K7X");
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].severity, Severity::Warning);
}

// Decision 20: the walk skips dot entries everywhere, so a module is never handed another one's panel.
#[test]
fn dot_entries_are_skipped_everywhere() {
    let t = Tmp::new("tree-dots");
    let lib = "lib/k7-core/modules";
    t.write(&format!("{lib}/audio/.hidden/module.k7x"), &plugin_text("Hidden"));
    t.write(&format!("{lib}/audio/Cat/.old/module.k7x"), &plugin_text("Old"));
    t.write(&format!("{lib}/audio/vis/module.k7x"), &plugin_text("Vis"));
    t.write(&format!("{lib}/audio/vis/.stash.k7x"), "content(\"x\")\n");
    let v = vocab(&t);
    assert!(v.modules.contains_key("@k7-core/audio/vis"));
    assert!(
        !v.modules
            .keys()
            .any(|k| k.contains(".hidden") || k.contains(".old") || k.contains(".stash"))
    );
    assert!(!v.folders.iter().any(|f| f.path.contains("/.")));
}

// A type folder that is a file, a root with no type folder and a root that is not there are all empty.
#[test]
fn what_is_not_a_folder_is_skipped() {
    let t = Tmp::new("tree-files");
    t.write("lib/k7-core/modules/audio", "i am a file");
    t.write("lib/k7-core/modules/instruments/x/module.k7x", &plugin_text("X"));
    t.write("lib/k7-core/modules/shaders/file-not-folder", "x");
    let v = vocab(&t);
    assert!(v.modules.contains_key("@k7-core/instruments/x"));
    assert!(
        !v.modules
            .keys()
            .any(|k| k.starts_with("@k7-core/audio") || k.starts_with("@k7-core/shaders"))
    );
    assert!(v.findings.is_empty());
}

// Decision 24: a link loop in a type folder is bounded by the walk's shape, and a link out of every mount is a warning.
#[cfg(unix)]
#[test]
fn links_are_bounded_and_confined() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("tree-links");
    let lib = t.0.join("lib/k7-core/modules/audio");
    t.write("lib/k7-core/modules/audio/real/module.k7x", &plugin_text("Real"));
    symlink(&lib, lib.join("loop")).expect("a link loop");
    symlink(lib.join("b"), lib.join("a")).expect("a");
    symlink(lib.join("a"), lib.join("b")).expect("b");
    t.write("elsewhere/evil/module.k7x", &plugin_text("Evil"));
    symlink(t.0.join("elsewhere/evil"), lib.join("evil")).expect("a link out");
    let started = std::time::Instant::now();
    let v = vocab(&t);
    assert!(started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
    assert!(v.modules.contains_key("@k7-core/audio/real"));
    assert!(!v.modules.contains_key("@k7-core/audio/evil"));
    let out = warned(&v, "@k7-core/audio/evil");
    assert_eq!(out.len(), 1, "{:?}", v.findings);
    assert_eq!(out[0].severity, Severity::Warning);
}

// Decision 24: a folder that cannot be listed is skipped silently, with the rest of the library still read.
#[cfg(unix)]
#[test]
fn an_unreadable_folder_is_skipped_silently() {
    use std::os::unix::fs::PermissionsExt;
    use support::skip;
    let t = Tmp::new("tree-unreadable");
    t.write("lib/k7-core/modules/audio/ok/module.k7x", &plugin_text("Ok"));
    t.write("lib/k7-core/modules/audio/locked/module.k7x", &plugin_text("Locked"));
    let locked = t.0.join("lib/k7-core/modules/audio/locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let v = vocab(&t);
    let readable = std::fs::read_dir(&locked).is_ok();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("chmod back");
    assert!(v.modules.contains_key("@k7-core/audio/ok"));
    if readable {
        skip(
            "an_unreadable_folder_is_skipped_silently",
            "this file system or user reads a mode 000 folder",
        );
    } else {
        assert!(!v.modules.contains_key("@k7-core/audio/locked"));
        assert!(v.findings.is_empty(), "{:?}", v.findings);
    }
}

// Decision 24's Windows half: a name holding `\` is refused there and is a character elsewhere (over MemSource, which holds any name).
#[test]
fn a_backslash_in_a_name_is_refused_under_the_windows_rule_and_a_character_under_the_posix_one() {
    let text = plugin_text("A");
    let source = mem(&[
        ("lib/modules/audio/a\\b/module.k7x", &text),
        ("lib/modules/audio/fine/module.k7x", &text),
    ]);
    for rule in [song::PathRule::Posix, song::PathRule::Windows] {
        let r = Roots {
            project: "/p/proj".into(),
            libraries: vec![song::Library {
                name: "k7-core".into(),
                root: "/p/lib".into(),
                editable: false,
            }],
            paths: rule,
            ..Roots::default()
        };
        let v = build_vocabulary_with(&source, &mut Cache::with_paths(rule), &r, 100_000).expect("a vocabulary");
        assert!(v.modules.contains_key("@k7-core/audio/fine"));
        assert_eq!(
            v.modules.contains_key("@k7-core/audio/a\\b"),
            !rule.is_windows(),
            "{rule:?}"
        );
    }
}

/// Whether the temporary folder's file system folds case, as Windows' and macOS's default do.
fn folds_case(t: &Tmp) -> bool {
    t.write("probe/Case.tmp", "x");
    t.0.join("probe/case.tmp").exists()
}

// Decision 33: a fixed name is probed by `stat`, so where the file system folds case `Module.K7X` is the module file.
#[test]
fn the_module_file_is_probed_by_stat_and_its_name_compared_exactly() {
    let t = Tmp::new("tree-case");
    t.write("lib/k7-core/modules/audio/x/Module.K7X", &plugin_text("X"));
    let v = vocab(&t);
    if folds_case(&t) {
        // The file system calls it module.k7x, and the listing calls it a node named `Module`.
        assert!(v.modules.contains_key("@k7-core/audio/x"), "{:?}", v.findings);
        assert!(v.modules.contains_key("@k7-core/audio/x:Module"));
    } else {
        assert!(!v.modules.contains_key("@k7-core/audio/x"));
        assert!(!v.modules.contains_key("@k7-core/audio/x:Module"));
    }
}
