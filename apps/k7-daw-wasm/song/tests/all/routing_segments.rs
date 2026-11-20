// routing_segments.rs — a chain project's files as segments, levels, bypasses, buses and tap feeds (chainSegments.ts `toSegments`).
// A reader would get wrong that a chain is compiled only when visible: a start, a link, a stretch or a placed item.

use crate::support;

use indexmap::IndexMap;
use song::routing::{Budget, Position, to_segments};
use support::chain_files;

fn run(files: &[(&str, &str)], picture: &[&str], placed: &[&str]) -> song::routing::Built {
    let chains = chain_files(files);
    let positions: IndexMap<String, Position> = placed
        .iter()
        .map(|n| ((*n).to_owned(), Position { x: 0.0, y: 0.0 }))
        .collect();
    to_segments(&chains, &|l| picture.contains(&l), &positions, &mut Budget::default()).expect("built")
}

fn dests(b: &song::routing::Built, k: &str) -> Vec<String> {
    b.segments
        .modules
        .as_ref()
        .and_then(|m| m.get(k))
        .cloned()
        .unwrap_or_default()
}

const GTR: &str = "start(true)\nsteps([id(\"lane-gtr\").track(\"gtr\"), id(\"amp-1\").extends(\"x\"), to([chain(\"wet\").level(0.5), chain(\"MIX\").level(1)])])\nsubchains([{id: \"wet\", steps: [id(\"verb-1\").extends(\"x\"), id(\"eq-1\").extends(\"x\"), to([chain(\"MIX\")])]}])\n";
const BASS: &str = "start(true)\nsteps([id(\"b\").track(\"bass\"), to([chain(\"plate\")])])\n";
const PLATE: &str = "steps([id(\"plate-1\").extends(\"x\"), to([chain(\"MIX\")])])\n";

#[test]
fn a_link_to_a_subchain_lands_inside_its_parent_and_a_chain_outside() {
    // chainSubchains.test.ts "compiles a link to the subchain inside its parent, and to the chain of that name outside it".
    let b = run(&[("gtr", GTR), ("bass", BASS), ("plate", PLATE)], &[], &[]);
    assert_eq!(dests(&b, "amp-1"), ["verb-1", "MIX"]);
    assert_eq!(dests(&b, "verb-1"), ["eq-1"]);
    assert_eq!(dests(&b, "eq-1"), ["MIX"]);
    assert_eq!(dests(&b, "bass"), ["plate-1"]);
    assert_eq!(b.levels["amp-1>verb-1"], 0.5);
    assert_eq!(b.levels["amp-1>MIX"], 1.0);
}

#[test]
fn a_chain_nothing_links_is_hidden_unless_it_is_placed() {
    // chainProject.test.ts "a chain nothing links" (visibility, decision 88).
    let b = run(&[("gtr", GTR), ("plate", PLATE)], &[], &[]);
    assert!(dests(&b, "plate-1").is_empty());
    let b = run(&[("plate", PLATE)], &[], &["plate-1"]);
    assert_eq!(dests(&b, "plate-1"), ["MIX"]);
}

#[test]
fn a_start_at_a_sum_begins_with_it_and_a_parallel_step_fans_out() {
    let mix = "start(\"MIX\")\nsteps([id(\"m\").extends(\"x\"), [id(\"a\").extends(\"x\"), id(\"b\").extends(\"x\")], id(\"c\").extends(\"x\"), to([chain(\"OUT\")])])\n";
    let b = run(&[("m", mix)], &[], &[]);
    assert_eq!(dests(&b, "MIX"), ["m"]);
    assert_eq!(dests(&b, "m"), ["a", "b"]);
    assert_eq!(
        (dests(&b, "a"), dests(&b, "b")),
        (vec!["c".to_owned()], vec!["c".to_owned()])
    );
    assert_eq!(dests(&b, "c"), ["OUT"]);
}

#[test]
fn a_port_and_a_built_in_end_keep_their_port() {
    let c = "start(true)\nsteps([id(\"a\").extends(\"x\"), to([chain(\"RENDER\").port(\"audio\").level(1), chain(\"t\").port(\"side\").level(1)])])\n";
    let t = "ports([\"side\"])\nsteps([id(\"h\").extends(\"x\"), to([chain(\"OUT\")])])\n";
    let b = run(&[("c", c), ("t", t)], &[], &[]);
    assert_eq!(dests(&b, "a"), ["RENDER#audio", "h#side"]);
}

#[test]
fn a_link_only_chain_passes_its_input_straight_on() {
    let a = "start(true)\nsteps([id(\"a\").extends(\"x\"), to([chain(\"hub\")])])\n";
    let hub = "steps([to([chain(\"OUT\").level(1), chain(\"MIX\").level(1)])])\n";
    let b = run(&[("a", a), ("hub", hub)], &[], &[]);
    assert_eq!(dests(&b, "a"), ["OUT", "MIX"]);
}

#[test]
fn a_link_only_cycle_ends_and_a_link_to_a_hidden_chain_is_dropped() {
    let a = "start(true)\nsteps([id(\"a\").extends(\"x\"), to([chain(\"p\")])])\n";
    let p = "steps([to([chain(\"q\")])])\n";
    let q = "steps([to([chain(\"p\")])])\n";
    assert!(dests(&run(&[("a", a), ("p", p), ("q", q)], &[], &[]), "a").is_empty());
    // A hidden chain is not in the map, so nothing resolves to it.
    assert!(dests(&run(&[("a", a)], &[], &[]), "a").is_empty());
}

#[test]
fn the_bypass_and_the_bus_only_mark_come_from_visible_chains() {
    let c = "start(true)\nbus_only(true)\nsteps([id(\"a\").extends(\"x\").bypass(true), id(\"b\").extends(\"x\"), to([chain(\"OUT\")])])\n";
    let b = run(&[("c", c)], &[], &[]);
    assert_eq!(b.segments.bypassed.as_deref(), Some(&["a".to_owned()][..]));
    assert_eq!(b.segments.bus_only.as_deref(), Some(&["a".to_owned()][..]));
}

#[test]
fn a_tap_routed_to_a_subchain_is_a_feed_of_its_first_node() {
    // tapOuts.test.ts "is a feed of that subchain's first node, which makes the subchain part of the song".
    let kit = "start(true)\nsteps([id(\"lane-kit\").track(\"kit\"), id(\"drums\").extends(\"x\").tap_outs({ Snare: \"snare-fx\" }), to([chain(\"MIX\")])])\nsubchains([{id: \"snare-fx\", steps: [id(\"snare-verb\").extends(\"x\"), to([chain(\"MIX\")])]}])\n";
    let b = run(&[("kit", kit)], &[], &[]);
    let feeds = b.segments.tap_feeds_of();
    assert_eq!(
        (feeds[0].node.as_str(), feeds[0].tap.as_str(), feeds[0].to.as_str()),
        ("drums", "Snare", "snare-verb")
    );
    assert_eq!(dests(&b, "snare-verb"), ["MIX"]);
}

#[test]
fn a_tap_to_a_call_is_no_feed() {
    // Decision 88: a subchain run in place has no run of its own to be fed.
    let kit = "start(true)\nsteps([id(\"drums\").extends(\"x\").tap_outs({ Snare: \"s\" }), {sub: \"s\"}, to([chain(\"MIX\")])])\nsubchains([{id: \"s\", steps: [id(\"v\").extends(\"x\")]}])\n";
    let b = run(&[("kit", kit)], &[], &[]);
    assert!(b.segments.tap_feeds.is_none());
    assert_eq!(dests(&b, "drums"), ["v"]);
}

#[test]
fn a_stretch_lands_on_its_picture_lane_with_no_level() {
    // chainPicture.test.ts "reads back the same graph, the stretch landing on its lane".
    let main = "start(true)\nsteps([id(\"l\").track(\"main\"), id(\"glow\").extends(\"x\"), to([chain(\"RENDER\")])])\nsubchains([{id: \"fade\", steps: [id(\"iris\").extends(\"x\")]}])\n";
    let b = run(&[("main", main)], &["main"], &[]);
    assert_eq!(dests(&b, "iris"), ["main"]);
    assert!(b.levels.is_empty());
    let none = run(&[("main", main)], &[], &[]);
    assert!(dests(&none, "iris").is_empty());
}

#[test]
fn a_call_runs_in_place_and_is_not_compiled_alone() {
    let c = "start(true)\nsteps([id(\"a\").extends(\"x\"), {sub: \"s\"}, id(\"z\").extends(\"x\"), to([chain(\"OUT\")])])\nsubchains([{id: \"s\", steps: [id(\"v\").extends(\"x\")]}])\n";
    let b = run(&[("c", c)], &[], &[]);
    assert_eq!(
        (dests(&b, "a"), dests(&b, "v")),
        (vec!["v".to_owned()], vec!["z".to_owned()])
    );
}

#[test]
fn a_node_name_in_the_segments_is_the_lane_for_a_lane() {
    let b = run(&[("gtr", GTR)], &[], &[]);
    assert_eq!(dests(&b, "gtr"), ["amp-1"]);
}
