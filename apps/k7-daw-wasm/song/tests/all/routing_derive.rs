// routing_derive.rs — chains derived from segments (decisions 88, 106, 107) and the port ref (ports.test.ts).
// A reader would get wrong that a lane is decided by the CALLER's closure: `derive_chains` never looks at a module.

use crate::support;

use indexmap::IndexMap;
use song::routing::{Chain, Segments, derive_chains, is_parked, port_ref, split_port, with_chain_module_refs};

fn seg(pairs: &[(&str, &[&str])], bypassed: &[&str]) -> Segments {
    let modules: IndexMap<String, Vec<String>> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect()))
        .collect();
    Segments {
        modules: Some(modules),
        bypassed: (!bypassed.is_empty()).then(|| bypassed.iter().map(|s| (*s).to_owned()).collect()),
        ..Segments::default()
    }
}

fn chains(pairs: &[(&str, &[&str])], lanes: &[&str]) -> IndexMap<String, Chain> {
    derive_chains(&seg(pairs, &[]), &|n| lanes.contains(&n))
}

fn used(c: &Chain) -> Vec<&str> {
    c.use_steps.iter().map(|s| s.module.as_str()).collect()
}

fn outs(c: &Chain) -> Vec<&str> {
    c.out.iter().map(|o| o.to.as_str()).collect()
}

#[test]
fn a_run_follows_the_arrows_and_stops_at_the_destination() {
    // chainsGraph.test.ts "follows the arrows and stops at the destination".
    let g = chains(
        &[("da.main", &["drum_1"]), ("drum_1", &["eq"]), ("eq", &["OUT"])],
        &["da.main"],
    );
    let c = &g["da.main"];
    assert_eq!(
        (c.from.as_deref(), used(c), outs(c)),
        (Some("da.main"), vec!["drum_1", "eq"], vec!["OUT"])
    );
    assert_eq!(c.out[0].gain, 1.0);
    assert_eq!(g.len(), 1);
}

#[test]
fn the_bypass_rides_on_the_step() {
    // chainsGraph.test.ts "carries the bypass onto the step".
    let g = derive_chains(&seg(&[("a", &["eq"]), ("eq", &["OUT"])], &["eq"]), &|n| n == "a");
    assert_eq!(g["a"].use_steps[0].enabled, Some(false));
}

#[test]
fn a_lane_with_no_modules_is_just_its_output_and_one_feeding_nothing_is_ignored() {
    // chainsGraph.test.ts "gives a lane with no modules a chain that is just its output", "ignores a lane that feeds nothing".
    let g = chains(&[("a", &["OUT"])], &["a"]);
    assert!(g["a"].use_steps.is_empty());
    let g = chains(&[("a", &[]), ("b", &["OUT"])], &["a", "b"]);
    assert_eq!(g.keys().collect::<Vec<_>>(), ["b"]);
}

#[test]
fn a_run_breaks_where_two_signals_meet() {
    // chainsGraph.test.ts "breaks where two signals MEET".
    let g = chains(&[("a", &["verb"]), ("b", &["verb"]), ("verb", &["OUT"])], &["a", "b"]);
    assert_eq!(outs(&g["a"]), ["verb"]);
    assert_eq!((used(&g["verb"]), outs(&g["verb"])), (vec!["verb"], vec!["OUT"]));
}

#[test]
fn a_run_breaks_where_the_signal_splits_and_a_branch_starts_its_own_chain() {
    // chainsGraph.test.ts "breaks where the signal SPLITS", "does not absorb a module into a run that only sends it a share".
    let g = chains(
        &[
            ("a", &["eq"]),
            ("eq", &["verb", "OUT"]),
            ("verb", &["tail"]),
            ("tail", &["OUT"]),
        ],
        &["a"],
    );
    assert_eq!((used(&g["a"]), outs(&g["a"])), (vec!["eq"], vec!["verb", "OUT"]));
    assert_eq!(used(&g["verb"]), ["verb", "tail"]);
}

#[test]
fn a_lane_is_never_absorbed() {
    // chainsGraph.test.ts "never absorbs a LANE, whatever feeds it".
    let g = chains(&[("in-1", &["gtr"]), ("gtr", &["amp"]), ("amp", &["OUT"])], &["gtr"]);
    assert_eq!((g["gtr"].from.as_deref(), used(&g["gtr"])), (Some("gtr"), vec!["amp"]));
    assert_eq!((used(&g["in-1"]), outs(&g["in-1"])), (vec!["in-1"], vec!["gtr"]));
}

#[test]
fn nodes_at_the_edges() {
    // chainsGraph.test.ts "keeps a module nothing feeds, wired to nothing", "does not call a wired chain parked", "treats `@in` as a lane".
    let g = chains(&[("@engine/io/IN/in-1", &[])], &[]);
    assert!(is_parked(&g["@engine/io/IN/in-1"]));
    let g = chains(&[("a", &["eq"]), ("eq", &["OUT"])], &["a"]);
    assert!(!is_parked(&g["a"]));
    let g = chains(&[("@in", &["eq"]), ("eq", &["@out"])], &[]);
    assert_eq!(
        (g["@in"].from.as_deref(), used(&g["@in"]), outs(&g["@in"])),
        (Some("@in"), vec!["eq"], vec!["@out"])
    );
}

#[test]
fn a_cycle_keeps_its_modules_visible() {
    // chainsGraph.test.ts "keeps a cycle's modules visible instead of dropping them".
    let g = chains(&[("a", &["b"]), ("b", &["a"])], &[]);
    assert_eq!(g.keys().collect::<Vec<_>>(), ["a"]);
    assert_eq!(used(&g["a"]), ["a", "b"]);
}

#[test]
fn nothing_and_a_single_lane() {
    // chainsGraph.test.ts "reads nothing as no chains", "no longer has a track/module collision to refuse".
    assert!(derive_chains(&Segments::default(), &|_| false).is_empty());
    let g = chains(&[("x", &["OUT"])], &["x"]);
    assert_eq!(g["x"].from.as_deref(), Some("x"));
}

#[test]
fn a_port_ref_splits_on_the_last_separator() {
    // ports.test.ts "a port ref": the last `#` is the port, and a half-written ref is a plain name.
    assert_eq!(split_port("crossfade#b"), ("crossfade", Some("b")));
    assert_eq!(split_port("a#b#c"), ("a#b", Some("c")));
    for plain in ["x", "x#", "#x", "#", ""] {
        assert_eq!(split_port(plain), (plain, None), "{plain}");
    }
    assert_eq!(port_ref("a", Some("b")), "a#b");
    assert_eq!(port_ref("a", None), "a");
}

#[test]
fn wiring_a_named_port_records_the_feeder_on_the_step_not_the_flow() {
    // ports.test.ts "records the feeder on the step, not as the flow", "does not let a side input absorb the module".
    let g = chains(&[("a", &["amp"]), ("side", &["amp#sc"]), ("amp", &["OUT"])], &["a"]);
    let step = g["a"].use_steps.iter().find(|s| s.module == "amp").expect("amp");
    assert_eq!(step.inputs.as_deref(), Some(&["side".to_owned()][..]));
    assert!(g.contains_key("side"));
}

#[test]
fn the_destinations_of_a_module_keyed_with_a_port_are_unioned() {
    // ports.test.ts "unions the destinations of a module keyed with a port on the source side" (106: a union of port keys).
    let g = chains(
        &[
            ("a", &["fx"]),
            ("fx", &["x"]),
            ("fx#aux", &["y"]),
            ("x", &["OUT"]),
            ("y", &["OUT"]),
        ],
        &["a"],
    );
    assert_eq!(outs(&g["a"]), ["x", "y"]);
    assert_eq!(g["a"].out[1].from_port.as_deref(), Some("aux"));
}

#[test]
fn a_lane_keyed_on_more_than_one_output_is_more_than_one_source() {
    // ports.test.ts "ends the run at a module that splits across its own outputs": a split lane is keyed per output.
    let g = chains(
        &[
            ("L#audio", &["a"]),
            ("L#midi", &["b"]),
            ("a", &["OUT"]),
            ("b", &["OUT"]),
        ],
        &["L"],
    );
    assert_eq!(g["L#audio"].from_port.as_deref(), Some("audio"));
    assert_eq!(g["L#midi"].from.as_deref(), Some("L"));
}

#[test]
fn self_loops_and_a_wide_fan_in_derive() {
    // Decision 106: `a: [a]`, a cycle, a fan-in.
    let g = chains(&[("a", &["a"])], &[]);
    assert_eq!((used(&g["a"]), outs(&g["a"])), (vec!["a"], vec!["a"]));
}

#[test]
fn a_vocabulary_module_a_chain_names_arrives_by_a_step_and_by_an_out() {
    // Decision 108: added as it stands when no node has that name; a ref named by an out's `to` works as well as a step's.
    let by_ref = support::by_ref(&[
        ("@k7-core/audio/reverb", "plugin({ type: \"internal\", name: \"R\" })\n"),
        ("@k7-core/audio/amp", "plugin({ type: \"internal\", name: \"A\" })\n"),
        ("@k7-core/audio/unused", "plugin({ type: \"internal\", name: \"U\" })\n"),
    ]);
    let nodes: IndexMap<String, song::LoadedModule> = IndexMap::from([(
        "mine".to_owned(),
        support::node_of("mine", "plugin({ type: \"internal\", name: \"M\" })\n"),
    )]);
    let g = chains(
        &[
            ("mine", &["@k7-core/audio/reverb"]),
            ("@k7-core/audio/reverb", &["@k7-core/audio/amp"]),
        ],
        &[],
    );
    let out = with_chain_module_refs(&nodes, &by_ref, &g);
    let mut keys: Vec<&str> = out.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["@k7-core/audio/amp", "@k7-core/audio/reverb", "mine"]);
}
