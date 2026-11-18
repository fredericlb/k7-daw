// lanes_ownership.rs — which track each chain's steps land on (decisions 252 to 256): masterChain.test.ts, tapOuts.test.ts, liveInput.test.ts.
// A reader would get wrong that all four owned kinds are named after the CHAIN: a lane-less input's track is its run's first module.

use crate::lanes_support;

use lanes_support::{inputs, inputs_with, lane};
use serde_json::json;
use song::lanes::inputs::Inputs;
use song::lanes::ownership::{Ownership, chain_ownership};

const AMP: &str = "plugin({ type: \"internal\", name: \"Chorus\" })\n";
const REC: &str = "io(\"in\")\nchannel([1])\n";

fn own(i: &Inputs) -> Ownership {
    chain_ownership(i, &i.lanes())
}

fn graph(modules: &str) -> (String, String) {
    ("graph.k7x".to_owned(), format!("modules({{ {modules} }})\n"))
}

/// A project of lane nodes `gtr`, `drums`, plugin nodes `amp`, `fx`, `verb`, an input `rec`, and one graph.
fn project(modules: &str, extra: &[(&str, &str)]) -> Inputs {
    let (g, text) = graph(modules);
    let mut files: Vec<(&str, String)> = vec![
        ("nodes/gtr.k7x", lane("")),
        ("nodes/drums.k7x", lane("")),
        ("nodes/amp.k7x", AMP.to_owned()),
        ("nodes/fx.k7x", AMP.to_owned()),
        ("nodes/verb.k7x", AMP.to_owned()),
        ("nodes/rec.k7x", REC.to_owned()),
        (g.as_str(), text),
    ];
    files.extend(extra.iter().map(|(a, b)| (*a, (*b).to_owned())));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    inputs(&borrowed)
}

// masterChain.test.ts "a lane's chain": a lane feeds a chain named after the lane, which owns the track.
#[test]
fn a_lanes_chain_is_its_own_track() {
    let i = project("gtr: [\"amp\"], amp: [\"MIX\"]", &[]);
    let o = own(&i);
    assert_eq!(o.owner_of("gtr"), Some("gtr"));
    assert!(o.bus_chains.is_empty() && o.input_chains.is_empty() && o.tap_chains.is_empty());
    assert_eq!(o.owner_of("nope"), None);
}

// masterChain.test.ts "does NOT become a track of its own", "still becomes a master chain the moment a module joins it".
#[test]
fn a_chain_from_mix_to_out_is_owned_by_the_master_with_a_step_or_none() {
    let empty = project("gtr: [\"amp\"], amp: [\"MIX\"], MIX: [\"OUT\"]", &[]);
    assert_eq!(own(&empty).owner_of("MIX"), Some("@master"));
    let with_step = project("gtr: [\"amp\"], amp: [\"MIX\"], MIX: [\"fx\"], fx: [\"OUT\"]", &[]);
    let o = own(&with_step);
    assert_eq!(o.owner_of("MIX"), Some("@master"));
    assert!(!o.bus_chains.contains("MIX") && o.input_chains.is_empty());
    // A run from MIX that does not end at OUT is nobody's.
    let open = project("gtr: [\"amp\"], amp: [\"MIX\"], MIX: [\"fx\"], fx: [\"RENDER\"]", &[]);
    assert_eq!(own(&open).owner_of("MIX"), None);
}

// A bus: a non-sink target naming a chain no lane feeds, in the order the outs are met.
#[test]
fn a_bus_fed_by_two_lanes_owns_a_track_named_after_its_chain() {
    let i = project("gtr: [\"verb\"], drums: [\"verb\"], verb: [\"MIX\"]", &[]);
    let o = own(&i);
    assert_eq!(o.bus_chains.iter().collect::<Vec<_>>(), ["verb"]);
    assert_eq!(o.owner_of("verb"), Some("verb"));
    assert_eq!(o.owner_of("gtr"), Some("gtr"));
}

// cue.md: a run leaving by CUE is cued; the pair comes from the rig's out labels or numbers.
#[test]
fn a_run_reaching_cue_is_cued_and_the_pair_is_the_rigs() {
    let m = "gtr: [\"verb\"], drums: [\"verb\"], verb: [\"CUE\"]";
    let bare = own(&project(m, &[]));
    assert!(bare.is_cued("verb") && !bare.is_cued("gtr"));
    assert!(bare.cue_pair.is_empty());
    let (g, text) = graph(m);
    let files: Vec<(&str, &str)> = vec![
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/drums.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/verb.k7x", AMP),
        (g.as_str(), text.as_str()),
    ];
    let rig = json!({"channels": {"out": [{"ch": [5, 6], "label": "phones"}]}, "cueOutput": "phones"});
    let i = inputs_with(&files, &rig);
    let o = own(&i);
    assert_eq!(o.cue_pair, [5, 6]);
    assert_eq!(o.owner_of("verb"), Some("verb"));
    let numbers = inputs_with(&files, &json!({"cueOutput": [3, 4]}));
    assert_eq!(own(&numbers).cue_pair, [3, 4]);
}

// A picture lane is never a bus, though a fade's stretch routes into its run; a port on a built-in or a chain is no bus either.
#[test]
fn a_picture_lane_a_port_and_a_sink_make_no_bus() {
    let pic = project(
        "gtr: [\"fx\"], fx: [\"cam\"], cam: [\"VIDEO_OUT\"]",
        &[("nodes/cam.k7x", "extends(\"@meta/picture-track\")\n")],
    );
    assert!(own(&pic).bus_chains.is_empty());
    let ported = project(
        "gtr: [\"verb\", \"verb#side\"], drums: [\"verb\"], verb: [\"MIX\"]",
        &[],
    );
    assert_eq!(own(&ported).bus_chains.iter().collect::<Vec<_>>(), ["verb"]);
    let side = project("gtr: [\"verb#side\"], verb: [\"MIX\"]", &[]);
    assert!(side.chains.contains_key("verb") && own(&side).bus_chains.is_empty());
    let builtin = project("gtr: [\"RENDER#audio\"]", &[]);
    assert!(own(&builtin).bus_chains.is_empty());
}

// liveInput.test.ts "MAKES its own track": an input run reaching no lane owns a track named after the chain.
#[test]
fn a_lane_less_input_owns_a_track_named_after_its_chain() {
    let i = project("rec: [\"OUT\"]", &[]);
    let o = own(&i);
    assert_eq!(o.input_chains.iter().collect::<Vec<_>>(), ["rec"]);
    assert_eq!(o.owner_of("rec"), Some("rec"));
}

// liveInput.test.ts "lands on the track its run ends at": a run that ends at a lane is the ordinary recording shape.
#[test]
fn an_input_that_lands_on_a_lane_owns_no_track() {
    let i = project("rec: [\"gtr\"], gtr: [\"OUT\"]", &[]);
    let o = own(&i);
    assert!(o.input_chains.is_empty() && o.owner_of("rec").is_none());
}

// An input in a bus is not an input chain; an input in the master is the master's.
#[test]
fn an_input_in_a_bus_or_the_master_is_not_an_input_chain() {
    let bus = project(
        "gtr: [\"verb\"], drums: [\"verb\"], verb: [\"rec\"], rec: [\"OUT\"]",
        &[],
    );
    let o = own(&bus);
    assert_eq!(o.bus_chains.iter().collect::<Vec<_>>(), ["verb"]);
    assert!(o.input_chains.is_empty());
    let master = project("gtr: [\"MIX\"], MIX: [\"rec\"], rec: [\"OUT\"]", &[]);
    let o = own(&master);
    assert_eq!(o.owner_of("MIX"), Some("@master"));
    assert!(o.input_chains.is_empty());
}

const KIT: &str =
    "plugin({ type: \"internal\", name: \"Sampler\" })\ntap_outs({ Snare: \"snare-fx\", Clap: \"clap-fx\" })\n";

fn kit_project(feeds: &str) -> Inputs {
    project(
        "gtr: [\"MIX\"], \"snare-fx\": [\"OUT\"], \"clap-fx\": [\"OUT\"]",
        &[
            ("nodes/kit.k7x", KIT),
            ("nodes/snare-fx.k7x", AMP),
            ("nodes/clap-fx.k7x", AMP),
            (
                "graph.k7x",
                &format!("modules({{ gtr: [\"MIX\"], \"snare-fx\": [\"OUT\"], \"clap-fx\": [\"OUT\"] }})\n{feeds}"),
            ),
        ],
    )
}

// tapOuts.test.ts "leaves by a pair of its own": the run a tap feeds owns a track, keyed by the chain.
#[test]
fn a_tap_with_a_run_owns_it_and_a_tap_with_none_is_a_fault() {
    let all = kit_project(
        "tap_feeds([{ node: \"kit\", tap: \"Snare\", to: \"snare-fx\" }, { node: \"kit\", tap: \"Clap\", to: \"clap-fx\" }])\n",
    );
    let o = own(&all);
    assert!(
        o.faults.is_empty(),
        "{:?}",
        o.faults.iter().map(|f| f.key()).collect::<Vec<_>>()
    );
    let taps: Vec<_> = o
        .tap_chains
        .iter()
        .map(|(k, t)| (k.as_str(), t.node.as_str(), t.pair))
        .collect();
    assert_eq!(taps, [("clap-fx", "kit", 1), ("snare-fx", "kit", 2)]);
    assert_eq!(o.owner_of("snare-fx"), Some("snare-fx"));
    // tapOuts.test.ts "is refused when it names no subchain": a fault per tap, not a throw at the first.
    let none = kit_project("");
    let keys: Vec<_> = own(&none).faults.iter().map(|f| f.key()).collect();
    assert_eq!(keys, ["tap_unrouted|node:kit|Snare", "tap_unrouted|node:kit|Clap"]);
    let one = kit_project("tap_feeds([{ node: \"kit\", tap: \"Snare\", to: \"snare-fx\" }])\n");
    let keys: Vec<_> = own(&one).faults.iter().map(|f| f.key()).collect();
    assert_eq!(keys, ["tap_unrouted|node:kit|Clap"]);
}

// 253: a later tap on one run replaces the value and keeps the position (Map.set).
#[test]
fn two_taps_on_one_run_keep_the_first_position_and_the_last_value() {
    let kit = "plugin({ type: \"internal\", name: \"Sampler\" })\ntap_outs({ a: \"r1\", b: \"r2\", c: \"r1\" })\n";
    let i = project(
        "gtr: [\"MIX\"], r1: [\"OUT\"], r2: [\"OUT\"]",
        &[
            ("nodes/kit.k7x", kit),
            ("nodes/r1.k7x", AMP),
            ("nodes/r2.k7x", AMP),
            (
                "graph.k7x",
                "modules({ gtr: [\"MIX\"], r1: [\"OUT\"], r2: [\"OUT\"] })\ntap_feeds([{ node: \"kit\", tap: \"a\", to: \"r1\" }, { node: \"kit\", tap: \"b\", to: \"r2\" }, { node: \"kit\", tap: \"c\", to: \"r1\" }])\n",
            ),
        ],
    );
    let o = own(&i);
    let taps: Vec<_> = o.tap_chains.iter().map(|(k, t)| (k.as_str(), t.pair)).collect();
    assert_eq!(taps, [("r1", 3), ("r2", 2)]);
}

// 255: a chain's owner, a lane's own name, the owner of the chain holding a step, and nothing.
#[test]
fn a_side_input_comes_from_a_chain_a_lane_a_step_or_nothing() {
    let i = project(
        "gtr: [\"amp\"], amp: [\"fx\"], fx: [\"MIX\"], drums: [\"MIX\"], MIX: [\"OUT\"]",
        &[],
    );
    let o = own(&i);
    assert_eq!(o.side_input_track("gtr"), Some("gtr"));
    assert_eq!(o.side_input_track("MIX"), Some("@master"));
    assert_eq!(o.side_input_track("drums"), Some("drums"));
    assert_eq!(o.side_input_track("fx"), Some("gtr"));
    assert_eq!(o.side_input_track("ghost"), None);
    // A lane that has no chain at all is still a lane.
    let alone = project("gtr: [\"MIX\"]", &[]);
    assert_eq!(own(&alone).side_input_track("drums"), Some("drums"));
}

// 256: a chain named like an integer is met in file order, not JavaScript's numeric-first order.
#[test]
fn integer_like_chain_names_keep_file_order() {
    let i = project(
        "\"10\": [\"OUT\"], \"2\": [\"OUT\"]",
        &[("nodes/10.k7x", REC), ("nodes/2.k7x", REC)],
    );
    let o = own(&i);
    assert_eq!(o.input_chains.iter().collect::<Vec<_>>(), ["10", "2"]);
}
