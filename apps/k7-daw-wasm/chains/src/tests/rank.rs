// The passes that line modules up on one rank and space them evenly.

use super::*;
use crate::module_row::{RankNode, align_ranks, space_modules};
use serde_json::json;
use std::collections::HashMap;

fn ranked(nodes: &[Value]) -> Vec<RankNode> {
    nodes
        .iter()
        .filter_map(|n| serde_json::from_value(n.clone()).ok())
        .collect()
}

fn names(m: &[&str]) -> Vec<String> {
    m.iter().map(|s| (*s).to_owned()).collect()
}

fn staircase() -> Value {
    doc(
        json!({
            "a": {"from": "vox", "use": [{"module": "comp"}, {"module": "mx-vox"}], "out": [{"to": "OUT", "gain": 1}]},
            "b": {"from": "bass", "use": [{"module": "mx-bass"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox", "bass"]),
    )
}

fn rank_of(nodes: &[Value], m: &str) -> f64 {
    num(&module(nodes, m)["rank"])
}

#[test]
fn align_leaves_the_graph_alone_when_fewer_than_two_of_the_names_are_in_it() {
    let b = built(&staircase(), json!({}));
    for asked in [&["mx-vox"][..], &[], &["nope", "gone"]] {
        assert!(align_ranks(&ranked(&b.nodes), &names(asked)).is_empty());
    }
}

#[test]
fn align_puts_them_on_one_rank_the_deepest_never_a_shallower_one() {
    let b = built(&staircase(), json!({}));
    let before = rank_of(&b.nodes, "mx-vox");
    assert!(before > rank_of(&b.nodes, "mx-bass"));
    let out = b.aligned(&["mx-vox", "mx-bass"]);
    assert_eq!(rank_of(&out, "mx-bass"), before);
    assert_eq!(rank_of(&out, "mx-vox"), before);
}

#[test]
fn align_pushes_the_sink_past_the_row_so_no_cable_has_to_climb() {
    let b = built(&staircase(), json!({}));
    let out = b.aligned(&["mx-vox", "mx-bass"]);
    let shared = rank_of(&out, "mx-vox");
    for n in out.iter().filter(|n| n["kind"] == "sink") {
        assert!(n["rank"].as_f64().unwrap_or(0.0) > shared);
    }
}

#[test]
fn align_does_not_touch_the_nodes_it_was_given() {
    let b = built(&staircase(), json!({}));
    let was = rank_of(&b.nodes, "mx-bass");
    b.aligned(&["mx-vox", "mx-bass"]);
    assert_eq!(rank_of(&b.nodes, "mx-bass"), was);
}

fn uneven() -> Value {
    doc(
        json!({
            "a": {"from": "vox", "use": [{"module": "comp", "ui": {"cols": 8, "rows": 2, "widgets": {}}}, {"module": "mx-vox"}],
                  "out": [{"to": "OUT", "gain": 1}, {"to": "reverb", "gain": 0.3}]},
            "reverb": {"use": [{"module": "conv"}, {"module": "mx-verb"}], "out": [{"to": "OUT", "gain": 1}]},
            "b": {"from": "bass", "use": [{"module": "mx-bass"}], "out": [{"to": "OUT", "gain": 1}]},
            "c": {"from": "gtr", "use": [{"module": "mx-gtr"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox", "bass", "gtr"]),
    )
}

const ROW: [&str; 4] = ["mx-vox", "mx-verb", "mx-bass", "mx-gtr"];

fn laid_row() -> (Value, Value) {
    let b = built(&uneven(), json!({}));
    (b.laid_aligned(&ROW), b.laid(json!(null), json!(null)))
}

#[test]
fn space_leaves_the_graph_alone_when_fewer_than_two_of_the_names_are_in_it() {
    let (_, laid) = laid_row();
    let nodes = ranked(&list(&laid["nodes"]));
    assert!(space_modules(&nodes, &names(&["mx-vox"])).is_none());
    assert!(space_modules(&nodes, &[]).is_none());
}

fn x_of(laid: &Value, moved: &[(String, f64)], m: &str) -> (f64, f64) {
    let n = module(&list(&laid["nodes"]), m).clone();
    let at = moved
        .iter()
        .find(|(id, _)| *id == text(&n["id"]))
        .map_or(num(&n["x"]), |(_, x)| *x);
    (at, num(&n["w"]))
}

#[test]
fn space_leaves_the_same_gap_between_every_pair_and_it_is_the_strips() {
    let (laid, _) = laid_row();
    let spaced = space_modules(&ranked(&list(&laid["nodes"])), &names(&ROW)).expect("a spacing");
    let mut cards: Vec<(f64, f64)> = ROW.iter().map(|m| x_of(&laid, &spaced.moved, m)).collect();
    cards.sort_by(|a, b| a.0.total_cmp(&b.0));
    // One gap, and 24 is the assertion: "all gaps equal" would pass with the pass removed.
    let gaps: Vec<f64> = cards.windows(2).map(|w| w[1].0 - (w[0].0 + w[0].1)).collect();
    assert_eq!(gaps, [24.0, 24.0, 24.0]);
}

#[test]
fn space_orders_them_as_asked_not_as_the_graph_laid_them_out() {
    let (laid, _) = laid_row();
    let spaced = space_modules(&ranked(&list(&laid["nodes"])), &names(&ROW)).expect("a spacing");
    let xs: Vec<f64> = ROW.iter().map(|m| x_of(&laid, &spaced.moved, m).0).collect();
    assert!(xs.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn space_starts_where_the_row_already_began_rather_than_at_the_origin() {
    let (laid, _) = laid_row();
    let was = list(&laid["nodes"])
        .iter()
        .filter(|n| text(&n["module"]).starts_with("mx-"))
        .map(|n| num(&n["x"]))
        .fold(f64::INFINITY, f64::min);
    let spaced = space_modules(&ranked(&list(&laid["nodes"])), &names(&ROW)).expect("a spacing");
    let first = ROW
        .iter()
        .map(|m| x_of(&laid, &spaced.moved, m).0)
        .fold(f64::INFINITY, f64::min);
    assert_eq!(first, was);
}

#[test]
fn space_widens_the_graph_when_the_row_runs_past_what_the_layout_measured() {
    let (laid, _) = laid_row();
    let spaced = space_modules(&ranked(&list(&laid["nodes"])), &names(&ROW)).expect("a spacing");
    let right = ROW
        .iter()
        .map(|m| x_of(&laid, &spaced.moved, m))
        .map(|(x, w)| x + w)
        .fold(f64::MIN, f64::max);
    assert!(num(&laid["width"]).max(spaced.right) >= right);
}

#[test]
fn align_pushes_only_a_sink_that_sat_at_or_above_the_row_and_never_a_sum() {
    let node = |id: &str, kind: &str, label: &str, module: Option<&str>, rank: f64| -> RankNode {
        serde_json::from_value(json!({"id": id, "kind": kind, "label": label, "module": module, "rank": rank}))
            .expect("a node")
    };
    let nodes = [
        node("mod:a", "module", "a", Some("a"), 3.0),
        node("mod:b", "module", "b", Some("b"), 1.0),
        node("snk:OUT", "sink", "OUT", None, 2.0),
        node("snk:FAR", "sink", "FAR", None, 9.0),
        node("snk:MIX", "sink", "MIX", None, 2.0),
    ];
    let changed: HashMap<String, f64> = align_ranks(&nodes, &names(&["a", "b"])).into_iter().collect();
    let want = HashMap::from([
        ("mod:a".to_owned(), 3.0),
        ("mod:b".to_owned(), 3.0),
        ("snk:OUT".to_owned(), 4.0),
    ]);
    assert_eq!(changed, want);
}
