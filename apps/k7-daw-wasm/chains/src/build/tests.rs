// The core of the TypeScript buildGraph tests: the id scheme, the cables, the ranks, the curves, the sums, the boxes.
// The whole file set (buildGraph, buildSums, buildLanes, jacks, assembly, moduleRow) also ran unchanged against these ops through the page's facade.

use super::*;
use crate::assembly::fold_assemblies;
use serde_json::json;

fn graph(v: serde_json::Value) -> Graph {
    serde_json::from_value(v).expect("a graph")
}

fn built(v: serde_json::Value, opts: BuildOptions) -> (Vec<Node>, Vec<Edge>) {
    build_graph(&graph(v), &opts)
}

fn node<'a>(nodes: &'a [Node], id: &str) -> &'a Node {
    nodes
        .iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("no node {id}"))
}

fn ids(edges: &[Edge]) -> Vec<&str> {
    edges.iter().map(|e| e.id.as_str()).collect()
}

fn lane_through_two() -> serde_json::Value {
    json!({
        "chains": {"gtr": {"from": "gtr", "use": [{"module": "amp"}, {"module": "verb"}], "out": [{"to": "OUT", "gain": 1}]}},
        "tracks": ["gtr"]
    })
}

#[test]
fn mints_the_ids_the_layout_and_the_page_address() {
    let (nodes, edges) = built(lane_through_two(), BuildOptions::default());
    let found: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(found, ["mod:amp", "mod:verb", "trk:gtr", "snk:OUT"]);
    assert_eq!(
        ids(&edges),
        ["mod:amp->mod:verb", "trk:gtr->mod:amp", "mod:verb->snk:OUT"]
    );
    // Within-chain series cables weigh more, so the layout keeps them tight.
    assert_eq!(edges[0].weight, Some(5.0));
}

#[test]
fn ranks_inputs_lanes_modules_the_sum_and_the_sinks_top_to_bottom() {
    let (nodes, _) = built(lane_through_two(), BuildOptions::default());
    let rank = |id: &str| node(&nodes, id).rank;
    assert_eq!(rank("trk:gtr"), Some(1.0));
    assert_eq!(rank("mod:amp"), Some(2.0));
    assert_eq!(rank("mod:verb"), Some(3.0));
    assert!(rank("snk:OUT") > rank("mod:verb"));
}

#[test]
fn a_module_shared_by_two_chains_is_one_node_naming_both() {
    let g = json!({
        "chains": {
            "a": {"from": "t", "use": [{"module": "bus"}], "out": []},
            "b": {"from": "t", "use": [{"module": "bus"}], "out": []}
        },
        "tracks": ["t"]
    });
    let (nodes, _) = built(g, BuildOptions::default());
    let bus = node(&nodes, "mod:bus");
    assert_eq!(bus.chains.as_deref(), Some(&["a".to_owned(), "b".to_owned()][..]));
    assert_eq!(bus.sub.as_deref(), Some("a, b"));
    assert_eq!(nodes.iter().filter(|n| n.id == "mod:bus").count(), 1);
}

#[test]
fn draws_no_cable_into_the_mix_but_draws_the_renders() {
    let g = json!({
        "chains": {"a": {"from": "t", "use": [{"module": "m"}], "out": [{"to": "MIX", "gain": 1}, {"to": "RENDER", "gain": 0.5}]}},
        "tracks": ["t"]
    });
    let (nodes, edges) = built(g, BuildOptions::default());
    assert!(edges.iter().all(|e| e.target != "snk:MIX"));
    let to_render = edges.iter().find(|e| e.target == "snk:RENDER").expect("a render cable");
    assert_eq!(to_render.label.as_deref(), Some("50%"));
    // The sum is still a card, and the render sink has its two sockets.
    assert!(nodes.iter().any(|n| n.id == "snk:MIX"));
    assert_eq!(node(&nodes, "snk:RENDER").inputs.as_ref().map(Vec::len), Some(2));
}

#[test]
fn a_host_may_leave_the_devices_out_entirely() {
    let opts = BuildOptions {
        draw_main_out: Some(false),
        ..BuildOptions::default()
    };
    let (nodes, _) = built(lane_through_two(), opts);
    assert!(nodes.iter().all(|n| n.id != "snk:OUT"));
}

#[test]
fn the_main_out_wears_its_fader_and_the_level_the_host_knows() {
    let opts = BuildOptions {
        main_out_volume: Some(-3.0),
        ..BuildOptions::default()
    };
    let (nodes, _) = built(lane_through_two(), opts);
    let out = node(&nodes, "snk:OUT");
    assert_eq!(out.module.as_deref(), Some("OUT"));
    assert_eq!(out.params.as_ref().and_then(|p| p.get("Volume")), Some(&json!(-3.0)));
    assert!(out.ui.is_some());
}

#[test]
fn a_bound_curve_is_a_card_per_binding_with_an_automation_cable_and_an_event_cable_for_an_envelope() {
    let g = json!({
        "chains": {"a": {"from": "t", "use": [{"module": "syn", "automations": [{"param": "Cutoff", "curve": "env"}]}], "out": []}},
        "tracks": ["t"],
        "curves": {"env": {"type": "adsr", "spec": {"type": "adsr", "amplitude": 0.5, "polarity": "bipolar"}}, "free": {"type": "lfo"}}
    });
    let (nodes, edges) = built(g, BuildOptions::default());
    let card = node(&nodes, "crv:env@syn:Cutoff");
    assert_eq!(card.sub.as_deref(), Some("adsr"));
    assert_eq!(card.event_in, Some(true));
    let auto = edges
        .iter()
        .find(|e| e.automation.is_some())
        .expect("an automation cable");
    assert_eq!(auto.id, "crv:env@syn:Cutoff->mod:syn");
    let depth = auto.automation.as_ref().and_then(|a| a.depth.clone()).expect("a depth");
    assert_eq!((depth.amplitude, depth.bipolar, depth.cut), (0.5, true, None));
    let ev = edges
        .iter()
        .find(|e| e.signal == Some(Signal::Event))
        .expect("an event cable");
    assert_eq!(
        (ev.id.as_str(), ev.source_port.as_deref()),
        ("ev:mod:syn->crv:env@syn:Cutoff", Some("note-on"))
    );
    // The curve nothing binds is one free card, its id carrying no binding.
    assert_eq!(
        node(&nodes, "crv:free").curve_of.as_ref().map(|c| c.module.clone()),
        Some(None)
    );
}

#[test]
fn an_on_off_curve_has_no_depth_and_a_cut_is_kept_unless_none() {
    let curve = |v: serde_json::Value| serde_json::from_value::<crate::graph::Curve>(v).expect("a curve");
    assert_eq!(
        super::super::build_chains::depth_of(&curve(json!({"type": "onoff"}))),
        None
    );
    let d = super::super::build_chains::depth_of(&curve(json!({"type": "lfo"}))).expect("a depth");
    assert_eq!((d.amplitude, d.bipolar, d.cut), (1.0, false, None));
    let cut = json!({"type": "lfo", "spec": {"cut": "positive", "polarity": "bipolar", "amplitude": 0.4}});
    let d = super::super::build_chains::depth_of(&curve(cut)).expect("a depth");
    assert_eq!(
        (d.amplitude, d.bipolar, d.cut.as_deref()),
        (0.4, true, Some("positive"))
    );
    let none = json!({"type": "lfo", "spec": {"cut": "none"}});
    assert_eq!(
        super::super::build_chains::depth_of(&curve(none)).and_then(|d| d.cut),
        None
    );
}

#[test]
fn a_lane_no_chain_mentions_is_an_orphan_card_and_a_routed_lane_lists_its_clips() {
    let g = json!({
        "chains": {"c": {"from": "a", "use": [], "out": []}},
        "tracks": ["a", "b"],
        "trackClips": {"a": [
            {"ref": "x", "start": 0, "tc": "1:1:1", "len": 4, "kind": "audio"},
            {"ref": "y", "start": 4, "tc": "2:1:1", "kind": "midi"}
        ]}
    });
    let (nodes, _) = built(g, BuildOptions::default());
    let a = node(&nodes, "trk:a");
    assert_eq!(a.clips.as_ref().map(Vec::len), Some(2));
    assert!(a.h > node(&nodes, "trk:b").h);
    // The orphan carries no clip list: nothing routes it.
    assert!(node(&nodes, "trk:b").clips.is_none());
    assert_eq!(a.outputs.as_ref().map(|o| o[0].name.as_str()), Some("audio"));
}

#[test]
fn a_rack_tap_feed_adds_an_output_named_after_the_tap_and_a_cable_from_it() {
    let g = json!({
        "chains": {"c": {"from": "t", "use": [{"module": "kit"}], "out": []}, "sub": {"use": [{"module": "comp"}], "out": []}},
        "tracks": ["t"],
        "tapFeeds": [{"node": "kit", "tap": "BD", "to": "sub"}]
    });
    let (nodes, edges) = built(g, BuildOptions::default());
    let kit = node(&nodes, "mod:kit");
    assert_eq!(
        kit.outputs
            .as_ref()
            .map(|o| o.iter().map(|p| p.name.as_str()).collect::<Vec<_>>()),
        Some(vec!["out", "BD"])
    );
    assert!(
        edges
            .iter()
            .any(|e| e.id == "mod:kit#BD->mod:comp" && e.source_port.as_deref() == Some("BD"))
    );
}

#[test]
fn a_narrow_layout_swaps_in_the_vertical_twin_and_a_fit_folds_what_still_overflows() {
    let ui = json!({"cols": 6, "rows": 2, "widgets": {"a": {"type": "knob", "col": 0, "row": 0}},
        "vertical": {"cols": 2, "rows": 3, "widgets": {"a": {"type": "knob", "col": 0, "row": 0}}}});
    let g = json!({"chains": {"c": {"from": "t", "use": [{"module": "m", "ui": ui}], "out": []}}, "tracks": ["t"]});
    let wide = built(g.clone(), BuildOptions::default()).0;
    let narrow = built(
        g,
        BuildOptions {
            layout: Some("vertical".into()),
            ..BuildOptions::default()
        },
    )
    .0;
    let (w, n) = (node(&wide, "mod:m"), node(&narrow, "mod:m"));
    assert_eq!(
        n.ui.as_ref().map(|u| (u.cols, u.rows, u.tabs_as.clone())),
        Some((2.0, 3.0, Some("dropdown".into())))
    );
    assert!(n.w < w.w);
}

#[test]
fn a_closed_box_stands_in_for_its_members_and_an_open_one_keeps_them() {
    let g = graph(json!({
        "chains": {"gtr": {"from": "gtr", "use": [{"module": "trimA"}, {"module": "fadeA"}], "out": [{"to": "OUT", "gain": 1}]}},
        "tracks": ["gtr"],
        "assemblies": {"tt": {"uses": "tpl", "label": "Turntable", "members": ["trimA", "fadeA"],
            "inputs": [{"name": "in", "type": "audio", "node": "trimA"}],
            "outputs": [{"name": "mix", "type": "audio", "node": "fadeA"}],
            "params": {"Trim": {"node": "trimA", "param": "Trim"}}}}
    }));
    let (nodes, edges) = build_graph(&g, &BuildOptions::default());
    let closed = fold_assemblies(&g, nodes.clone(), edges.clone(), &HashSet::new());
    assert!(closed.0.iter().all(|n| n.module.as_deref() != Some("trimA")));
    let boxed = closed.0.iter().find(|n| n.kind == Kind::Assembly).expect("a box");
    assert_eq!((boxed.id.as_str(), boxed.module.as_deref()), ("asm:tt", Some("tt")));
    assert_eq!(boxed.ui.as_ref().map(|u| u.widgets.len()), Some(1));
    // The cable between the two members is what the box hides; the lane and the device meet it on its plugs.
    assert_eq!(closed.1.len(), 2);
    let open = fold_assemblies(&g, nodes, edges, &HashSet::from(["tt".to_owned()]));
    assert!(
        open.0
            .iter()
            .any(|n| n.module.as_deref() == Some("trimA") && n.assembly.as_deref() == Some("tt"))
    );
    assert!(
        open.0
            .iter()
            .find(|n| n.kind == Kind::Assembly)
            .and_then(|n| n.jacks.clone())
            .is_some_and(|j| j.inputs.is_empty())
    );
}
