// routing_chain_file.rs — a chain file decoded: steps by shape, items, links, subchains, every refinement (decisions 91 to 94, 113).
// A reader would get wrong that a file reports ONE finding: each case asserts the line it lands on, never a message.

use song::PathRule;
use song::routing::{ChainFile, Item, Start, Step, decode_chain};

fn ok(text: &str) -> ChainFile {
    decode_chain("chains/a.k7x", text, PathRule::HOST)
        .unwrap_or_else(|f| panic!("{text}: {}:{} {}", f.line, f.col, f.message))
}

fn refused(text: &str) -> (u32, u32) {
    let f = decode_chain("chains/a.k7x", text, PathRule::HOST).expect_err(text);
    (f.line, f.col)
}

const GUITAR: &str = "// the guitar chain\nstart(true)\nsteps([\n  id(\"in\").extends(\"@k7-core/io/in\"),\n  // the drive\n  id(\"drive\").extends(\"@k7-core/audio/overdrive\").params({ Amount: 5 }),\n  id(\"verb\").extends(\"@k7-core/audio/reverb\"),\n  to([chain(\"OUT\")]),\n])\n";

#[test]
fn reads_a_chain_written_as_k7x() {
    // chainFile.test.ts "reads a chain written as k7x" (91).
    let c = ok(GUITAR);
    assert_eq!(c.start, Some(Start::Own));
    assert_eq!(c.steps.len(), 4);
    assert!(matches!(&c.steps[3], Step::End(e) if e.to.len() == 1 && e.to[0].chain == "OUT"));
    assert_eq!(c.steps[1].items()[0].id(), "drive");
}

#[test]
fn checks_a_node_item_as_a_module_file_at_the_items_line() {
    // chainFile.test.ts "checks a node item as a module file, at the item's line" (92, 94).
    let text = "start(true)\nsteps([\n  id(\"in\").extends(\"@k7-core/io/in\"),\n  id(\"x\").paramz({}),\n])\n";
    assert_eq!(refused(text).0, 4);
}

#[test]
fn reports_a_syntax_error_with_its_line() {
    // chainFile.test.ts "reports a syntax error with its line".
    assert!(refused("steps([\n").0 >= 1);
}

#[test]
fn a_step_is_told_by_its_shape() {
    // Decision 91: a list is parallel, `to` an end, `sub` without `id` a call, `track` within three keys a lane.
    let c = ok(
        "start(true)\nsteps([id(\"l\").track(\"t\"), [id(\"a\").extends(\"x\"), id(\"b\").extends(\"y\")], {sub: \"s\"}, to([chain(\"OUT\")])])\nsubchains([{ id: \"s\", steps: [id(\"d\").extends(\"w\")] }])\n",
    );
    assert!(matches!(c.steps[0].items()[0], Item::Track { .. }));
    assert!(matches!(c.steps[1], Step::Parallel(_)));
    assert!(c.steps[2].is_call());
    assert_eq!(c.called_subchains().len(), 1);
    assert_eq!(c.expand_calls().len(), 4);
}

#[test]
fn an_item_with_a_track_beside_other_keys_is_a_node_and_fails_as_one() {
    // chainFile.ts `isTrackItem`: a lane is `{id, track, bypass}` exactly, so this is a node and `track` is no module field (91).
    refused("steps([id(\"n\").extends(\"x\").track(\"t\")])\n");
}

#[test]
fn the_wrong_steps_are_refused() {
    // Decision 91: the union's first-member rule answered "invalid input"; here each wrong step lands on its own key.
    for bad in [
        "steps([[id(\"a\").extends(\"x\")]])\n",
        "steps([\"a\"])\n",
        "steps([id(\"a\").extends(\"x\"), [[id(\"b\").extends(\"x\")], id(\"c\").extends(\"x\")]])\n",
        "steps([id(\"a\").extends(\"x\"), {to: [{chain: \"OUT\"}], id: \"z\"}])\n",
        "steps([id(\"a\").extends(\"x\"), {sub: \"s\", extends: \"x\"}])\nsubchains([{id: \"s\", steps: [id(\"d\").extends(\"w\")]}])\n",
        "steps([{sub: \"s\"}])\nsubchains([{id: \"s\", steps: [id(\"d\").extends(\"w\")]}])\n",
        "steps([id(\"a\").extends(\"x\"), {to: []}])\n",
        "steps([id(\"a\").extends(\"x\"), to([chain(\"OUT\").level(-1)])])\n",
        "steps([id(\"a\").extends(\"x\"), to([chain(\"OUT\").level(1e400)])])\n",
        "start(false)\n",
        "steps([id(\"\").extends(\"x\")])\n",
        "steps([id(\"#a\").extends(\"x\")])\n",
        "start(true)\nsteps([id(\"a\").track(\"\")])\n",
    ] {
        decode_chain("chains/a.k7x", bad, PathRule::HOST).expect_err(bad);
    }
}

#[test]
fn a_lane_name_that_leaves_tracks_is_refused() {
    // Decision 93 DEPARTS: a lane name is a file name below `tracks/`.
    for lane in ["../../x", "a/b", ".", "..", "a#"] {
        let text = format!("start(true)\nsteps([id(\"l\").track(\"{lane}\")])\n");
        decode_chain("chains/a.k7x", &text, PathRule::HOST).expect_err(lane);
    }
}

#[test]
fn several_links_each_carry_a_level() {
    // chainSchema's EndStepSchema refine (94).
    assert_eq!(
        refused("steps([id(\"a\").extends(\"x\"), to([chain(\"b\"), chain(\"c\")])])\n").0,
        1
    );
    ok("steps([id(\"a\").extends(\"x\"), to([chain(\"b\").level(0.5), chain(\"c\").level(1)])])\n");
}

#[test]
fn links_come_last_and_the_first_step_is_one_node() {
    // stepIssues (94), on the key path the TypeScript names.
    let text = "steps([\n  id(\"a\").extends(\"x\"),\n  to([chain(\"OUT\")]),\n  id(\"b\").extends(\"x\"),\n])\n";
    assert_eq!(refused(text).0, 3);
    let text = "steps([\n  [id(\"a\").extends(\"x\"), id(\"b\").extends(\"x\")],\n])\n";
    assert_eq!(refused(text).0, 2);
}

#[test]
fn a_chain_with_a_start_has_no_ports_and_only_start_true_holds_a_lane() {
    // chainSchema superRefine (94).
    assert_eq!(refused("start(true)\nports([\"a\"])\n").0, 2);
    assert_eq!(refused("steps([id(\"l\").track(\"t\")])\n").0, 1);
    ok("ports([\"a\"])\nsteps([id(\"n\").extends(\"x\")])\n");
}

fn subs(sub: &str, steps: &str) -> String {
    format!("steps([id(\"a\").extends(\"x\"){steps}])\nsubchains([{sub}])\n")
}

#[test]
fn a_subchain_links_out_only_to_a_sibling_or_a_built_in_end() {
    // chainSubchains.test.ts "link out only to a sibling or a built-in end" (94).
    let leaf = "id(\"d\").extends(\"w\")";
    ok(&subs(
        &format!(
            "{{id: \"s\", steps: [{leaf}, to([chain(\"t\")])]}}, {{id: \"t\", steps: [{leaf}, to([chain(\"MIX\")])]}}"
        ),
        "",
    ));
    refused(&subs(
        &format!("{{id: \"s\", steps: [{leaf}, to([chain(\"elsewhere\")])]}}"),
        "",
    ));
}

#[test]
fn a_subchain_holds_no_lane_and_takes_no_sinks_name_nor_a_siblings() {
    // chainSubchains.test.ts "hold no lane, and take no sink's name nor a sibling's" (94).
    let leaf = "id(\"d\").extends(\"w\")";
    refused(&subs("{id: \"s\", steps: [id(\"l\").track(\"t\")]}", ""));
    refused(&subs(&format!("{{id: \"OUT\", steps: [{leaf}]}}"), ""));
    refused(&subs(
        &format!("{{id: \"s\", steps: [{leaf}]}}, {{id: \"s\", steps: [{leaf}]}}"),
        "",
    ));
}

#[test]
fn the_calls_obey_their_rules() {
    // chainSchema's call step rules: first, unknown, twice (94); a call inside a subchain; a called one with `to`.
    let leaf = "id(\"d\").extends(\"w\")";
    let sub = format!("{{id: \"s\", steps: [{leaf}]}}");
    ok(&subs(&sub, ", {sub: \"s\"}"));
    refused(&subs(&sub, ", {sub: \"nope\"}"));
    refused(&subs(&sub, ", {sub: \"s\"}, {sub: \"s\"}"));
    refused(&subs(
        &format!("{{id: \"s\", steps: [{leaf}, to([chain(\"MIX\")])]}}"),
        ", {sub: \"s\"}",
    ));
    refused(&subs(&format!("{{id: \"s\", steps: [{leaf}, {{sub: \"s\"}}]}}"), ""));
    refused(&subs("{id: \"s\", steps: []}", ", {sub: \"s\"}"));
}

#[test]
fn vets_a_subchain_nodes_module_fields_like_any_nodes() {
    // chainSubchains.test.ts "vet a subchain node's module fields" (94).
    let text = "steps([id(\"a\").extends(\"x\")])\nsubchains([{ id: \"s\", steps: [\n  id(\"d\").paramz({}),\n] }])\n";
    assert!(decode_chain("chains/a.k7x", text, PathRule::HOST).is_err());
}

#[test]
fn input_and_mapping_are_decoded_and_read_by_nothing() {
    // Decision 113: no file in the repository writes `input(...)`, so a unit test does.
    let c =
        ok("start(true)\ninput({ channel: [3], monitor: true })\nsteps([id(\"n\").extends(\"x\").mapping(\"m\")])\n");
    assert!(c.input.is_some());
    assert!(matches!(&c.steps[0].items()[0], Item::Node(n) if n.mapping.as_deref() == Some("m")));
    refused("start(true)\ninput({ chanel: [3] })\n");
}

#[test]
fn a_nodes_plugin_is_placed_from_the_item_and_no_default_type_is_applied() {
    // Decision 92.
    let c = ok("steps([\n  id(\"n\").plugin({ type: \"internal\", name: \"4osc\" }),\n])\n");
    let Item::Node(n) = &c.steps[0].items()[0] else {
        panic!("a node")
    };
    assert_eq!(n.module.plugin_at, Some((2, 11)));
    assert!(n.module.module_type.is_none());
}

#[test]
fn a_hidden_chain_decodes_and_keeps_its_flag() {
    let c = ok("hidden(true)\nbus_only(true)\nsteps([id(\"n\").extends(\"x\")])\n");
    assert_eq!((c.hidden, c.bus_only), (Some(true), Some(true)));
}

fn k7x_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("a folder").flatten() {
        let p = e.path();
        if p.is_dir() {
            k7x_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "k7x") {
            out.push(p);
        }
    }
}

#[test]
fn every_chain_file_of_k7_core_decodes_and_some_end_at_the_out_sink() {
    // The library's own chains (a smoke test): `@out` is read here before the graph view (2k) uses it.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../k7-core/chains");
    let mut files = Vec::new();
    k7x_files(&root, &mut files);
    assert!(files.len() >= 20, "{} chain files", files.len());
    let mut at_out = 0;
    for f in &files {
        let text = std::fs::read_to_string(f).expect("a chain file");
        let c = decode_chain(&f.display().to_string(), &text, PathRule::HOST)
            .unwrap_or_else(|e| panic!("{}: {}:{} {}", f.display(), e.line, e.col, e.message));
        at_out += c
            .step_lists()
            .flat_map(|(_, steps)| steps)
            .filter_map(|s| s.end())
            .flat_map(|e| &e.to)
            .filter(|l| l.chain == "@out")
            .count();
    }
    assert!(at_out > 0, "no `@out` link in {} files", files.len());
}
