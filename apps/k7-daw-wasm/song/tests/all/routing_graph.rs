// routing_graph.rs — `graph.k7x` decoded into Segments: runs expanded, strict, every finding on its line (decisions 88, 90, 110).
// A reader would get wrong that a test here is the TypeScript's: each names its source case and the decision it holds.

use song::routing::decode_graph;

const AUTHORED: &str = "// The signal graph, as segments.\nmodules({\n  \"vo.jo\": [\"eq_dj:vox\"],\n  \"da.main\": [\"drum_1\"],\n  \"eq_dj:vox\": [\"OUT\"],\n  \"amp:bass\": [\"cabinet\"],\n  cabinet: [\"OUT\"],\n})\nbypassed([\"eq_dj:vox\"])\n";

fn modules(text: &str) -> Vec<(String, Vec<String>)> {
    let s = decode_graph("graph.k7x", text).expect("a graph");
    s.modules.expect("modules").into_iter().collect()
}

fn dests(pairs: &[(String, Vec<String>)], key: &str) -> Vec<String> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

#[test]
fn reads_nodes_destinations_and_bypass() {
    // graphYaml.test.ts "reads nodes, destinations and bypass": a lane and a module are one kind of node (88).
    let s = decode_graph("graph.k7x", AUTHORED).expect("a graph");
    let m = s.modules.as_ref().expect("modules");
    assert_eq!(m["vo.jo"], ["eq_dj:vox"]);
    assert_eq!(m["amp:bass"], ["cabinet"]);
    assert_eq!(s.bypassed.as_deref(), Some(&["eq_dj:vox".to_owned()][..]));
    assert!(s.bus_only.is_none() && s.tap_feeds.is_none() && s.positions.is_none());
}

#[test]
fn an_unknown_section_is_refused_on_its_line() {
    // graphYaml.test.ts "rejects an unknown section, on its line" (88).
    let f = decode_graph("graph.k7x", "modules({ a: [\"b\"] })\nbusses({ c: [\"d\"] })\n").expect_err("refused");
    assert_eq!((f.line, f.col), (2, 1));
}

#[test]
fn a_syntax_error_is_placed() {
    // graphYaml.test.ts "reports a syntax error with its line and column" (110).
    let f = decode_graph("graph.k7x", "modules({ a: [\"b\" })\n").expect_err("refused");
    assert_eq!(f.line, 1);
}

#[test]
fn runs_expand_into_segments() {
    // graphYaml.test.ts "expands runs into segments" (90).
    let m = modules("runs([[\"a\", \"b\", \"OUT\"]])\n");
    assert_eq!(
        m,
        [
            ("a".to_owned(), vec!["b".to_owned()]),
            ("b".to_owned(), vec!["OUT".to_owned()])
        ]
    );
}

#[test]
fn runs_are_added_after_what_modules_already_says() {
    // sugar.test.ts "adds each run's segments after what modules already says" (90).
    let m = modules("modules({ MIX: [\"OUT\"] })\nruns([[\"melody\", \"lead\", \"MIX\"], [\"pad\", \"MIX\"]])\n");
    let keys: Vec<&str> = m.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["MIX", "melody", "lead", "pad"]);
    assert_eq!(dests(&m, "lead"), ["MIX"]);
}

#[test]
fn a_run_duplicating_modules_is_deduplicated_but_modules_own_duplicates_stay() {
    // Decision 90: `includes` against the list, the list's own duplicates kept.
    let m = modules("modules({ a: [\"b\", \"b\"] })\nruns([[\"a\", \"b\", \"c\"]])\n");
    assert_eq!(dests(&m, "a"), ["b", "b"]);
    assert_eq!(dests(&m, "b"), ["c"]);
}

#[test]
fn a_runs_that_does_not_read_is_named_on_runs() {
    // graphYaml.test.ts "names a runs that does not read" (90): the error is on the `runs` key.
    for bad in [
        "runs([\"a\"])\n",
        "runs(\"a\")\n",
        "runs([[\"a\"]])\n",
        "runs([[\"a\", 1]])\n",
    ] {
        let f = decode_graph("graph.k7x", bad).expect_err(bad);
        assert_eq!((f.line, f.col), (1, 1), "{bad}");
    }
}

#[test]
fn modules_that_is_no_object_beside_runs_is_refused() {
    // Decision 90 DEPARTS: `expandGraph` drops it, a graph whose `modules` is a number is no graph.
    assert!(decode_graph("graph.k7x", "modules(3)\nruns([[\"a\", \"b\"]])\n").is_err());
}

#[test]
fn the_other_sections_are_strict() {
    for bad in [
        "modules({ a: [\"#x\"] })\n",
        "modules({ a: [\"x#\"] })\n",
        "modules({ \"\": [\"x\"] })\n",
        "modules({ a: \"x\" })\n",
        "positions({ a: { x: 1, y: 2, z: 3 } })\n",
        "positions({ a: { x: \"1\", y: 2 } })\n",
        "positions({ a: { x: 1e400, y: 2 } })\n",
        "tap_feeds([{ node: \"a\", tap: \"t\" }])\n",
        "bypassed([\"\"])\n",
    ] {
        assert!(decode_graph("graph.k7x", bad).is_err(), "{bad}");
    }
}

#[test]
fn positions_and_tap_feeds_are_read() {
    let s = decode_graph(
        "graph.k7x",
        "modules({ a: [\"b\"] })\ntap_feeds([{ node: \"s\", tap: \"kick\", to: \"b\" }])\npositions({ a: { x: 1, y: -2.5 } })\nbus_only([\"b\"])\n",
    )
    .expect("a graph");
    assert_eq!(s.tap_feeds_of()[0].tap, "kick");
    assert_eq!(s.positions.as_ref().expect("positions")["a"].y, -2.5);
    assert_eq!(s.bus_only_of().collect::<Vec<_>>(), ["b"]);
}

#[test]
fn a_chain_list_is_an_unknown_key() {
    // Decision 88: the legacy `chains:` list is dropped, so it is an unknown key like any other.
    assert!(decode_graph("graph.k7x", "chains([])\n").is_err());
}

#[test]
fn a_runs_first_name_is_a_key_and_only_destinations_are_port_refs() {
    // Decision 90: `expandGraph` takes any string, and `modules` accepts a `#x` key; only a destination is checked.
    for first in ["#x", "x#"] {
        let m = modules(&format!("runs([[\"{first}\", \"b\"]])\n"));
        assert_eq!(dests(&m, first), ["b"], "{first}");
    }
    for bad in [
        "runs([[\"a\", \"#b\"]])\n",
        "runs([[\"a\", \"b#\"]])\n",
        "runs([[\"\", \"b\"]])\n",
    ] {
        assert!(decode_graph("graph.k7x", bad).is_err(), "{bad}");
    }
}

#[test]
fn a_bad_runs_is_located_on_runs_whatever_else_the_file_holds() {
    // Decision 90: `runs` is validated first, so an unknown key or a bad `modules` does not take its place.
    let f = decode_graph("graph.k7x", "busses(1)\nruns(\"a\")\n").expect_err("refused");
    assert_eq!(f.line, 2);
    let f = decode_graph("graph.k7x", "modules(3)\nruns([[\"a\"]])\n").expect_err("refused");
    assert_eq!(f.line, 2);
    let f = decode_graph("graph.k7x", "busses(1)\nruns([[\"a\", \"b\"]])\n").expect_err("refused");
    assert_eq!(f.line, 1);
}
