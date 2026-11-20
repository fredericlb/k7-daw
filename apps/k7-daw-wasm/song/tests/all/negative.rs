// negative.rs — known-bad files, family by family: each must be a finding for that file and for no other, on disk and in memory.
// A reader would get wrong that this needs no TypeScript: the expected file sets are written in negative.json; stages.rs adds the TypeScript's.

use crate::support;

use serde_json::{Value, json};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use std::collections::BTreeMap;
use support::{Case, Tmp, case_roots, cases, dump, dump_modules, finding_files, head_flags, placed, spell};

fn names(expect: &Value, family: &str) -> Vec<String> {
    let mut v: Vec<String> = expect[family]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn every_known_bad_file_is_a_finding_for_that_file_and_no_other() {
    let t = Tmp::new("negative");
    let mut per_category: BTreeMap<String, usize> = BTreeMap::new();
    for case in cases() {
        if case.module {
            *per_category.entry(case.category.clone()).or_default() += module_case(&t, &case);
            continue;
        }
        let project = t.project(&case.name, &case.files);
        let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
        let rec = Recorder::new(&disk);
        let on_disk = dump(&rec, &project).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let mut expected = 0;
        for family in ["clips", "sequences", "timelines"] {
            let want = names(&case.expect, family);
            expected += want.len();
            assert_eq!(finding_files(&on_disk.json, family), want, "{} {family}", case.name);
        }
        let flag = |k: &str| case.expect["head"][k].as_bool().unwrap_or(false);
        let want_head = json!({
            "format_too_new": flag("format_too_new"), "root": flag("root"),
            "settings": flag("settings"), "compile": flag("compile"),
        });
        assert_eq!(
            head_flags(&on_disk.json, on_disk.compile_findings),
            want_head,
            "{} head",
            case.name
        );
        expected += ["format_too_new", "root", "settings", "compile"]
            .iter()
            .filter(|k| flag(k))
            .count();
        *per_category.entry(case.category.clone()).or_default() += expected;
        if case.mem {
            let bundle = rec.into_bundle(std::slice::from_ref(&project));
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            let again =
                dump(&mem, &project).unwrap_or_else(|e| panic!("{}: the bundle was not enough: {e}", case.name));
            assert_eq!(again.json, on_disk.json, "{} read through its bundle", case.name);
            assert_eq!(again.compile_findings, on_disk.compile_findings, "{}", case.name);
        }
    }
    let mins = [
        ("unknown-key", 5),
        ("wrong-type", 5),
        ("backwards-window", 2),
        ("bad-notes", 3),
        ("bad-marker-key", 1),
        ("format-too-new", 1),
        ("empty-file", 3),
        ("syntax-error", 5),
        ("non-utf8", 3),
        ("hostile", 8),
        ("good", 0),
        ("mod-unknown-key", 1),
        ("mod-wrong-type", 3),
        ("mod-widget", 7),
        ("mod-regex", 8),
        ("mod-tabs", 3),
        ("mod-rack", 3),
        ("mod-plugin-name", 2),
        ("mod-library", 3),
        ("mod-library-node", 2),
        ("mod-engine", 3),
        ("mod-scan", 4),
        ("mod-dot", 2),
        ("mod-workspace", 2),
        ("mod-category", 2),
        ("mod-utf8", 2),
        ("mod-meta", 1),
        ("mod-chain", 1),
    ];
    let routing: Vec<Case> = cases().into_iter().filter(|c| c.routing).collect();
    assert!(routing.len() >= 40, "{} routing cases", routing.len());
    assert!(
        routing.iter().filter(|c| c.ts).count() >= 30,
        "routing cases the TypeScript reads"
    );
    let resolve_cases = per_category.keys().filter(|c| c.starts_with("res-")).count();
    assert!(resolve_cases >= 19, "{resolve_cases} resolution cases");
    for (category, min) in mins {
        let got = per_category.get(category).copied();
        assert!(
            got.is_some_and(|n| n >= min),
            "category {category}: {got:?} findings, at least {min} expected"
        );
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// One module case: its files laid out under a base, read through a Recorder, then through the bundle; what it expects holds.
fn module_case(t: &Tmp, case: &Case) -> usize {
    let base = t.0.join(&case.name);
    for (rel, bytes) in &case.files {
        let path = base.join(placed(rel));
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        std::fs::write(path, bytes).expect("write the file");
    }
    std::fs::create_dir_all(base.join("ws/proj")).expect("the project");
    let roots = case_roots(&base, case.workspace_namespace.as_deref());
    let (mounts, files) = roots.mounts().expect("mounts");
    let disk = DiskSource::with_files(&mounts, &files).expect("a source");
    let rec = Recorder::new(&disk);
    let got = dump_modules(&rec, &mut Cache::new(), &roots).unwrap_or_else(|e| panic!("{}: {e}", case.name));
    let v = &got.json;
    let sorted = |k: &str| {
        let mut x = strings(&case.expect[k]);
        x.sort();
        x
    };
    assert_eq!(strings(&v["nodes"]["files"]), sorted("nodes"), "{} nodes", case.name);
    assert_eq!(
        strings(&v["vocab"]["findings"]),
        sorted("library"),
        "{} library",
        case.name
    );
    if !case.routing {
        assert_eq!(
            got.chain,
            case.expect["chain"].as_bool().unwrap_or(false),
            "{} chain",
            case.name
        );
    }
    for r in strings(&case.expect["refs_present"]) {
        assert!(v["vocab"]["modules"].get(&r).is_some(), "{}: {r} is missing", case.name);
    }
    for r in strings(&case.expect["refs_absent"]) {
        assert!(v["vocab"]["modules"].get(&r).is_none(), "{}: {r} is there", case.name);
    }
    if let Some(builtin) = case.expect["scan_builtin"].as_bool() {
        assert_eq!(v["scan_builtin"], builtin, "{} scan", case.name);
    }
    check_resolution(case, v, &spell(&base));
    if case.routing {
        check_routing(case, v);
    }
    // The same read through the bundle it made, with a cache of its own: the browser's.
    if case.mem {
        let bundle = rec.into_bundle_with_files(&mounts, &files);
        let mem = MemSource::from_bundle(bundle).expect("a bundle");
        let again = dump_modules(&mem, &mut Cache::new(), &roots)
            .unwrap_or_else(|e| panic!("{}: the bundle was not enough: {e}", case.name));
        assert_eq!(again.json, got.json, "{} read through its bundle", case.name);
    }
    let c = &case.expect;
    (strings(&c["nodes"]).len()
        + strings(&c["library"]).len()
        + strings(&c["refs_absent"]).len()
        + strings(&c["refs_present"]).len())
    .max(1)
}

#[test]
fn a_hostile_notes_string_is_refused_quickly() {
    // Limits count what is laid, not what is written: `!100` three deep shares one step and lays a million notes.
    let started = std::time::Instant::now();
    for src in [
        "[[[c4!100]!100]!100]!100",
        "c4!99999999999999999999",
        "c999999999",
        "c-999999999",
        "c4@0 d4@0",
    ] {
        assert!(song::files::expand_notes(src, 1.0, None).is_err(), "{src}");
    }
    let deep = format!("{}c4{}", "[".repeat(100), "]".repeat(100));
    assert!(song::files::expand_notes(&deep, 1.0, None).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
}

/// What a resolution case expects: exact file sets (never messages), the refs stamped, and a few resolved values.
fn check_resolution(case: &Case, v: &Value, base: &str) {
    let e = &case.expect;
    let set = |x: &Value| {
        let mut s = strings(x);
        s.sort();
        s
    };
    let ours = |x: &Value| set(x);
    for (key, got) in [
        ("panels", &v["resolve"]["panels"]),
        ("maps", &v["resolve"]["maps"]),
        ("plugin", &v["resolved"]["plugin"]),
        ("resolve_findings", &v["resolve"]["findings"]),
        ("resolve_vocab_findings", &v["resolve"]["vocab_findings"]),
        ("node_warnings", &v["resolve"]["node_warnings"]),
    ] {
        if e.get(key).is_some() {
            assert_eq!(ours(got), set(&e[key]), "{} {key}", case.name);
        }
    }
    for (r, why) in e["absent"].as_object().into_iter().flatten() {
        assert_eq!(v["resolve"]["absent"][r], *why, "{} absent {r}", case.name);
    }
    for (at, want) in e["resolved"].as_object().into_iter().flatten() {
        let (part, key) = at.split_once(':').unwrap_or(("nodes", at));
        let module = &v["resolved"][part][key];
        assert!(module.is_object(), "{}: {at} was not resolved", case.name);
        for (pointer, value) in want.as_object().into_iter().flatten() {
            let expected = match value {
                Value::String(s) => json!(s.replace("$base", base)),
                other => other.clone(),
            };
            let got = module.pointer(pointer).cloned().unwrap_or(Value::Null);
            let same = match (got.as_f64(), expected.as_f64()) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                _ => got == expected,
            };
            assert!(same, "{} {at} {pointer}: {got} is not {expected}", case.name);
        }
    }
}

/// What a routing case expects: exact file sets (never messages), segments, levels and derived chains as pointers.
fn check_routing(case: &Case, v: &Value) {
    let e = &case.expect;
    let r = &v["routing"];
    let set = |x: &Value| {
        let mut s = strings(x);
        s.sort();
        s
    };
    for (key, got) in [
        ("routing_files", &r["files"]),
        ("node_files", &v["nodes"]["labels"]),
        ("chain_files", &r["chains"]),
        ("hidden", &r["hidden"]),
    ] {
        if e.get(key).is_some() {
            assert_eq!(set(got), set(&e[key]), "{} {key}", case.name);
        }
    }
    if let Some(capped) = e["capped"].as_bool() {
        assert_eq!(r["capped"], capped, "{} capped", case.name);
    }
    if e.get("levels").is_some() {
        assert_eq!(r["levels"], e["levels"], "{} levels", case.name);
    }
    for (part, root) in [
        ("segments", &r["segments"]),
        ("derived", &r["derived"]),
        ("rnodes", &r["nodes"]),
    ] {
        for (pointer, want) in e[part].as_object().into_iter().flatten() {
            let got = root.pointer(pointer).cloned().unwrap_or(Value::Null);
            let same = match (got.as_f64(), want.as_f64()) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                _ => got == *want,
            };
            assert!(same, "{} {part} {pointer}: {got} is not {want}", case.name);
        }
    }
}
