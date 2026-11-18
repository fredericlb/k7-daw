// resolve_extends.rs — a module's ancestry folded: moduleResolve.test.ts, and decisions 53, 54, 56, 57 on the guard and the budget.
// A reader would get wrong that a cycle is not an error: the module resolves with the ancestry folded so far and a warning says so.

use crate::support;

use indexmap::IndexMap;
use song::files::Severity;
use song::modules::{LoadedModule, ParamValue};
use song::resolve::{InstrumentType, OWN_ONLY, resolve_map_with, resolved_rack, resolved_type, resolved_ui};
use support::{by_ref, mem, mem_world_roots, module_of, node_of, resolve, resolve_one};

fn params(m: &LoadedModule) -> Vec<(String, f64)> {
    m.authored
        .params
        .iter()
        .flatten()
        .map(|(k, v)| match v {
            ParamValue::Number(n) => (k.clone(), *n),
            ParamValue::Text(_) => (k.clone(), f64::NAN),
        })
        .collect()
}

// moduleResolve.test.ts, "resolvedType": plugin unless the module says rack, and what it inherited counts.
#[test]
fn the_type_is_plugin_unless_the_module_or_its_base_says_rack() {
    assert_eq!(resolved_type(None), InstrumentType::Plugin);
    let kit = [("@library/kit", "type(\"rack\")\n")];
    assert_eq!(
        resolved_type(Some(&resolve_one("extends(\"@library/kit\")\n", &kit))),
        InstrumentType::Rack
    );
    let own = "extends(\"@library/kit\")\ntype(\"plugin\")\n";
    assert_eq!(resolved_type(Some(&resolve_one(own, &kit))), InstrumentType::Plugin);
}

// moduleResolve.test.ts, "resolvedRack": symbols merge over the base's, the module wins on a shared one, the flavour is inherited.
#[test]
fn a_rack_merges_symbols_per_entry_and_inherits_its_flavour() {
    let kit = [(
        "@library/kit",
        "type(\"rack\")\nrack({ rack_type: \"samples\", symbols: { BD: { note: 36, clip: \"kick\" } } })\n",
    )];
    let merged = resolve_one(
        "extends(\"@library/kit\")\nrack({ symbols: { HH: { note: 42, clip: \"hat\" } } })\n",
        &kit,
    );
    let rack = resolved_rack(Some(&merged));
    assert_eq!(
        rack.symbols.keys().map(String::as_str).collect::<Vec<_>>(),
        ["BD", "HH"]
    );
    let wins = resolve_one(
        "extends(\"@library/kit\")\nrack({ symbols: { BD: { note: 35, clip: \"kick2\" } } })\n",
        &kit,
    );
    let r = resolved_rack(Some(&wins));
    assert_eq!(r.symbols.len(), 1);
    assert_eq!(r.symbols["BD"].note.0, 35);
    let slice = [(
        "@library/loop",
        "type(\"rack\")\nrack({ rack_type: \"slice\", source: \"loop\", symbols: { A: { note: 60, from: 0, to: 0.5 } } })\n",
    )];
    let inherited = resolved_rack(Some(&resolve_one("extends(\"@library/loop\")\n", &slice)));
    assert_eq!(format!("{:?}", inherited.rack_type), "Slice");
    assert_eq!(inherited.symbols["A"].note.0, 60);
    let empty = resolved_rack(None);
    assert!(empty.symbols.is_empty() && empty.source.is_none());
}

// moduleResolve.test.ts, "resolvedUI": undefined without a panel; widgets merge per id, the child's grid wins, hidden ones are dropped.
#[test]
fn a_panel_merges_per_widget_and_drops_the_hidden() {
    assert!(resolved_ui(None).is_none());
    assert!(resolved_ui(Some(&resolve_one("", &[]))).is_none());
    let amp = [(
        "@library/amp",
        "ui({ cols: 2, rows: 2, widgets: { a: { type: \"knob\", col: 0, row: 0, label: \"BP\" } } })\n",
    )];
    let child = "extends(\"@library/amp\")\nui({ cols: 3, rows: 1, widgets: { a: { type: \"knob\", col: 1, row: 1, label: \"OWN\" }, b: { type: \"toggle\", col: 0, row: 1, hidden: true } } })\n";
    let ui = resolved_ui(Some(&resolve_one(child, &amp))).expect("a panel");
    assert_eq!((ui.cols, ui.rows), (3, 1));
    assert_eq!(ui.widgets["a"].label.as_deref(), Some("OWN"));
    assert!(!ui.widgets.contains_key("b"));
}

// moduleResolve.test.ts, "merges params per key, deepest ancestor first".
#[test]
fn params_merge_per_key_deepest_first() {
    let refs = [
        ("@library/a", "params({ a: 1, b: 2, c: 3 })\n"),
        ("@library/b", "extends(\"@library/a\")\nparams({ b: 20 })\n"),
    ];
    let m = resolve_one("extends(\"@library/b\")\nparams({ c: 30 })\n", &refs);
    assert_eq!(params(&m), [("a".into(), 1.0), ("b".into(), 20.0), ("c".into(), 30.0)]);
}

// moduleResolve.test.ts, "keeps an inherited asset the loader wrote as undefined on the child" (decision 57).
#[test]
fn a_child_without_a_file_keeps_the_one_it_inherits() {
    let mut refs = by_ref(&[("@k7-core/audio/amp", "")]);
    let amp = refs.get_mut("@k7-core/audio/amp").expect("amp");
    amp.model = Some("/mods/amp/assets/jcm800.nam".into());
    amp.preset = Some("/mods/amp/p.vstpreset".into());
    amp.assets = Some([("model".to_owned(), "jcm800.nam".to_owned())].into_iter().collect());
    let node: IndexMap<String, LoadedModule> = [("n".to_owned(), node_of("n", "extends(\"@k7-core/audio/amp\")\n"))]
        .into_iter()
        .collect();
    let out = resolve(&node, &refs).modules.shift_remove("n").expect("resolved");
    assert_eq!(out.model.as_deref(), Some("/mods/amp/assets/jcm800.nam"));
    assert_eq!(out.preset.as_deref(), Some("/mods/amp/p.vstpreset"));
    assert_eq!(out.assets.as_ref().map(IndexMap::len), Some(1));
}

// moduleResolve.test.ts, "never inherits what says WHERE the module lives": the category and the ports are the child's own.
#[test]
fn what_says_where_a_module_lives_is_never_inherited() {
    let mut refs = by_ref(&[("@library/synth", "inputs([{ name: \"midi\", type: \"midi\" }])\n")]);
    refs["@library/synth"].category = Some("instruments".into());
    let mut node = module_of(
        "audio",
        "n",
        "extends(\"@library/synth\")\ninputs([{ name: \"in\", type: \"audio\" }])\n",
    );
    node.category = Some("audio".into());
    let modules: IndexMap<String, LoadedModule> = [("n".to_owned(), node)].into_iter().collect();
    let out = resolve(&modules, &refs).modules.shift_remove("n").expect("resolved");
    assert_eq!(out.category.as_deref(), Some("audio"));
    assert_eq!(out.inputs()[0].name, "in");
}

// Decision 57: the identity keys are the child's by presence, an absent `node` included; no producer leaves one out.
#[test]
fn a_childs_identity_is_never_the_parents() {
    assert_eq!(
        OWN_ONLY,
        ["name", "file", "node", "home", "chain", "nodeId", "presetRel"]
    );
    let mut refs = by_ref(&[("@library/parent", "")]);
    let parent = refs.get_mut("@library/parent").expect("parent");
    parent.node = Some("a-node".into());
    parent.name = Some("parent-name".into());
    parent.home = Some("nodes".into());
    let child = module_of("audio", "child", "extends(\"@library/parent\")\n");
    let modules: IndexMap<String, LoadedModule> = [("n".to_owned(), child)].into_iter().collect();
    let out = resolve(&modules, &refs).modules.shift_remove("n").expect("resolved");
    assert_eq!(
        (out.node.as_deref(), out.name.as_deref(), out.home.as_deref()),
        (None, None, None)
    );
    assert_eq!(out.file, "child");
}

// moduleResolve.test.ts, "stops at a cycle rather than throwing", and decision 54: the same, with a warning at the module's file.
#[test]
fn a_cycle_stops_the_fold_with_a_warning_and_keeps_the_ancestry_so_far() {
    let refs = by_ref(&[
        ("@x/audio/a", "extends(\"@x/audio/b\")\nparams({ x: 1 })\n"),
        ("@x/audio/b", "extends(\"@x/audio/a\")\nparams({ y: 2 })\n"),
    ]);
    let out = resolve(&refs, &refs);
    assert_eq!(
        params(&out.modules["@x/audio/a"]),
        [("y".into(), 2.0), ("x".into(), 1.0)]
    );
    let a = out
        .findings
        .iter()
        .filter(|f| f.file == "@x/audio/a/module.k7x")
        .count();
    assert_eq!(a, 1, "{:?}", out.findings);
    assert!(out.findings.iter().all(|f| f.severity == Severity::Warning));
    // A self-extends is the shortest cycle.
    let me = by_ref(&[("@x/audio/me", "extends(\"@x/audio/me\")\nparams({ z: 1 })\n")]);
    let out = resolve(&me, &me);
    assert_eq!(out.findings.len(), 1);
    assert_eq!(params(&out.modules["@x/audio/me"]), [("z".into(), 1.0)]);
}

// Decision 54: a chain of 1,000 levels stops at the 32nd, with a warning, and costs about that many merges.
#[test]
fn a_chain_past_32_levels_stops_at_the_32nd() {
    let started = std::time::Instant::now();
    let mut entries: Vec<(String, String)> = Vec::new();
    for i in 0..1000 {
        let ext = if i + 1 < 1000 {
            format!("extends(\"@x/audio/m{}\")\n", i + 1)
        } else {
            String::new()
        };
        entries.push((format!("@x/audio/m{i}"), format!("{ext}params({{ p{i}: {i} }})\n")));
    }
    let borrowed: Vec<(&str, &str)> = entries.iter().map(|(r, t)| (r.as_str(), t.as_str())).collect();
    let refs = by_ref(&borrowed);
    let out = resolve(&refs, &refs);
    let m0 = &out.modules["@x/audio/m0"];
    assert_eq!(params(m0).len(), 33, "itself and 32 ancestors");
    assert!(
        out.findings
            .iter()
            .any(|f| f.file == "@x/audio/m0/module.k7x" && f.severity == Severity::Warning)
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{:?}",
        started.elapsed()
    );
}

// moduleResolve.test.ts, "leaves a module whose base does not exist alone", and decision 55: Unknown, or Broken with the file.
#[test]
fn a_base_that_does_not_exist_leaves_the_module_unmerged() {
    let m = resolve_one("extends(\"@library/gone\")\nparams({ a: 1 })\n", &[]);
    assert_eq!(params(&m), [("a".into(), 1.0)]);
    assert_eq!(m.authored.extends.as_deref(), Some("@library/gone"));
}

// moduleResolve.test.ts, "mergeAuthored takes the child's value, including one it sets to a falsy".
#[test]
fn a_falsy_child_value_is_still_a_value() {
    let m = resolve_one(
        "extends(\"@library/p\")\ngain(0)\nsource(\"\")\n",
        &[("@library/p", "gain(3)\nsource(\"loop\")\n")],
    );
    assert_eq!(m.authored.gain, Some(0.0));
    assert_eq!(m.authored.source.as_deref(), Some(""));
}

// Decision 56: a fan-out is weighed by its OUTPUT; the budget stops the call with one error at the module it was resolving.
#[test]
fn a_fan_out_over_one_big_parent_stops_at_the_budget() {
    let started = std::time::Instant::now();
    let big: String = (0..1000).map(|i| format!("p{i}: {i}, ")).collect();
    let parent = format!("params({{ {big} }})\n");
    let mut refs = by_ref(&[("@x/audio/parent", &parent)]);
    let mut nodes: IndexMap<String, LoadedModule> = IndexMap::new();
    for i in 0..10_000 {
        nodes.insert(
            format!("n{i}"),
            node_of(&format!("n{i}"), "extends(\"@x/audio/parent\")\n"),
        );
    }
    refs.insert("@x/audio/other".into(), module_of("audio", "other", ""));
    let src = mem(&[]);
    let out = resolve_map_with(&src, &mem_world_roots(), &nodes, &refs, 1_000_000).expect("resolution");
    assert!(out.capped);
    assert!(
        out.modules.len() < 1100 && out.modules.len() > 100,
        "{}",
        out.modules.len()
    );
    let errors: Vec<_> = out.findings.iter().filter(|f| f.severity == Severity::Error).collect();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].file.starts_with("nodes/n"), "{}", errors[0].file);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{:?}",
        started.elapsed()
    );
}

// Decision 56: 32 levels each adding 3,000 widgets under 100 nodes would be a hundred million copies; the first module stops it.
#[test]
fn a_widget_blow_up_through_32_levels_stops_at_the_budget() {
    let widgets = |level: usize| -> String {
        (0..3000)
            .map(|i| format!("l{level}w{i}: {{ type: \"knob\", col: 0, row: 0 }}, "))
            .collect()
    };
    let mut entries: Vec<(String, String)> = Vec::new();
    for i in 0..32 {
        let ext = if i + 1 < 32 {
            format!("extends(\"@x/audio/m{}\")\n", i + 1)
        } else {
            String::new()
        };
        entries.push((
            format!("@x/audio/m{i}"),
            format!("{ext}ui({{ cols: 1, rows: 1, widgets: {{ {} }} }})\n", widgets(i)),
        ));
    }
    let borrowed: Vec<(&str, &str)> = entries.iter().map(|(r, t)| (r.as_str(), t.as_str())).collect();
    let refs = by_ref(&borrowed);
    let mut nodes: IndexMap<String, LoadedModule> = IndexMap::new();
    for i in 0..100 {
        nodes.insert(format!("n{i}"), node_of(&format!("n{i}"), "extends(\"@x/audio/m0\")\n"));
    }
    let started = std::time::Instant::now();
    let out = resolve_map_with(&mem(&[]), &mem_world_roots(), &nodes, &refs, 1_000_000).expect("resolution");
    assert!(out.capped && out.modules.is_empty(), "{}", out.modules.len());
    assert_eq!(out.findings.iter().filter(|f| f.severity == Severity::Error).count(), 1);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{:?}",
        started.elapsed()
    );
}

// Decision 56 and the shared weight: a list a merge copies counts, alternatives included.
#[test]
fn nodes_over_a_parent_with_10000_alternatives_stop_at_the_budget() {
    let started = std::time::Instant::now();
    let alts: String = (0..10_000)
        .map(|i| format!("{{ name: \"a{i}\", params: {{ x: 1 }} }},"))
        .collect();
    let refs = by_ref(&[("@x/audio/parent", &format!("alternatives([{alts}])\n"))]);
    let mut nodes: IndexMap<String, LoadedModule> = IndexMap::new();
    for i in 0..10_000 {
        nodes.insert(
            format!("n{i}"),
            node_of(&format!("n{i}"), "extends(\"@x/audio/parent\")\n"),
        );
    }
    let out = resolve_map_with(&mem(&[]), &mem_world_roots(), &nodes, &refs, 1_000_000).expect("resolution");
    assert!(out.capped && out.modules.len() < 200, "{}", out.modules.len());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{:?}",
        started.elapsed()
    );
}

fn json_of(out: &song::Resolution) -> (serde_json::Value, Vec<(String, Severity)>) {
    let mut f: Vec<(String, Severity)> = out.findings.iter().map(|f| (f.file.clone(), f.severity)).collect();
    f.sort_by(|a, b| a.0.cmp(&b.0));
    (serde_json::to_value(&out.modules).expect("json"), f)
}

// Decision 74: a kept fold is reused only where walking gives the same answer, so the order of a map decides nothing.
#[test]
fn a_chain_of_100_levels_answers_the_same_in_either_order() {
    let entries: Vec<(String, String)> = (0..100)
        .map(|i| {
            let ext = if i < 99 {
                format!("extends(\"@x/audio/m{}\")\n", i + 1)
            } else {
                String::new()
            };
            (format!("@x/audio/m{i}"), format!("{ext}params({{ p{i}: {i} }})\n"))
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = entries.iter().map(|(r, t)| (r.as_str(), t.as_str())).collect();
    let forward = by_ref(&borrowed);
    let reversed: IndexMap<String, LoadedModule> = forward.iter().rev().map(|(k, v)| (k.clone(), v.clone())).collect();
    let (a, b) = (resolve(&forward, &forward), resolve(&reversed, &forward));
    let (mut ja, mut jb) = (json_of(&a), json_of(&b));
    ja.1.dedup();
    jb.1.dedup();
    assert_eq!(ja.0.as_object().map(|o| o.len()), jb.0.as_object().map(|o| o.len()));
    for (k, v) in ja.0.as_object().expect("modules") {
        assert_eq!(&jb.0[k.as_str()], v, "{k}");
    }
    assert_eq!(ja.1, jb.1);
    assert_eq!(params(&a.modules["@x/audio/m0"]).len(), 33);
    assert_eq!(params(&b.modules["@x/audio/m0"]).len(), 33);
}

// Decision 74: K swaps into T and T extends K: K's answer is the same whichever of the two is resolved first.
#[test]
fn a_swap_into_a_module_that_extends_the_swapper_answers_the_same_in_either_order() {
    let entries = [
        (
            "@x/audio/k",
            "params({ k: 1 })\nalternatives([{ name: \"s\", extends: \"@x/audio/t\" }])\nalternative(\"s\")\n",
        ),
        ("@x/audio/t", "extends(\"@x/audio/k\")\nparams({ t: 2 })\n"),
    ];
    let forward = by_ref(&entries);
    let reversed: IndexMap<String, LoadedModule> = forward.iter().rev().map(|(k, v)| (k.clone(), v.clone())).collect();
    let (a, b) = (resolve(&forward, &forward), resolve(&reversed, &forward));
    assert_eq!(params(&a.modules["@x/audio/k"]), [("t".into(), 2.0)]);
    assert_eq!(params(&b.modules["@x/audio/k"]), [("t".into(), 2.0)]);
    assert_eq!(params(&a.modules["@x/audio/t"]), params(&b.modules["@x/audio/t"]));
}

// A ref resolved alone equals the same ref in a map.
#[test]
fn resolve_ref_equals_the_map() {
    let files = [
        (
            "lib/k7-core/modules/audio/a/module.k7x",
            "extends(\"@k7-core/audio/b\")\nparams({ a: 1 })\n",
        ),
        ("lib/k7-core/modules/audio/b/module.k7x", "params({ b: 2 })\n"),
    ];
    let src = mem(&files);
    let w = support::world(&src, &mut song::Cache::new(), &mem_world_roots()).expect("a world");
    let (one, findings) =
        song::resolve_ref(&src, &mem_world_roots(), &w.vocab, "@k7-core/audio/a").expect("resolution");
    assert_eq!(one.as_ref(), w.resolved_vocab.modules.get("@k7-core/audio/a"));
    assert!(findings.is_empty());
    assert!(
        song::resolve_ref(&src, &mem_world_roots(), &w.vocab, "@k7-core/audio/nope")
            .expect("none")
            .0
            .is_none()
    );
}
