// The two rules on their own, away from the engine: a receiver below its inputs, and the master run as a block.

use super::arrange::{master_run_aside, push_below_inputs, x, y};
use crate::node::{Edge, Kind, Node};

fn card(id: &str, kind: Kind, px: f64, py: f64) -> Node {
    let mut n = Node::plain(id, id, kind);
    (n.x, n.y, n.w, n.h) = (Some(px), Some(py), 100.0, 40.0);
    n
}

fn edges(pairs: &[(&str, &str)]) -> Vec<Edge> {
    pairs.iter().map(|(s, t)| Edge::new(&format!("{s}>{t}"), s, t)).collect()
}

fn find<'a>(nodes: &'a [Node], id: &str) -> &'a Node {
    nodes.iter().find(|n| n.id == id).expect("a node")
}

#[test]
fn a_push_is_carried_all_the_way_down_a_chain() {
    let nodes = vec![card("a", Kind::Module, 0.0, 500.0), card("b", Kind::Module, 0.0, 100.0), card("c", Kind::Module, 0.0, 200.0)];
    let out = push_below_inputs(nodes, &edges(&[("a", "b"), ("b", "c")]));
    assert_eq!([y(&out[0]), y(&out[1]), y(&out[2])], [500.0, 540.0, 580.0]);
}

#[test]
fn the_lowest_of_several_inputs_is_cleared() {
    let nodes = vec![card("a", Kind::Module, 0.0, 10.0), card("b", Kind::Module, 0.0, 900.0), card("sum", Kind::Module, 0.0, 0.0)];
    let out = push_below_inputs(nodes, &edges(&[("a", "sum"), ("b", "sum")]));
    assert_eq!(y(&out[2]), 940.0);
}

#[test]
fn nothing_out_of_place_nothing_moves() {
    let nodes = vec![card("a", Kind::Module, 0.0, 0.0), card("b", Kind::Module, 0.0, 100.0)];
    assert_eq!(push_below_inputs(nodes.clone(), &edges(&[("a", "b")])), nodes);
}

#[test]
fn a_cycle_is_left_alone() {
    let nodes = vec![card("a", Kind::Module, 0.0, 0.0), card("b", Kind::Module, 0.0, 0.0)];
    assert_eq!(push_below_inputs(nodes.clone(), &edges(&[("a", "b"), ("b", "a")])), nodes);
}

#[test]
fn a_node_feeding_itself_and_a_cable_to_nowhere_constrain_nothing() {
    let nodes = vec![card("a", Kind::Module, 0.0, 0.0)];
    assert_eq!(push_below_inputs(nodes.clone(), &edges(&[("a", "a")])), nodes);
    assert_eq!(push_below_inputs(nodes.clone(), &edges(&[("ghost", "a")])), nodes);
}

#[test]
fn the_master_run_moves_as_a_block_on_both_axes() {
    let nodes = vec![
        card("MIX", Kind::Sink, 0.0, 0.0),
        card("OUT", Kind::Sink, 40.0, 90.0),
        card("a", Kind::Module, 500.0, 300.0),
        card("b", Kind::Module, 900.0, 700.0),
    ];
    let out = master_run_aside(nodes, &edges(&[("MIX", "OUT"), ("a", "MIX"), ("b", "MIX")]));
    assert_eq!((x(find(&out, "MIX")), y(find(&out, "MIX"))), (1060.0, 810.0));
    assert_eq!((x(find(&out, "OUT")), y(find(&out, "OUT"))), (1100.0, 900.0));
    assert_eq!((x(find(&out, "a")), y(find(&out, "a"))), (500.0, 300.0));
}

#[test]
fn a_run_left_too_far_out_is_pulled_back_in() {
    let nodes = vec![card("MIX", Kind::Sink, 5000.0, 5000.0), card("a", Kind::Module, 0.0, 0.0)];
    let out = master_run_aside(nodes, &edges(&[("a", "MIX")]));
    assert_eq!((x(find(&out, "MIX")), y(find(&out, "MIX"))), (160.0, 110.0));
}

#[test]
fn the_whole_run_travels_however_deep() {
    let nodes = vec![
        card("MIX", Kind::Sink, 0.0, 0.0),
        card("limiter", Kind::Module, 10.0, 50.0),
        card("OUT", Kind::Sink, 20.0, 100.0),
        card("a", Kind::Module, 400.0, 400.0),
    ];
    let out = master_run_aside(nodes, &edges(&[("MIX", "limiter"), ("limiter", "OUT"), ("a", "MIX")]));
    for id in ["MIX", "limiter", "OUT"] {
        assert!(x(find(&out, id)) >= 500.0 && y(find(&out, id)) >= 440.0, "{id}");
    }
}

#[test]
fn no_sum_or_a_run_that_is_the_whole_graph_changes_nothing() {
    let nodes = vec![card("a", Kind::Module, 0.0, 0.0), card("OUT", Kind::Sink, 100.0, 100.0)];
    assert_eq!(master_run_aside(nodes.clone(), &edges(&[("a", "OUT")])), nodes);
    let nodes = vec![card("MIX", Kind::Sink, 0.0, 0.0), card("OUT", Kind::Sink, 40.0, 90.0)];
    assert_eq!(master_run_aside(nodes.clone(), &edges(&[("MIX", "OUT")])), nodes);
}
